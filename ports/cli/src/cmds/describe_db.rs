use crate::dtos::output_format::DatabaseDescriptionOutputFormat;

use anyhow::Result;
use clap::Parser;
use classeq_ports_lib::load_database;
use std::{collections::HashMap, path::PathBuf};

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the classeq database
    ///
    /// The file should be in JSON or YAML format.
    #[arg(short, long)]
    pub(super) database_file_path: PathBuf,

    /// Output format
    ///
    /// The format in which the database will be serialized.
    #[arg(long, short = 'f', default_value = "tsv")]
    pub(super) out_format: DatabaseDescriptionOutputFormat,
}

pub(crate) fn describe_database_cmd(args: Arguments) -> Result<()> {
    let tree = load_database(args.database_file_path)?;

    let mut stats = HashMap::new();

    let id = tree.id.to_string().to_owned();
    let name = tree.name.to_owned();
    let min_branch_support = tree.min_branch_support.to_string().to_owned();

    stats.insert("ID", id.to_owned());
    stats.insert("Name", name.to_owned());
    stats.insert("MinBranchSupport", min_branch_support);

    if let Some(index) = &tree.index {
        let sizes = index.bucket_sizes()?;
        let kmers = sizes.iter().sum::<u64>();

        stats.insert("KmerSize", index.k_size().to_string());
        stats.insert("kmerCount", kmers.to_string());
        stats.insert("MinimizerSize", index.m_size().to_string());
        stats.insert("MinimizerCount", sizes.len().to_string());
        stats.insert(
            "MinimizerAvgKmers",
            kmers
                .checked_div(sizes.len() as u64)
                .unwrap_or_default()
                .to_string(),
        );
        stats.insert(
            "LargestMinimizer",
            sizes.iter().max().copied().unwrap_or_default().to_string(),
        );
        stats.insert(
            "SmallestMinimizer",
            sizes.iter().min().copied().unwrap_or_default().to_string(),
        );
    }

    match args.out_format {
        DatabaseDescriptionOutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        DatabaseDescriptionOutputFormat::Yaml => {
            println!("{}", serde_saphyr::to_string(&stats)?);
        }
        DatabaseDescriptionOutputFormat::Tsv => {
            for (k, v) in stats {
                println!("{}\t{}", k, v);
            }
        }
    }

    Ok(())
}
