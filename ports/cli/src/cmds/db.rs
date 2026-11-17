use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// The index of the databases published with classeq2.
const DEFAULT_INDEX: &str = "https://github.com/LepistaBioinformatics/classeq2/releases/download/databases/index.json";

/// The index schema this version of `cls` reads.
const SCHEMA: u32 = 1;

/// The roles of the files `fetch` takes without `--all`.
const DEFAULT_ROLES: [&str; 3] = ["database", "annotations", "doc"];

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Ready-made database related commands
    #[clap(subcommand)]
    pub db: Commands,
}

#[derive(Parser, Debug)]
pub(crate) enum Commands {
    /// List the ready-made databases and their versions
    List(ListArguments),

    /// Download a ready-made database
    ///
    /// Each file is checked against the SHA-256 recorded in the index.
    Fetch(FetchArguments),
}

#[derive(Parser, Debug)]
pub(crate) struct ListArguments {
    /// Location of the index
    ///
    /// An `http(s)` address or a local path. A local index is read with the
    /// same layout as the releases: `<root>/databases/index.json` and
    /// `<root>/<tag>/<file>`.
    #[arg(long, default_value = DEFAULT_INDEX)]
    pub(super) index: String,
}

#[derive(Parser, Debug)]
pub(crate) struct FetchArguments {
    /// Name of the database, as `cls db list` prints it
    pub(super) name: String,

    /// Version to download
    ///
    /// Without a value, the latest version.
    #[arg(long)]
    pub(super) version: Option<u32>,

    /// Download every file of the database
    ///
    /// Without this flag only the database, its annotations and its README
    /// are downloaded. The other files are the alignment and the tree the
    /// database was built from, the report of `cls assess` and an example.
    #[arg(long)]
    pub(super) all: bool,

    /// Directory to write into
    ///
    /// The files go to a directory named after the database inside it.
    #[arg(short, long, default_value = ".")]
    pub(super) output_dir: PathBuf,

    /// Overwrite files that already exist
    #[arg(long)]
    pub(super) force: bool,

    /// Location of the index
    ///
    /// An `http(s)` address or a local path. A local index is read with the
    /// same layout as the releases: `<root>/databases/index.json` and
    /// `<root>/<tag>/<file>`.
    #[arg(long, default_value = DEFAULT_INDEX)]
    pub(super) index: String,
}

#[derive(Deserialize, Debug)]
struct Index {
    schema: u32,
    databases: Vec<Database>,
}

#[derive(Deserialize, Debug)]
struct Database {
    name: String,
    title: String,
    versions: Vec<Version>,
}

#[derive(Deserialize, Debug)]
struct Version {
    version: u32,
    tag: String,
    date: String,
    cls: String,
    files: Vec<FileEntry>,
}

#[derive(Deserialize, Debug)]
struct FileEntry {
    name: String,
    role: String,
    size: u64,
    sha256: String,
}

fn is_remote(location: &str) -> bool {
    location.starts_with("http://") || location.starts_with("https://")
}

/// Reads a whole file from an `http(s)` address or from a local path.
fn read(location: &str) -> Result<Vec<u8>> {
    let mut content = Vec::new();

    if is_remote(location) {
        ureq::get(location)
            .call()
            .with_context(|| format!("Could not download {location}"))?
            .into_reader()
            .read_to_end(&mut content)
            .with_context(|| format!("Could not download {location}"))?;
    } else {
        fs::File::open(location)
            .and_then(|mut file| file.read_to_end(&mut content))
            .with_context(|| format!("Could not read {location}"))?;
    }

    Ok(content)
}

fn load_index(location: &str) -> Result<Index> {
    let index: Index = serde_json::from_slice(&read(location)?)
        .with_context(|| format!("{location} is not a database index"))?;

    if index.schema != SCHEMA {
        bail!(
            "The index at {location} has schema {}, and this version of cls \
reads schema {SCHEMA}. Update cls.",
            index.schema
        );
    }

    Ok(index)
}

/// The location of a file of a version: `<root>/<tag>/<name>`, where the index
/// is `<root>/databases/index.json`.
fn file_location(index: &str, tag: &str, name: &str) -> Result<String> {
    let root = index
        .strip_suffix("/databases/index.json")
        .or_else(|| (index == "databases/index.json").then_some("."))
        .ok_or_else(|| {
            anyhow!(
                "The index must be at <root>/databases/index.json, and \
{index} is not"
            )
        })?;

    Ok(format!("{root}/{tag}/{name}"))
}

fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1_000_000 => format!("{:.1} MB", b as f64 / 1e6),
        b if b >= 1_000 => format!("{:.1} kB", b as f64 / 1e3),
        b => format!("{b} B"),
    }
}

fn sha256_hex(content: &[u8]) -> String {
    Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Major and minor numbers of a version string.
fn major_minor(version: &str) -> Option<(&str, &str)> {
    let mut parts = version.split('.');
    Some((parts.next()?, parts.next()?))
}

pub(crate) fn list_databases_cmd(args: ListArguments) -> Result<()> {
    let index = load_index(&args.index)?;

    println!("name\tversion\tlatest\tdate\tcls\tsize\ttitle");

    for database in &index.databases {
        let latest = database.versions.iter().map(|v| v.version).max();
        let mut versions = database.versions.iter().collect::<Vec<_>>();
        versions.sort_by_key(|v| std::cmp::Reverse(v.version));

        for version in versions {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                database.name,
                version.version,
                if Some(version.version) == latest {
                    "yes"
                } else {
                    ""
                },
                version.date,
                version.cls,
                human_size(version.files.iter().map(|f| f.size).sum()),
                database.title,
            );
        }
    }

    Ok(())
}

pub(crate) fn fetch_database_cmd(args: FetchArguments) -> Result<()> {
    let index = load_index(&args.index)?;

    let database = index
        .databases
        .iter()
        .find(|database| database.name == args.name)
        .ok_or_else(|| {
            anyhow!(
                "No database named {}. Available: {}",
                args.name,
                index
                    .databases
                    .iter()
                    .map(|database| database.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    let wanted = match args.version {
        Some(version) => Some(version),
        None => database.versions.iter().map(|v| v.version).max(),
    };

    let version = database
        .versions
        .iter()
        .find(|v| Some(v.version) == wanted)
        .ok_or_else(|| {
            let mut have = database
                .versions
                .iter()
                .map(|v| v.version)
                .collect::<Vec<_>>();
            have.sort_unstable();

            anyhow!(
                "Database {} has no version {}. Available: {}",
                database.name,
                wanted.map(|v| v.to_string()).unwrap_or_default(),
                have.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    let files = version
        .files
        .iter()
        .filter(|file| args.all || DEFAULT_ROLES.contains(&file.role.as_str()))
        .collect::<Vec<_>>();

    for file in &files {
        if Path::new(&file.name).file_name().and_then(|n| n.to_str())
            != Some(file.name.as_str())
        {
            bail!(
                "The index names a file that is not a plain name: {}",
                file.name
            );
        }
    }

    let target = args.output_dir.join(&database.name);

    if !args.force {
        if let Some(file) =
            files.iter().find(|file| target.join(&file.name).exists())
        {
            bail!(
                "{} already exists. Use --force to overwrite it.",
                target.join(&file.name).display()
            );
        }
    }

    // Every file is downloaded and checked before the first one is written,
    // so a failed download leaves nothing behind.
    let mut contents = Vec::with_capacity(files.len());

    for file in &files {
        let location = file_location(&args.index, &version.tag, &file.name)?;
        let content = read(&location)?;
        let found = sha256_hex(&content);

        if found != file.sha256.to_lowercase() {
            bail!(
                "{} failed its checksum: expected {}, found {found}. Nothing \
was written.",
                file.name,
                file.sha256
            );
        }

        contents.push(content);
    }

    fs::create_dir_all(&target)
        .with_context(|| format!("Could not create {}", target.display()))?;

    for (file, content) in files.iter().zip(contents) {
        let path = target.join(&file.name);

        fs::File::create(&path)
            .and_then(|mut out| out.write_all(&content))
            .with_context(|| format!("Could not write {}", path.display()))?;

        println!("{}\t{}", path.display(), human_size(file.size));
    }

    let running = env!("CARGO_PKG_VERSION");

    if major_minor(&version.cls) != major_minor(running) {
        tracing::warn!(
            "Database {} version {} was built with cls {}, and this is cls \
{running}. If it does not load, build it again from its alignment and tree \
(`cls db fetch --all`).",
            database.name,
            version.version,
            version.cls
        );
    }

    Ok(())
}
