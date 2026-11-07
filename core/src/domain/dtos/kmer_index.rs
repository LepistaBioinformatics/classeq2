//! The frozen k-mer index and the `.cls` v2 database file
//! (005-placement-performance, design D1).
//!
//! `KmersMap` stays the build-time structure and the model of the legacy
//! YAML format. Placement only reads a `KmerIndex`: sorted bucket keys,
//! sorted k-mer hashes per bucket and a sorted posting list of node ids per
//! k-mer, either owned (frozen from a `KmersMap`) or mapped from a v2 file.

use super::{
    annotation::Annotation,
    clade::Clade,
    kmers_map::{KmersMap, MinimizerKey, MinimizerValue},
    tree::Tree,
};

use memmap2::Mmap;
use mur3::murmurhash3_x64_128;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::{self, Write},
    ops::Range,
    path::Path,
};
use thiserror::Error;
use uuid::Uuid;
use zerocopy::{AsBytes, FromBytes};

#[cfg(target_endian = "big")]
compile_error!("The classeq database format is little-endian only.");

/// The first eight bytes of a v2 database.
pub const MAGIC: [u8; 8] = *b"CLSQIDX\x02";

/// The layout version written after the magic.
pub const FORMAT_VERSION: u32 = 1;

const HEADER_LEN: usize = 128;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Could not read or write the database: {0}")]
    Io(#[from] io::Error),

    #[error("The file is not a classeq v2 database")]
    BadMagic,

    #[error("Unsupported database format version {0}")]
    UnsupportedVersion(u32),

    #[error("Corrupted database: {0}")]
    Corrupted(String),

    #[error("Invalid tree section: {0}")]
    Tree(#[from] serde_json::Error),
}

fn corrupted(msg: impl Into<String>) -> DatabaseError {
    DatabaseError::Corrupted(msg.into())
}

// ? ---------------------------------------------------------------------------
// ? Query k-mers
// ? ---------------------------------------------------------------------------

fn hash_bytes(bytes: &[u8]) -> u64 {
    murmurhash3_x64_128(bytes, 0).0
}

fn reverse_complement(sequence: &str) -> String {
    sequence
        .chars()
        .rev()
        .map(|c| match c {
            'a' | 'A' => 'T',
            't' | 'T' => 'A',
            'c' | 'C' => 'G',
            'g' | 'G' => 'C',
            _ => panic!("Invalid character in sequence"),
        })
        .collect()
}

/// The `(bucket key, hash)` pair of every k-mer of both strands, in the same
/// order and with the same values as `KmersMap::build_kmer_from_string`
/// followed by the minimizer of each k-mer, without one `String` per k-mer.
pub fn query_kmers(
    sequence: &str,
    k_size: u64,
    m_size: u64,
) -> Vec<(u64, u64)> {
    let k = k_size as usize;

    if sequence.len() < k {
        return vec![];
    }

    let forward = sequence.to_uppercase();
    let reverse = reverse_complement(sequence);
    let m = (m_size as usize).min(k);
    let mut kmers = Vec::with_capacity(2 * (sequence.len() - k + 1));

    for strand in [forward.as_bytes(), reverse.as_bytes()] {
        for window in strand.windows(k) {
            let key = if m_size == 0 {
                0
            } else {
                hash_bytes(&window[..m])
            };
            kmers.push((key, hash_bytes(window)));
        }
    }

    kmers
}

// ? ---------------------------------------------------------------------------
// ? Array views
// ? ---------------------------------------------------------------------------

/// The node ids indexed under one k-mer, sorted ascending.
#[derive(Clone, Copy, Debug)]
pub enum PostingList<'a> {
    U16(&'a [u16]),
    U32(&'a [u32]),
}

impl<'a> PostingList<'a> {
    pub fn len(&self) -> usize {
        match self {
            PostingList::U16(ids) => ids.len(),
            PostingList::U32(ids) => ids.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, node: u64) -> bool {
        match self {
            PostingList::U16(ids) => u16::try_from(node)
                .is_ok_and(|node| ids.binary_search(&node).is_ok()),
            PostingList::U32(ids) => u32::try_from(node)
                .is_ok_and(|node| ids.binary_search(&node).is_ok()),
        }
    }

    pub fn to_vec(&self) -> Vec<u64> {
        match self {
            PostingList::U16(ids) => ids.iter().map(|id| *id as u64).collect(),
            PostingList::U32(ids) => ids.iter().map(|id| *id as u64).collect(),
        }
    }

    fn slice(&self, range: Range<usize>) -> Self {
        match self {
            PostingList::U16(ids) => PostingList::U16(&ids[range]),
            PostingList::U32(ids) => PostingList::U32(&ids[range]),
        }
    }

    fn as_bytes(&self) -> &'a [u8] {
        match self {
            PostingList::U16(ids) => ids.as_bytes(),
            PostingList::U32(ids) => ids.as_bytes(),
        }
    }
}

#[derive(Clone, Copy)]
enum Offsets<'a> {
    U32(&'a [u32]),
    U64(&'a [u64]),
}

impl<'a> Offsets<'a> {
    fn get(&self, index: usize) -> u64 {
        match self {
            Offsets::U32(offsets) => offsets[index] as u64,
            Offsets::U64(offsets) => offsets[index],
        }
    }

    fn as_bytes(&self) -> &'a [u8] {
        match self {
            Offsets::U32(offsets) => offsets.as_bytes(),
            Offsets::U64(offsets) => offsets.as_bytes(),
        }
    }
}

// ? ---------------------------------------------------------------------------
// ? Storage
// ? ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    k_size: u64,
    m_size: u64,
    n_buckets: u64,
    n_kmers: u64,
    n_postings: u64,
    posting_width: u8,
    offset_width: u8,
}

impl Shape {
    fn buckets_len(&self) -> Option<u64> {
        self.n_buckets.checked_add(1)?.checked_mul(16)
    }

    fn kmers_len(&self) -> Option<u64> {
        self.n_kmers.checked_mul(8)
    }

    fn offsets_len(&self) -> Option<u64> {
        self.n_kmers
            .checked_add(1)?
            .checked_mul(self.offset_width as u64)
    }

    fn postings_len(&self) -> Option<u64> {
        self.n_postings.checked_mul(self.posting_width as u64)
    }
}

enum OwnedOffsets {
    U32(Vec<u32>),
    U64(Vec<u64>),
}

impl OwnedOffsets {
    fn push(&mut self, offset: u64) {
        match self {
            OwnedOffsets::U32(offsets) => offsets.push(offset as u32),
            OwnedOffsets::U64(offsets) => offsets.push(offset),
        }
    }
}

enum OwnedPostings {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl OwnedPostings {
    fn extend(&mut self, ids: &[u64]) {
        match self {
            OwnedPostings::U16(postings) => {
                postings.extend(ids.iter().map(|id| *id as u16))
            }
            OwnedPostings::U32(postings) => {
                postings.extend(ids.iter().map(|id| *id as u32))
            }
        }
    }
}

struct Owned {
    /// `(key, start)` pairs, flattened, plus the sentinel `(0, n_kmers)`.
    buckets: Vec<u64>,
    kmers: Vec<u64>,
    offsets: OwnedOffsets,
    postings: OwnedPostings,
}

struct Mapped {
    mmap: Mmap,
    buckets: Range<usize>,
    kmers: Range<usize>,
    offsets: Range<usize>,
    postings: Range<usize>,
}

enum Storage {
    Owned(Owned),
    Mapped(Mapped),
}

/// The read-only k-mer index placement queries.
pub struct KmerIndex {
    shape: Shape,
    storage: Storage,
}

impl fmt::Debug for KmerIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KmerIndex")
            .field("k_size", &self.shape.k_size)
            .field("m_size", &self.shape.m_size)
            .field("n_buckets", &self.shape.n_buckets)
            .field("n_kmers", &self.shape.n_kmers)
            .field("n_postings", &self.shape.n_postings)
            .field(
                "storage",
                &match self.storage {
                    Storage::Owned(_) => "owned",
                    Storage::Mapped(_) => "mapped",
                },
            )
            .finish()
    }
}

/// Borrowed views of the index arrays.
#[derive(Clone, Copy)]
pub struct IndexView<'a> {
    shape: Shape,
    buckets: &'a [u64],
    kmers: &'a [u64],
    offsets: Offsets<'a>,
    postings: PostingList<'a>,
}

fn cast<T: FromBytes>(bytes: &[u8]) -> &[T] {
    T::slice_from(bytes).expect("Section validated when the file was opened")
}

impl<'a> IndexView<'a> {
    fn bucket_at(&self, index: usize) -> Result<Range<usize>, DatabaseError> {
        let start = self.buckets[2 * index + 1];
        let end = self.buckets[2 * index + 3];

        if start > end || end > self.shape.n_kmers {
            return Err(corrupted(format!("bucket {index} is out of range")));
        }

        Ok(start as usize..end as usize)
    }

    /// The k-mer range of the bucket with `key`, if any.
    pub fn bucket(
        &self,
        key: u64,
    ) -> Result<Option<Range<usize>>, DatabaseError> {
        let (mut low, mut high) = (0, self.shape.n_buckets as usize);

        while low < high {
            let mid = low + (high - low) / 2;
            let current = self.buckets[2 * mid];

            if current < key {
                low = mid + 1;
            } else if current > key {
                high = mid;
            } else {
                return self.bucket_at(mid).map(Some);
            }
        }

        Ok(None)
    }

    fn postings_at(
        &self,
        kmer: usize,
    ) -> Result<PostingList<'a>, DatabaseError> {
        let low = self.offsets.get(kmer);
        let high = self.offsets.get(kmer + 1);

        if low > high || high > self.shape.n_postings {
            return Err(corrupted(format!(
                "posting list of k-mer {kmer} is out of range"
            )));
        }

        Ok(self.postings.slice(low as usize..high as usize))
    }

    /// The posting list of the k-mer `hash` in the bucket `key`.
    pub fn lookup(
        &self,
        key: u64,
        hash: u64,
    ) -> Result<Option<PostingList<'a>>, DatabaseError> {
        let Some(range) = self.bucket(key)? else {
            return Ok(None);
        };

        let start = range.start;

        match self.kmers[range].binary_search(&hash) {
            Err(_) => Ok(None),
            Ok(position) => self.postings_at(start + position).map(Some),
        }
    }
}

// ? ---------------------------------------------------------------------------
// ? Build, convert and query
// ? ---------------------------------------------------------------------------

impl KmerIndex {
    pub fn k_size(&self) -> u64 {
        self.shape.k_size
    }

    pub fn m_size(&self) -> u64 {
        self.shape.m_size
    }

    pub fn n_kmers(&self) -> u64 {
        self.shape.n_kmers
    }

    pub fn n_buckets(&self) -> u64 {
        self.shape.n_buckets
    }

    pub fn n_postings(&self) -> u64 {
        self.shape.n_postings
    }

    pub fn view(&self) -> IndexView<'_> {
        match &self.storage {
            Storage::Owned(owned) => IndexView {
                shape: self.shape,
                buckets: &owned.buckets,
                kmers: &owned.kmers,
                offsets: match &owned.offsets {
                    OwnedOffsets::U32(offsets) => Offsets::U32(offsets),
                    OwnedOffsets::U64(offsets) => Offsets::U64(offsets),
                },
                postings: match &owned.postings {
                    OwnedPostings::U16(postings) => PostingList::U16(postings),
                    OwnedPostings::U32(postings) => PostingList::U32(postings),
                },
            },
            Storage::Mapped(mapped) => {
                let bytes = &mapped.mmap[..];

                IndexView {
                    shape: self.shape,
                    buckets: cast(&bytes[mapped.buckets.clone()]),
                    kmers: cast(&bytes[mapped.kmers.clone()]),
                    offsets: match self.shape.offset_width {
                        4 => Offsets::U32(cast(&bytes[mapped.offsets.clone()])),
                        _ => Offsets::U64(cast(&bytes[mapped.offsets.clone()])),
                    },
                    postings: match self.shape.posting_width {
                        2 => PostingList::U16(cast(
                            &bytes[mapped.postings.clone()],
                        )),
                        _ => PostingList::U32(cast(
                            &bytes[mapped.postings.clone()],
                        )),
                    },
                }
            }
        }
    }

    /// The number of k-mers in each bucket, in key order.
    pub fn bucket_sizes(&self) -> Result<Vec<u64>, DatabaseError> {
        let view = self.view();

        (0..self.shape.n_buckets as usize)
            .map(|index| view.bucket_at(index).map(|range| range.len() as u64))
            .collect()
    }

    /// The `(bucket key, hash)` pairs of the query k-mers on both strands.
    pub fn query_kmers(&self, sequence: &str) -> Vec<(u64, u64)> {
        query_kmers(sequence, self.shape.k_size, self.shape.m_size)
    }

    /// Build an owned index from a `KmersMap`. The same map always gives
    /// the same arrays: keys, hashes and node ids are sorted.
    pub fn freeze(map: &KmersMap) -> Self {
        Self::freeze_with_widths(map, None)
    }

    fn freeze_with_widths(map: &KmersMap, widths: Option<(u8, u8)>) -> Self {
        let mut keys = map.get_map().keys().collect::<Vec<&MinimizerKey>>();
        keys.sort_unstable_by_key(|key| key.0);

        let mut n_kmers = 0u64;
        let mut n_postings = 0u64;
        let mut max_node = 0u64;

        for value in map.get_map().values() {
            n_kmers += value.0.len() as u64;
            for nodes in value.0.values() {
                n_postings += nodes.len() as u64;
                max_node = nodes.iter().copied().fold(max_node, u64::max);
            }
        }

        assert!(
            max_node <= u32::MAX as u64,
            "Node ids above {} are not supported",
            u32::MAX
        );

        let (posting_width, offset_width) = widths.unwrap_or((
            if max_node < 1 << 16 { 2 } else { 4 },
            if n_postings < 1 << 32 { 4 } else { 8 },
        ));

        let mut buckets = Vec::with_capacity(2 * (keys.len() + 1));
        let mut kmers = Vec::with_capacity(n_kmers as usize);

        let mut offsets = match offset_width {
            4 => OwnedOffsets::U32(Vec::with_capacity(n_kmers as usize + 1)),
            _ => OwnedOffsets::U64(Vec::with_capacity(n_kmers as usize + 1)),
        };

        let mut postings = match posting_width {
            2 => OwnedPostings::U16(Vec::with_capacity(n_postings as usize)),
            _ => OwnedPostings::U32(Vec::with_capacity(n_postings as usize)),
        };

        let mut written = 0u64;
        let mut ids = Vec::new();

        offsets.push(0);

        for key in keys {
            buckets.extend([key.0, kmers.len() as u64]);

            let value = &map.get_map()[key];
            let mut entries = value.0.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(hash, _)| **hash);

            for (hash, nodes) in entries {
                ids.clear();
                ids.extend(nodes.iter().copied());
                ids.sort_unstable();

                kmers.push(*hash);
                postings.extend(&ids);
                written += ids.len() as u64;
                offsets.push(written);
            }
        }

        buckets.extend([0, kmers.len() as u64]);

        KmerIndex {
            shape: Shape {
                k_size: map.get_kmer_size(),
                m_size: map.get_minimizer_size(),
                n_buckets: (buckets.len() / 2 - 1) as u64,
                n_kmers,
                n_postings,
                posting_width,
                offset_width,
            },
            storage: Storage::Owned(Owned {
                buckets,
                kmers,
                offsets,
                postings,
            }),
        }
    }

    /// Rebuild the `KmersMap` the index was frozen from. The index keeps
    /// every bucket key, hash and node id, so the round trip is exact.
    pub fn thaw(&self) -> Result<KmersMap, DatabaseError> {
        let view = self.view();
        let mut map = HashMap::with_capacity(self.shape.n_buckets as usize);

        for index in 0..self.shape.n_buckets as usize {
            let range = view.bucket_at(index)?;
            let mut value = HashMap::with_capacity(range.len());

            for kmer in range {
                value.insert(
                    view.kmers[kmer],
                    view.postings_at(kmer)?
                        .to_vec()
                        .into_iter()
                        .collect::<HashSet<u64>>(),
                );
            }

            map.insert(
                MinimizerKey(view.buckets[2 * index]),
                MinimizerValue(value),
            );
        }

        Ok(KmersMap::from_parts(
            self.shape.k_size,
            self.shape.m_size,
            map,
        ))
    }
}

// ? ---------------------------------------------------------------------------
// ? The v2 file
// ? ---------------------------------------------------------------------------

/// The tree as stored in the v2 file: every `Tree` field except the k-mers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TreeSection<'a> {
    id: &'a Uuid,
    name: &'a str,
    min_branch_support: f64,
    root: &'a Clade,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<&'a Vec<Annotation>>,
}

fn align8(value: u64) -> u64 {
    (value + 7) & !7
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

fn section(
    file_len: usize,
    offset: u64,
    len: Option<u64>,
    name: &str,
) -> Result<Range<usize>, DatabaseError> {
    if !offset.is_multiple_of(8) {
        return Err(corrupted(format!("the {name} section is misaligned")));
    }

    match len.and_then(|len| offset.checked_add(len)) {
        Some(end) if end <= file_len as u64 && offset >= HEADER_LEN as u64 => {
            Ok(offset as usize..end as usize)
        }
        _ => Err(corrupted(format!("the {name} section is out of bounds"))),
    }
}

impl KmerIndex {
    /// Write `tree` and the index as a v2 database. The output depends only
    /// on the tree and the index content.
    pub fn write<W: Write>(
        &self,
        tree: &Tree,
        mut writer: W,
    ) -> Result<(), DatabaseError> {
        let tree_json = serde_json::to_vec(&TreeSection {
            id: &tree.id,
            name: &tree.name,
            min_branch_support: tree.min_branch_support,
            root: &tree.root,
            annotations: tree.annotations.as_ref(),
        })?;

        let shape = self.shape;
        let view = self.view();
        let overflow = || corrupted("the index is too large");

        let tree_offset = HEADER_LEN as u64;
        let buckets_offset = align8(tree_offset + tree_json.len() as u64);
        let kmers_offset =
            buckets_offset + shape.buckets_len().ok_or_else(overflow)?;
        let offsets_offset =
            kmers_offset + shape.kmers_len().ok_or_else(overflow)?;
        let postings_offset =
            align8(offsets_offset + shape.offsets_len().ok_or_else(overflow)?);
        let end =
            postings_offset + shape.postings_len().ok_or_else(overflow)?;

        let mut header = [0u8; HEADER_LEN];
        header[0..8].copy_from_slice(&MAGIC);
        header[8..12].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        header[12] = shape.posting_width;
        header[13] = shape.offset_width;

        for (at, value) in [
            (16, shape.k_size),
            (24, shape.m_size),
            (32, shape.n_buckets),
            (40, shape.n_kmers),
            (48, shape.n_postings),
            (56, tree_offset),
            (64, tree_json.len() as u64),
            (72, buckets_offset),
            (80, kmers_offset),
            (88, offsets_offset),
            (96, postings_offset),
        ] {
            header[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }

        header[104..108]
            .copy_from_slice(&crc32fast::hash(&tree_json).to_le_bytes());

        let padding = [0u8; 8];
        let pad = |from: u64, to: u64| &padding[..(to - from) as usize];

        writer.write_all(&header)?;
        writer.write_all(&tree_json)?;
        writer.write_all(pad(
            tree_offset + tree_json.len() as u64,
            buckets_offset,
        ))?;
        writer.write_all(view.buckets.as_bytes())?;
        writer.write_all(view.kmers.as_bytes())?;
        writer.write_all(view.offsets.as_bytes())?;
        writer.write_all(pad(
            offsets_offset + shape.offsets_len().unwrap(),
            postings_offset,
        ))?;
        writer.write_all(view.postings.as_bytes())?;
        writer.write_all(pad(end, align8(end)))?;
        writer.flush()?;

        Ok(())
    }

    /// Open a v2 database. The file is mapped, not read: only the header
    /// and the tree are parsed, and every lookup is bounds-checked.
    pub fn open(path: &Path) -> Result<(Tree, KmerIndex), DatabaseError> {
        let file = File::open(path)?;

        // SAFETY: the database is read-only input. As with BLAST and Kraken 2
        // databases, the file must not be modified while `cls` runs. Every
        // read below is bounds-checked, so a modified file gives wrong
        // results or an error, never an out-of-range read.
        let mmap = unsafe { Mmap::map(&file)? };

        Self::from_mmap(mmap)
    }

    fn from_mmap(mmap: Mmap) -> Result<(Tree, KmerIndex), DatabaseError> {
        let bytes = &mmap[..];

        if bytes.len() < 8 || bytes[..8] != MAGIC {
            return Err(DatabaseError::BadMagic);
        }

        if bytes.len() < HEADER_LEN {
            return Err(corrupted("the header is truncated"));
        }

        let version = read_u32(bytes, 8);
        if version != FORMAT_VERSION {
            return Err(DatabaseError::UnsupportedVersion(version));
        }

        let shape = Shape {
            k_size: read_u64(bytes, 16),
            m_size: read_u64(bytes, 24),
            n_buckets: read_u64(bytes, 32),
            n_kmers: read_u64(bytes, 40),
            n_postings: read_u64(bytes, 48),
            posting_width: bytes[12],
            offset_width: bytes[13],
        };

        if !matches!(shape.posting_width, 2 | 4) {
            return Err(corrupted(format!(
                "unknown posting width {}",
                shape.posting_width
            )));
        }

        if !matches!(shape.offset_width, 4 | 8) {
            return Err(corrupted(format!(
                "unknown offset width {}",
                shape.offset_width
            )));
        }

        let len = bytes.len();
        let tree = section(
            len,
            read_u64(bytes, 56),
            Some(read_u64(bytes, 64)),
            "tree",
        )?;
        let buckets =
            section(len, read_u64(bytes, 72), shape.buckets_len(), "buckets")?;
        let kmers =
            section(len, read_u64(bytes, 80), shape.kmers_len(), "kmers")?;
        let offsets = section(
            len,
            read_u64(bytes, 88),
            shape.offsets_len(),
            "posting offsets",
        )?;
        let postings = section(
            len,
            read_u64(bytes, 96),
            shape.postings_len(),
            "postings",
        )?;

        let tree_bytes = &bytes[tree];
        if crc32fast::hash(tree_bytes) != read_u32(bytes, 104) {
            return Err(corrupted("the tree checksum does not match"));
        }

        let tree: Tree = serde_json::from_slice(tree_bytes)?;

        let index = KmerIndex {
            shape,
            storage: Storage::Mapped(Mapped {
                mmap,
                buckets,
                kmers,
                offsets,
                postings,
            }),
        };

        let view = index.view();
        let last_bucket = 2 * shape.n_buckets as usize;

        if view.buckets[1] != 0
            || view.buckets[last_bucket + 1] != shape.n_kmers
        {
            return Err(corrupted("the bucket table does not end at n_kmers"));
        }

        if view.offsets.get(0) != 0
            || view.offsets.get(shape.n_kmers as usize) != shape.n_postings
        {
            return Err(corrupted(
                "the posting offsets do not end at n_postings",
            ));
        }

        Ok((tree, index))
    }
}

// ? ---------------------------------------------------------------------------
// ? Matches of one query
// ? ---------------------------------------------------------------------------

/// The posting lists of the distinct query k-mers found in the index.
pub struct MatchSet<'a> {
    lists: Vec<PostingList<'a>>,
}

impl<'a> MatchSet<'a> {
    /// Look up each distinct `(bucket key, hash)` pair once: one search in
    /// the bucket table per distinct key, then one inside the bucket.
    pub fn find(
        view: &IndexView<'a>,
        query: &[(u64, u64)],
    ) -> Result<Self, DatabaseError> {
        let mut pairs = query.to_vec();
        pairs.sort_unstable();
        pairs.dedup();

        let mut lists = Vec::new();

        for group in pairs.chunk_by(|a, b| a.0 == b.0) {
            let Some(range) = view.bucket(group[0].0)? else {
                continue;
            };

            let hashes = &view.kmers[range.clone()];

            for (_, hash) in group {
                if let Ok(position) = hashes.binary_search(hash) {
                    lists.push(view.postings_at(range.start + position)?);
                }
            }
        }

        Ok(MatchSet { lists })
    }

    pub fn len(&self) -> usize {
        self.lists.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    /// Keep only the matches indexed under `node`.
    pub fn retain_containing(&mut self, node: u64) {
        self.lists.retain(|list| list.contains(node));
    }

    /// The score of each child: the number of matches indexed under it or,
    /// with `remove_intersection`, under it and under no sibling.
    pub fn scores(
        &self,
        children: &[u64],
        remove_intersection: bool,
    ) -> Vec<usize> {
        let mut scores = vec![0; children.len()];
        let mut hits = Vec::with_capacity(children.len());

        for list in &self.lists {
            hits.clear();
            hits.extend(
                (0..children.len())
                    .filter(|child| list.contains(children[*child])),
            );

            if !remove_intersection {
                hits.iter().for_each(|child| scores[*child] += 1);
            } else if let [child] = hits[..] {
                scores[child] += 1;
            }
        }

        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{synth, use_cases::map_kmers_to_tree};
    use std::collections::{BTreeMap, BTreeSet};
    use tempfile::TempDir;

    fn flatten(map: &KmersMap) -> BTreeMap<(u64, u64), BTreeSet<u64>> {
        map.get_map()
            .iter()
            .flat_map(|(key, value)| {
                value.0.iter().map(move |(hash, nodes)| {
                    ((key.0, *hash), nodes.iter().copied().collect())
                })
            })
            .collect()
    }

    fn synth_tree(
        leaves: usize,
        k: u64,
        m: u64,
        seed: u64,
    ) -> (Tree, Vec<(String, String)>) {
        let topology = synth::yule(leaves, seed);
        let records = synth::evolve(&topology, 600, seed);
        let dir = TempDir::new().unwrap();
        let tree_path = dir.path().join("tree.nwk");
        let msa_path = dir.path().join("msa.fasta");
        std::fs::write(&tree_path, synth::write_newick(&topology)).unwrap();
        std::fs::write(
            &msa_path,
            synth::write_fasta(&records, 60, false, false),
        )
        .unwrap();
        let tree =
            map_kmers_to_tree(tree_path, msa_path, Some(k), Some(m), None)
                .unwrap();
        (tree, records)
    }

    fn write_to_vec(index: &KmerIndex, tree: &Tree) -> Vec<u8> {
        let mut bytes = Vec::new();
        index.write(tree, &mut bytes).unwrap();
        bytes
    }

    fn open_bytes(bytes: &[u8]) -> Result<(Tree, KmerIndex), DatabaseError> {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("db.cls");
        std::fs::write(&path, bytes).unwrap();
        KmerIndex::open(&path)
    }

    fn tree_json(tree: &Tree) -> serde_json::Value {
        serde_json::to_value(TreeSection {
            id: &tree.id,
            name: &tree.name,
            min_branch_support: tree.min_branch_support,
            root: &tree.root,
            annotations: tree.annotations.as_ref(),
        })
        .unwrap()
    }

    // ? -----------------------------------------------------------------------
    // ? Query k-mers
    // ? -----------------------------------------------------------------------

    #[test]
    fn murmur_of_empty_input_is_zero() {
        // `m = 0` indexes under key 0 and queries the hash of the empty
        // prefix: both must be the same bucket.
        assert_eq!(hash_bytes(b""), 0);
    }

    #[test]
    fn query_kmers_equal_the_legacy_string_kmers() {
        for (seed, len) in [(1u64, 10usize), (2, 35), (3, 36), (4, 500)] {
            let upper = synth::random_seq(len, seed);
            let mixed = upper
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 3 == 0 {
                        c.to_ascii_lowercase()
                    } else {
                        c
                    }
                })
                .collect::<String>();

            for sequence in [upper, mixed] {
                for (k, m) in [(5u64, 0u64), (5, 2), (15, 4), (35, 4), (4, 9)] {
                    let legacy = KmersMap::new(k, m)
                        .build_kmer_from_string(sequence.clone(), None)
                        .into_iter()
                        .map(|(kmer, hash)| {
                            let prefix = kmer
                                .chars()
                                .take(m as usize)
                                .collect::<String>();
                            let key = if m == 0 {
                                0
                            } else {
                                hash_bytes(prefix.as_bytes())
                            };
                            (key, hash)
                        })
                        .collect::<Vec<_>>();

                    assert_eq!(
                        query_kmers(&sequence, k, m),
                        legacy,
                        "k={k} m={m}"
                    );
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "Invalid character in sequence")]
    fn query_kmers_panic_on_non_acgt() {
        query_kmers("ACGTNACGT", 3, 1);
    }

    // ? -----------------------------------------------------------------------
    // ? Lookup oracle: the index gives the legacy counts
    // ? -----------------------------------------------------------------------

    /// The legacy per-child scores of `place_sequence` before 005.
    fn legacy_scores(
        introspection: &KmersMap,
        children: &[u64],
        remove_intersection: bool,
    ) -> Vec<usize> {
        let sets = children
            .iter()
            .map(|id| {
                introspection
                    .get_hashed_kmers_with_node(*id)
                    .unwrap_or_default()
            })
            .collect::<Vec<HashSet<u64>>>();

        sets.iter()
            .enumerate()
            .map(|(index, kmers)| {
                if !remove_intersection {
                    return kmers.len();
                }
                kmers
                    .iter()
                    .filter(|kmer| {
                        !sets.iter().enumerate().any(|(other, set)| {
                            other != index && set.contains(kmer)
                        })
                    })
                    .count()
            })
            .collect()
    }

    fn internal_nodes(clade: &Clade, out: &mut Vec<Vec<u64>>) {
        if let Some(children) = &clade.children {
            out.push(children.iter().map(|child| child.id).collect());
            children.iter().for_each(|child| internal_nodes(child, out));
        }
    }

    #[test]
    fn match_set_counts_and_scores_equal_the_legacy_map() {
        for (m, seed) in [(4u64, 11u64), (0, 12), (2, 13)] {
            let (tree, records) = synth_tree(30, 15, m, seed);
            let map = tree.kmers_map.as_ref().unwrap();
            let index = KmerIndex::freeze(map);
            let view = index.view();

            let mut levels = Vec::new();
            internal_nodes(&tree.root, &mut levels);

            let mut queries = synth::queries(&records, 40, seed + 100);
            queries.push(("whole".into(), records[0].1.clone()));

            for (name, query) in queries {
                let legacy_kmers =
                    map.build_kmer_from_string(query.clone(), None);
                let overlap = map.get_overlapping_hashed_kmers(legacy_kmers);
                let legacy_matches = overlap
                    .get_map()
                    .values()
                    .map(|v| v.0.len())
                    .sum::<usize>();

                let mut matches =
                    MatchSet::find(&view, &index.query_kmers(&query)).unwrap();
                assert_eq!(
                    matches.len(),
                    legacy_matches,
                    "{name}: tree matches"
                );

                let Some(rooted) =
                    overlap.get_minimized_hashes_with_node(tree.root.id)
                else {
                    matches.retain_containing(tree.root.id);
                    assert!(matches.is_empty(), "{name}");
                    continue;
                };
                let introspection =
                    overlap.get_overlapping_minimized_hashes(rooted);

                matches.retain_containing(tree.root.id);
                assert_eq!(
                    matches.len(),
                    introspection
                        .get_map()
                        .values()
                        .map(|v| v.0.len())
                        .sum::<usize>(),
                    "{name}: root matches"
                );

                for children in &levels {
                    for remove_intersection in [false, true] {
                        assert_eq!(
                            matches.scores(children, remove_intersection),
                            legacy_scores(&introspection, children, remove_intersection),
                            "{name}: children {children:?} ri={remove_intersection}"
                        );
                    }
                }
            }
        }
    }

    // ? -----------------------------------------------------------------------
    // ? File format
    // ? -----------------------------------------------------------------------

    #[test]
    fn freeze_write_open_thaw_round_trips() {
        let (tree, _) = synth_tree(20, 11, 4, 21);
        let map = tree.kmers_map.as_ref().unwrap();

        for widths in
            [None, Some((2, 4)), Some((4, 4)), Some((2, 8)), Some((4, 8))]
        {
            let index = KmerIndex::freeze_with_widths(map, widths);
            assert_eq!(
                flatten(&index.thaw().unwrap()),
                flatten(map),
                "{widths:?} owned"
            );

            let (opened_tree, opened) =
                open_bytes(&write_to_vec(&index, &tree)).unwrap();
            assert_eq!(opened.shape, index.shape, "{widths:?}");
            assert_eq!(
                flatten(&opened.thaw().unwrap()),
                flatten(map),
                "{widths:?} mapped"
            );
            assert_eq!(tree_json(&opened_tree), tree_json(&tree));
            assert!(opened_tree.kmers_map.is_none());
            assert_eq!(
                opened.bucket_sizes().unwrap(),
                index.bucket_sizes().unwrap()
            );
        }
    }

    #[test]
    fn default_widths_follow_the_counts() {
        let (tree, _) = synth_tree(10, 11, 4, 22);
        let index = KmerIndex::freeze(tree.kmers_map.as_ref().unwrap());
        assert_eq!(
            (index.shape.posting_width, index.shape.offset_width),
            (2, 4)
        );

        let mut map = KmersMap::new(5, 1);
        map.insert_or_append_kmer_hash("ACGTA".into(), 7, [0, 70_000].into());
        let index = KmerIndex::freeze(&map);
        assert_eq!(index.shape.posting_width, 4);
        assert!(index
            .view()
            .lookup(hash_bytes(b"A"), 7)
            .unwrap()
            .unwrap()
            .contains(70_000));
    }

    #[test]
    fn writes_are_deterministic() {
        let (tree, _) = synth_tree(15, 11, 4, 23);
        let map = tree.kmers_map.as_ref().unwrap();
        let first = write_to_vec(&KmerIndex::freeze(map), &tree);
        let second = write_to_vec(&KmerIndex::freeze(&map.clone()), &tree);
        assert_eq!(first, second);

        // A mapped index writes the same bytes it was read from.
        let (opened_tree, opened) = open_bytes(&first).unwrap();
        assert_eq!(write_to_vec(&opened, &opened_tree), first);
        assert_eq!(first.len() % 8, 0);
    }

    #[test]
    fn empty_index_round_trips() {
        let (tree, _) = synth_tree(4, 11, 4, 24);
        let map = KmersMap::new(11, 4);
        let (_, opened) =
            open_bytes(&write_to_vec(&KmerIndex::freeze(&map), &tree)).unwrap();
        assert_eq!(opened.n_kmers(), 0);
        assert!(opened.view().lookup(0, 0).unwrap().is_none());
    }

    // ? -----------------------------------------------------------------------
    // ? Corruption (FMT-03): typed errors, never a panic
    // ? -----------------------------------------------------------------------

    fn valid_file() -> Vec<u8> {
        let (tree, _) = synth_tree(8, 11, 4, 25);
        write_to_vec(
            &KmerIndex::freeze(tree.kmers_map.as_ref().unwrap()),
            &tree,
        )
    }

    fn set_u64(bytes: &mut [u8], at: usize, value: u64) {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn corrupted_message(bytes: &[u8]) -> String {
        match open_bytes(bytes) {
            Err(DatabaseError::Corrupted(msg)) => msg,
            other => panic!(
                "expected Corrupted, got {:?}",
                other.map(|(_, index)| index)
            ),
        }
    }

    #[test]
    fn truncated_header_is_corrupted() {
        let bytes = valid_file();
        assert!(corrupted_message(&bytes[..64]).contains("truncated"));
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut bytes = valid_file();
        bytes[0] = b'X';
        assert!(matches!(open_bytes(&bytes), Err(DatabaseError::BadMagic)));
        assert!(matches!(
            open_bytes(b"---\nid: x\n"),
            Err(DatabaseError::BadMagic)
        ));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let mut bytes = valid_file();
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(
            open_bytes(&bytes),
            Err(DatabaseError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn unknown_widths_are_corrupted() {
        let mut bytes = valid_file();
        bytes[12] = 3;
        assert!(corrupted_message(&bytes).contains("posting width"));
        let mut bytes = valid_file();
        bytes[13] = 2;
        assert!(corrupted_message(&bytes).contains("offset width"));
    }

    #[test]
    fn section_out_of_bounds_is_corrupted() {
        let mut bytes = valid_file();
        set_u64(&mut bytes, 40, u64::MAX / 2);
        assert!(corrupted_message(&bytes).contains("out of bounds"));

        let mut bytes = valid_file();
        let len = bytes.len();
        bytes.truncate(len - 64);
        assert!(corrupted_message(&bytes).contains("out of bounds"));
    }

    #[test]
    fn misaligned_section_is_corrupted() {
        let mut bytes = valid_file();
        let kmers = read_u64(&bytes, 80);
        set_u64(&mut bytes, 80, kmers + 4);
        assert!(corrupted_message(&bytes).contains("misaligned"));
    }

    #[test]
    fn bad_tree_checksum_is_corrupted() {
        let mut bytes = valid_file();
        bytes[HEADER_LEN + 3] ^= 0x01;
        assert!(corrupted_message(&bytes).contains("checksum"));
    }

    #[test]
    fn sentinel_mismatch_is_corrupted() {
        let mut bytes = valid_file();
        let buckets = read_u64(&bytes, 72) as usize;
        let n_buckets = read_u64(&bytes, 32) as usize;
        set_u64(&mut bytes, buckets + (2 * n_buckets + 1) * 8, 1);
        assert!(corrupted_message(&bytes).contains("bucket table"));

        let mut bytes = valid_file();
        let offsets = read_u64(&bytes, 88) as usize;
        let n_kmers = read_u64(&bytes, 40) as usize;
        bytes[offsets + n_kmers * 4..offsets + n_kmers * 4 + 4]
            .copy_from_slice(&1u32.to_le_bytes());
        assert!(corrupted_message(&bytes).contains("posting offsets"));
    }

    #[test]
    fn interior_offset_out_of_range_fails_at_lookup() {
        let mut bytes = valid_file();
        let offsets = read_u64(&bytes, 88) as usize;
        bytes[offsets + 4..offsets + 8]
            .copy_from_slice(&u32::MAX.to_le_bytes());

        let (_, index) = open_bytes(&bytes).unwrap();
        let view = index.view();
        let (key, hash) = (view.buckets[0], view.kmers[0]);

        assert!(matches!(
            view.lookup(key, hash),
            Err(DatabaseError::Corrupted(_))
        ));
        assert!(MatchSet::find(&view, &[(key, hash)]).is_err());
        assert!(index.thaw().is_err());

        let mut bytes = valid_file();
        let buckets = read_u64(&bytes, 72) as usize;
        set_u64(&mut bytes, buckets + 8 * 3, u64::MAX);
        let (_, index) = open_bytes(&bytes).unwrap();
        let view = index.view();
        assert!(matches!(
            view.bucket(view.buckets[0]),
            Err(DatabaseError::Corrupted(_))
        ));
        assert!(index.bucket_sizes().is_err());
    }
}
