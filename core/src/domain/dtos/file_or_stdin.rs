///
/// An special congrats to `thepacketgeek`
/// (https://crates.io/users/thepacketgeek) for the clap-stdin code
/// (https://github.com/thepacketgeek/clap-stdin) used as a base for this
/// implementation.
///
///
use super::sequence::{Sequence, SequenceBody};

use std::io::{self, BufRead};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use thiserror::Error;
static STDIN_HAS_BEEN_USED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
pub enum StdinError {
    #[error("stdin argument used more than once")]
    StdInRepeatedUse,
    #[error(transparent)]
    StdIn(#[from] io::Error),
    #[error("unable to parse from_str: {0}")]
    FromStr(String),
}

/// Source of the value contents will be either from `stdin` or a CLI arg provided value
#[derive(Clone)]
pub enum Source {
    Stdin,
    Arg(String),
}

impl FromStr for Source {
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "-" => {
                if STDIN_HAS_BEEN_USED
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(StdinError::StdInRepeatedUse);
                }
                STDIN_HAS_BEEN_USED
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(Self::Stdin)
            }
            arg => Ok(Self::Arg(arg.to_owned())),
        }
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Stdin => write!(f, "stdin"),
            Source::Arg(v) => v.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileOrStdin<T = String> {
    pub source: Source,
    _type: PhantomData<T>,
}

impl FileOrStdin {
    /// Read content and build a fasta sequence
    ///
    /// Each fasta sequence should be returned through a channel received by the
    /// caller. The channel is used to avoid memory overhead when reading large
    /// files.
    ///
    pub fn sequence_content_by_channel(
        self,
        chan: std::sync::mpsc::Sender<Sequence>,
    ) -> Result<(), StdinError> {
        let reader = self.into_chunked_reader()?;
        let mut header = String::new();
        let mut sequence = String::new();

        for line in reader.lines() {
            let line = line?;

            if line.is_empty() {
                continue;
            }

            if line.starts_with('>') {
                if !header.is_empty() {
                    chan.send(Sequence::new(header.clone(), sequence.clone()))
                        .unwrap();
                    sequence.clear();
                } else if !sequence.is_empty() {
                    return Err(StdinError::FromStr(
                        "unexpected sequence without header".to_owned(),
                    ));
                }

                header = line.replace(">", "");
            } else {
                sequence.push_str(
                    SequenceBody::remove_non_iupac_from_sequence(&line)
                        .as_str(),
                );
            }
        }

        if !header.is_empty() && !sequence.is_empty() {
            chan.send(Sequence::new(header, sequence)).unwrap();
        };

        Ok(())
    }

    /// Read content and build a fasta sequence
    ///
    /// Content should be a multi fasta file. Each fasta record can contain a
    /// fasta header starting with `>` and a sequence of a single line or
    /// multiline sequence.
    ///
    #[deprecated(
        since = "0.3.0",
        note = "Use `sequence_content_by_channel` instead."
    )]
    pub fn sequence_content(self) -> Result<Vec<Sequence>, StdinError> {
        let reader = self.into_chunked_reader()?;

        let mut sequences = Vec::<Sequence>::new();
        let mut header = String::new();
        let mut sequence = String::new();

        for line in reader.lines() {
            let line = line?;

            if line.is_empty() {
                continue;
            }

            if line.starts_with('>') {
                if !header.is_empty() {
                    sequences
                        .push(Sequence::new(header.clone(), sequence.clone()));
                    sequence.clear();
                } else if !sequence.is_empty() {
                    return Err(StdinError::FromStr(
                        "unexpected sequence without header".to_owned(),
                    ));
                }

                header = line.replace(">", "");
            } else {
                sequence.push_str(
                    SequenceBody::remove_non_iupac_from_sequence(&line)
                        .as_str(),
                );
            }
        }

        if !header.is_empty() && !sequence.is_empty() {
            sequences.push(Sequence::new(header, sequence));
        }

        Ok(sequences)
    }

    fn into_chunked_reader(&self) -> Result<impl std::io::BufRead, StdinError> {
        let input: Box<dyn std::io::Read + 'static> = match &self.source {
            Source::Stdin => Box::new(std::io::stdin()),
            Source::Arg(filepath) => {
                let f = std::fs::File::open(filepath)?;
                Box::new(f)
            }
        };

        Ok(std::io::BufReader::new(input))
    }

    pub fn from_file(file: &str) -> Self {
        Self {
            source: Source::Arg(file.to_string()),
            _type: PhantomData,
        }
    }
}

impl<T> FromStr for FileOrStdin<T> {
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = Source::from_str(s)?;
        Ok(Self {
            source,
            _type: PhantomData,
        })
    }
}
