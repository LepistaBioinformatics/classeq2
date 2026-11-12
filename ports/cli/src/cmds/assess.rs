use super::root_tree::{rooting_method, RootingMethodArg};
use crate::dtos::output_format::DatabaseDescriptionOutputFormat;

use anyhow::{anyhow, Result};
use clap::Parser;
use classeq_core::{
    domain::dtos::evidence_rule::EvidenceRule,
    use_cases::{assess_reference, AssessOptions, Assessment, OutcomeShares},
};
use std::{fmt::Write as _, path::PathBuf};

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the tree file
    ///
    /// The file should be in Newick format.
    pub(super) tree_file_path: PathBuf,

    /// Path to the msa file
    ///
    /// The file should be in FASTA format.
    pub(super) msa_file_path: PathBuf,

    /// The kmer size
    #[arg(long, short, default_value = "35")]
    pub(super) k_size: Option<u64>,

    /// The minimizer size
    #[arg(long, short, default_value = "4")]
    pub(super) m_size: Option<u64>,

    /// Minimum branch support
    #[arg(short = 's', long, default_value = "70")]
    pub(super) min_branch_support: Option<f64>,

    /// Root the tree in memory if it is not rooted
    #[arg(long, value_enum)]
    pub(super) root: Option<RootingMethodArg>,

    /// Outgroup leaf names for --root outgroup (repeat the flag or separate
    /// names with commas)
    #[arg(long, value_delimiter = ',')]
    pub(super) outgroup: Vec<String>,

    /// The length of the reads cut from each reference sequence
    #[arg(long, default_value = "150")]
    pub(super) read_length: usize,

    /// The number of reads cut from each reference sequence
    #[arg(long, default_value = "5")]
    pub(super) reads_per_leaf: usize,

    /// The fewest specific k-mers the chosen child must hold (as in `place`)
    #[arg(long, default_value = "2")]
    pub(super) min_evidence: u32,

    /// The largest ratio between the specific k-mers of the strongest
    /// sibling and those of the chosen child (as in `place`)
    #[arg(long, default_value = "0.2")]
    pub(super) max_rival_ratio: f64,

    /// The ratio of a clade's calibrated containment that a query must
    /// reach (as in `place`)
    #[arg(long, default_value = "0.25")]
    pub(super) min_containment_ratio: f64,

    /// The most leaves used as queries for each missing clade
    #[arg(long, default_value = "3")]
    pub(super) leaves_per_clade: usize,

    /// Output format
    #[arg(long, short = 'f', default_value = "tsv")]
    pub(super) out_format: DatabaseDescriptionOutputFormat,

    /// Write the report to this file instead of the standard output
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Write one line per simulated query to this TSV file
    #[arg(long)]
    pub(super) per_query: Option<PathBuf>,
}

fn tsv(assessment: &Assessment) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "Leaves\t{}", assessment.leaves);
    let _ = writeln!(out, "KmerSize\t{}", assessment.k_size);
    let _ =
        writeln!(out, "MinBranchSupport\t{}", assessment.min_branch_support);
    let _ = writeln!(out, "ReadLength\t{}", assessment.read_length);

    let mut shares = |label: &str, shares: &OutcomeShares| {
        let _ = writeln!(out, "{label}Verdict\t{:?}", shares.verdict);
        let _ = writeln!(out, "{label}Queries\t{}", shares.queries);

        for (name, value) in [
            ("Exact", shares.exact),
            ("Under", shares.under),
            ("Over", shares.over),
            ("Wrong", shares.wrong),
            ("Refused", shares.refused),
            ("Reachable", shares.reachable),
            ("UnderNoEvidence", shares.under_no_evidence),
            ("UnderContested", shares.under_contested),
        ] {
            let _ = writeln!(out, "{label}{name}\t{value:.4}");
        }
    };

    shares(
        "MissingLeafFullLength",
        &assessment.missing_leaf.full_length,
    );
    shares("MissingLeafReads", &assessment.missing_leaf.reads);
    shares(
        "MissingCladeFullLength",
        &assessment.missing_clade.full_length,
    );
    shares("MissingCladeReads", &assessment.missing_clade.reads);

    out
}

pub(crate) fn assess_cmd(
    args: Arguments,
    threads: Option<usize>,
) -> Result<()> {
    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.unwrap_or(1))
        .build_global()
    {
        panic!("Error creating thread pool: {err}");
    };

    let rooting = rooting_method(args.root, args.outgroup)?;

    let options = AssessOptions {
        read_length: args.read_length,
        reads_per_leaf: args.reads_per_leaf,
        leaves_per_clade: args.leaves_per_clade,
        rule: EvidenceRule {
            min_evidence: args.min_evidence,
            max_rival_ratio: args.max_rival_ratio,
            min_containment_ratio: args.min_containment_ratio,
        },
    };

    let assessment = assess_reference(
        args.tree_file_path,
        args.msa_file_path,
        args.k_size,
        args.m_size,
        args.min_branch_support,
        rooting,
        &options,
    )
    .map_err(|err| anyhow!("{}", err.msg()))?;

    let report = match args.out_format {
        DatabaseDescriptionOutputFormat::Json => {
            serde_json::to_string_pretty(&assessment)? + "\n"
        }
        DatabaseDescriptionOutputFormat::Yaml => {
            serde_saphyr::to_string(&assessment)?
        }
        DatabaseDescriptionOutputFormat::Tsv => tsv(&assessment),
    };

    match args.output_file_path {
        Some(path) => std::fs::write(path, report)?,
        None => print!("{report}"),
    }

    if let Some(path) = args.per_query {
        let mut out = String::from(
            "leaf\tmissing\tmissing_leaves\tread_start\toutcome\tstop\tclade\treachable\n",
        );

        for detail in &assessment.details {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}",
                detail.leaf,
                detail.missing,
                detail.missing_leaves,
                detail
                    .read_start
                    .map_or("full".to_string(), |s| s.to_string()),
                detail.outcome,
                detail.stop,
                detail.clade.map_or("NA".to_string(), |c| c.to_string()),
                detail.reachable,
            );
        }

        std::fs::write(path, out)?;
    }

    Ok(())
}
