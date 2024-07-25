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

    if let Some(size) = tree.get_in_memory_size() {
        let binding = size.to_string();
        let splitted = binding.split_whitespace().collect::<Vec<&str>>();
        stats.insert(
            "InMemorySizeMb",
            splitted.first().unwrap_or(&"0").to_string(),
        );
    }

    if let Some(kmers_map) = &tree.kmers_map {
        let minimized_kmers =
            kmers_map.get_map().into_iter().map(|(_, v)| v.0.len());

        stats.insert("KmerSize", kmers_map.get_kmer_size().to_string());
        stats.insert(
            "kmerCount",
            minimized_kmers.to_owned().sum::<usize>().to_string(),
        );
        stats.insert(
            "kmerAvgKmers",
            (minimized_kmers.to_owned().sum::<usize>()
                / kmers_map.get_map().len())
            .to_string(),
        );

        stats.insert(
            "MinimizerSize",
            kmers_map.get_minimizer_size().to_string(),
        );

        stats.insert("MinimizerCount", kmers_map.get_map().len().to_string());
        stats.insert(
            "MinimizerAvgKmers",
            (minimized_kmers.to_owned().sum::<usize>()
                / kmers_map.get_map().len())
            .to_string(),
        );

        stats.insert(
            "LargestMinimizer",
            minimized_kmers.to_owned().max().unwrap().to_string(),
        );

        stats.insert(
            "SmallestMinimizer",
            minimized_kmers.to_owned().min().unwrap().to_string(),
        );
    }

    match args.out_format {
        DatabaseDescriptionOutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        DatabaseDescriptionOutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&stats)?);
        }
        DatabaseDescriptionOutputFormat::Tsv => {
            for (k, v) in stats {
                println!("{}\t{}", k, v);
            }
        }
    }

    Ok(())
}
