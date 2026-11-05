#![allow(dead_code)]

const BASES: [u8; 4] = [b'A', b'C', b'G', b'T'];

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

pub enum Topology {
    Leaf {
        name: String,
        length: f64,
    },
    Node {
        children: Vec<Topology>,
        support: Option<f64>,
        length: f64,
    },
}

pub fn leaf(name: &str, length: f64) -> Topology {
    Topology::Leaf {
        name: name.to_string(),
        length,
    }
}

pub fn node(
    children: Vec<Topology>,
    support: Option<f64>,
    length: f64,
) -> Topology {
    Topology::Node {
        children,
        support,
        length,
    }
}

/// `(((A1,A2)99,(B1,B2)99)99,((C1,C2)99,(D1,D2)99)99);` with inter-clade
/// branches of 0.08 and intra-clade branches of 0.01.
pub fn default_topology() -> Topology {
    let clade = |p: &str| {
        node(
            vec![leaf(&format!("{p}1"), 0.01), leaf(&format!("{p}2"), 0.01)],
            Some(99.0),
            0.08,
        )
    };
    node(
        vec![
            node(vec![clade("A"), clade("B")], Some(99.0), 0.08),
            node(vec![clade("C"), clade("D")], Some(99.0), 0.08),
        ],
        None,
        0.0,
    )
}

pub fn random_seq(len: usize, seed: u64) -> String {
    let mut rng = Rng::new(seed);
    (0..len).map(|_| BASES[rng.below(4)] as char).collect()
}

/// Applies `n_subs` substitutions at distinct positions, each to a different
/// base.
pub fn mutate(seq: &str, n_subs: usize, seed: u64) -> String {
    let mut rng = Rng::new(seed);
    let mut bytes = seq.as_bytes().to_vec();
    let mut positions: Vec<usize> = (0..bytes.len()).collect();
    let n = n_subs.min(bytes.len());

    for i in 0..n {
        let j = i + rng.below(positions.len() - i);
        positions.swap(i, j);
        let pos = positions[i];
        let others: Vec<u8> =
            BASES.iter().copied().filter(|b| *b != bytes[pos]).collect();
        bytes[pos] = others[rng.below(others.len())];
    }

    String::from_utf8(bytes).unwrap()
}

pub fn fragment(seq: &str, start: usize, len: usize) -> String {
    seq[start..start + len].to_string()
}

/// First half of `a` followed by the second half of `b`.
pub fn chimera(a: &str, b: &str) -> String {
    format!("{}{}", &a[..a.len() / 2], &b[b.len() / 2..])
}

/// Evolves a random root sequence of length `len` along `topology`. Each edge
/// applies `round(length * len)` substitutions. Returns `(name, sequence)` for
/// every leaf in topology order.
pub fn evolve(
    topology: &Topology,
    len: usize,
    seed: u64,
) -> Vec<(String, String)> {
    let mut rng = Rng::new(seed);
    let root = random_seq(len, rng.next_u64());
    let mut records = Vec::new();
    evolve_from(topology, &root, &mut rng, &mut records);
    records
}

fn evolve_from(
    topology: &Topology,
    parent: &str,
    rng: &mut Rng,
    records: &mut Vec<(String, String)>,
) {
    let length = match topology {
        Topology::Leaf { length, .. } | Topology::Node { length, .. } => {
            *length
        }
    };
    let n_subs = (length * parent.len() as f64).round() as usize;
    let seq = mutate(parent, n_subs, rng.next_u64());

    match topology {
        Topology::Leaf { name, .. } => records.push((name.clone(), seq)),
        Topology::Node { children, .. } => {
            for child in children {
                evolve_from(child, &seq, rng, records);
            }
        }
    }
}

/// Rooted newick with supports and branch lengths, e.g.
/// `(((A1:0.01,A2:0.01)99:0.08,...)99:0.08,...);`.
pub fn write_newick(topology: &Topology) -> String {
    fn fmt(t: &Topology, is_root: bool) -> String {
        match t {
            Topology::Leaf { name, length } => format!("{name}:{length}"),
            Topology::Node {
                children,
                support,
                length,
            } => {
                let inner = children
                    .iter()
                    .map(|c| fmt(c, false))
                    .collect::<Vec<_>>()
                    .join(",");
                let support =
                    support.map(|s| s.to_string()).unwrap_or_default();
                if is_root {
                    format!("({inner}){support}")
                } else {
                    format!("({inner}){support}:{length}")
                }
            }
        }
    }
    format!("{};", fmt(topology, true))
}

/// FASTA text. `line_width == 0` writes each sequence on one line. `gaps`
/// inserts a `-` after every 37th base. `crlf` uses `\r\n` line endings.
pub fn write_fasta(
    records: &[(String, String)],
    line_width: usize,
    gaps: bool,
    crlf: bool,
) -> String {
    let eol = if crlf { "\r\n" } else { "\n" };
    let mut out = String::new();

    for (name, seq) in records {
        let body: String = if gaps {
            seq.chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    if (i + 1) % 37 == 0 {
                        vec![c, '-']
                    } else {
                        vec![c]
                    }
                })
                .collect()
        } else {
            seq.clone()
        };

        out.push('>');
        out.push_str(name);
        out.push_str(eol);

        let width = if line_width == 0 {
            body.len().max(1)
        } else {
            line_width
        };
        for chunk in body.as_bytes().chunks(width) {
            out.push_str(std::str::from_utf8(chunk).unwrap());
            out.push_str(eol);
        }
    }

    out
}
