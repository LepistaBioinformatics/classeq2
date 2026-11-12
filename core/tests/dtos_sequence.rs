mod common;

use classeq_core::domain::dtos::{
    file_or_stdin::{FileOrStdin, Source, StdinError},
    sequence::{Sequence, SequenceBody},
};
use common::synth;
use std::str::FromStr;
use tempfile::TempDir;

fn parse(content: &str) -> Result<Vec<(String, String)>, StdinError> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("query.fasta");
    std::fs::write(&path, content).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<Sequence>();
    FileOrStdin::from_file(path.to_str().unwrap())
        .sequence_content_by_channel(tx)?;

    Ok(rx
        .into_iter()
        .map(|s| {
            (
                s.header_content().to_string(),
                s.sequence_content().to_string(),
            )
        })
        .collect())
}

fn records(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(h, s)| (h.to_string(), s.to_string()))
        .collect()
}

// ? SEQ-01 --------------------------------------------------------------------

#[test]
fn remove_non_iupac_upper_cases_maps_u_and_keeps_ambiguity_as_n() {
    // BUG-045: ambiguity codes stay as `N`, so no k-mer joins their
    // neighbours, and RNA `U` reads as `T`. Gaps and other characters go.
    assert_eq!(
        SequenceBody::remove_non_iupac_from_sequence("acgt-NNryACGT.\r 12*u"),
        "ACGTNNNNACGTT"
    );
    assert_eq!(SequenceBody::remove_non_iupac_from_sequence(""), "");
    assert_eq!(SequenceBody::remove_non_iupac_from_sequence("---"), "");
}

#[test]
fn sequence_accessors_and_to_fasta() {
    let seq = Sequence::new("q1 desc", "ACGT");
    assert_eq!(seq.header_content(), "q1 desc");
    assert_eq!(seq.header().header(), "q1 desc");
    assert_eq!(seq.sequence_content(), "ACGT");
    assert_eq!(seq.sequence().seq(), "ACGT");
    assert_eq!(seq.to_fasta(), ">q1 desc\nACGT\n");
}

// ? SEQ-02 --------------------------------------------------------------------

#[test]
fn parser_handles_multiline_records_blank_lines_and_last_record() {
    let parsed =
        parse(">a\nACGT\nacgt\n\n>b desc\nTTTT\n\nGG\n>c\nCCCC").unwrap();

    assert_eq!(
        parsed,
        records(&[("a", "ACGTACGT"), ("b desc", "TTTTGG"), ("c", "CCCC")])
    );
}

#[test]
fn parser_round_trips_synthetic_fasta_with_line_wrapping_and_gaps() {
    let input: Vec<(String, String)> = (0..5)
        .map(|i| (format!("seq{i}"), synth::random_seq(150, i)))
        .collect();

    for (width, gaps) in [(0, false), (60, false), (13, true)] {
        let parsed = parse(&synth::write_fasta(&input, width, gaps, false));
        assert_eq!(parsed.unwrap(), input, "width = {width}, gaps = {gaps}");
    }
}

#[test]
fn parser_errors_on_sequence_before_first_header() {
    let err = parse("ACGT\n>a\nACGT\n").unwrap_err();
    assert!(matches!(err, StdinError::FromStr(_)));
    assert_eq!(
        err.to_string(),
        "unable to parse from_str: unexpected sequence without header"
    );
}

/// BUG-011: a file with sequence lines but no header at all was accepted
/// silently and produced no records.
#[test]
fn parser_errors_on_sequence_without_any_header() {
    let err = parse("ACGT\nACGT\n").unwrap_err();
    assert!(matches!(err, StdinError::FromStr(_)));
}

#[test]
fn parser_returns_no_records_for_empty_file() {
    assert!(parse("").unwrap().is_empty());
    assert!(parse("\n\n").unwrap().is_empty());
}

#[test]
fn parser_errors_on_missing_file() {
    let (tx, _rx) = std::sync::mpsc::channel::<Sequence>();
    let err = FileOrStdin::from_file("/nonexistent/dir/query.fasta")
        .sequence_content_by_channel(tx)
        .unwrap_err();
    assert!(matches!(err, StdinError::StdIn(_)));
}

// ? SEQ-03 --------------------------------------------------------------------

/// A header with an empty body is emitted in the middle of the file but
/// dropped when it is the last record.
#[test]
fn parser_empty_body_records_current_behavior() {
    let parsed = parse(">a\n>b\nACGT\n>c\n").unwrap();
    assert_eq!(parsed, records(&[("a", ""), ("b", "ACGT")]));
}

/// Every `>` in the header line is removed, not only the leading one. The
/// build-db parser does the same.
#[test]
fn parser_strips_every_gt_in_header_current_behavior() {
    let parsed = parse(">a>b c>d\nACGT\n").unwrap();
    assert_eq!(parsed, records(&[("ab cd", "ACGT")]));
}

/// S-10: `BufRead::lines` strips `\r\n`, so headers carry no trailing `\r`.
#[test]
fn parser_accepts_crlf_line_endings() {
    let input: Vec<(String, String)> = (0..3)
        .map(|i| (format!("seq{i} some description"), synth::random_seq(90, i)))
        .collect();

    let parsed = parse(&synth::write_fasta(&input, 40, false, true)).unwrap();
    assert_eq!(parsed, input);
    assert!(parsed
        .iter()
        .all(|(h, s)| !h.contains('\r') && !s.contains('\r')));

    let parsed = parse(">a\r\nAC\r\n\r\nGT\r\n").unwrap();
    assert_eq!(parsed, records(&[("a", "ACGT")]));
}

#[test]
#[allow(deprecated)]
fn deprecated_sequence_content_matches_channel_parser() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("query.fasta");
    std::fs::write(&path, ">a\nAC\nGT\n>b\nTT\n").unwrap();

    let parsed: Vec<(String, String)> =
        FileOrStdin::from_file(path.to_str().unwrap())
            .sequence_content()
            .unwrap()
            .into_iter()
            .map(|s| {
                (
                    s.header_content().to_string(),
                    s.sequence_content().to_string(),
                )
            })
            .collect();

    assert_eq!(parsed, records(&[("a", "ACGT"), ("b", "TT")]));
}

// ? SEQ-04 (file side; stdin is in stdin_source.rs) ----------------------------

#[test]
fn from_str_maps_non_dash_to_file_path() {
    match Source::from_str("reads.fasta").unwrap() {
        Source::Arg(path) => assert_eq!(path, "reads.fasta"),
        Source::Stdin => panic!("expected a file path"),
    }

    let file = FileOrStdin::<String>::from_str("/tmp/x.fa").unwrap();
    assert!(matches!(file.source, Source::Arg(ref p) if p == "/tmp/x.fa"));
    assert_eq!(format!("{:?}", file.source), "\"/tmp/x.fa\"");
}

#[test]
fn from_file_and_from_str_parse_the_same_content() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("query.fasta");
    std::fs::write(&path, ">a\nACGT\n").unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<Sequence>();
    let file: FileOrStdin = path.to_str().unwrap().parse().unwrap();
    file.sequence_content_by_channel(tx).unwrap();

    let parsed: Vec<String> = rx
        .into_iter()
        .map(|s| s.sequence_content().to_string())
        .collect();
    assert_eq!(parsed, ["ACGT"]);
}
