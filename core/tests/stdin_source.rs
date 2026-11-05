//! Uses `"-"` (stdin), which flips the process-global `STDIN_HAS_BEEN_USED`
//! flag. Kept in its own test binary and in a single test so the order of the
//! calls is deterministic.

use classeq_core::domain::dtos::file_or_stdin::{
    FileOrStdin, Source, StdinError,
};
use std::str::FromStr;

#[test]
fn dash_maps_to_stdin_once_then_errors_on_repeated_use() {
    let first = Source::from_str("-").unwrap();
    assert!(matches!(first, Source::Stdin));
    assert_eq!(format!("{first:?}"), "stdin");

    let second = Source::from_str("-").unwrap_err();
    assert!(matches!(second, StdinError::StdInRepeatedUse));
    assert_eq!(second.to_string(), "stdin argument used more than once");

    let third = FileOrStdin::<String>::from_str("-").unwrap_err();
    assert!(matches!(third, StdinError::StdInRepeatedUse));

    assert!(matches!(
        Source::from_str("file.fasta").unwrap(),
        Source::Arg(ref p) if p == "file.fasta"
    ));
}
