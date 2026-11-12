use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SequenceHeader(String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SequenceBody(String);

impl SequenceHeader {
    pub fn new<T>(header: T) -> Self
    where
        T: Into<String>,
    {
        SequenceHeader(
            header
                .try_into()
                .expect("Error converting header to string"),
        )
    }

    pub fn header(&self) -> &str {
        &self.0
    }
}

impl SequenceBody {
    pub fn new<T>(body: T) -> Self
    where
        T: Into<String>,
    {
        SequenceBody(
            body.try_into().expect("Error converting header to string"),
        )
    }

    pub fn seq(&self) -> &str {
        &self.0
    }

    /// Remove non-IUPAC characters from a sequence
    ///
    /// Returns the sequence in upper case with only `A`, `C`, `G`, `T` and
    /// `N`. RNA `U` reads as `T`. Every IUPAC ambiguity code becomes `N`,
    /// which stays in place so that no k-mer joins its neighbours: the k-mer
    /// builders skip the windows that hold an `N`. Gaps and any other
    /// character are removed.
    pub fn remove_non_iupac_from_sequence(sequence: &str) -> String {
        sequence
            .to_uppercase()
            .chars()
            .filter_map(|c| match c {
                'A' | 'C' | 'G' | 'T' => Some(c),
                'U' => Some('T'),
                'N' | 'R' | 'Y' | 'K' | 'M' | 'S' | 'W' | 'B' | 'D' | 'H'
                | 'V' => Some('N'),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Sequence {
    header: SequenceHeader,
    sequence: SequenceBody,
}

impl Sequence {
    pub fn new<T>(header: T, sequence: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            header: SequenceHeader::new(header),
            sequence: SequenceBody::new(sequence),
        }
    }

    pub fn header(&self) -> &SequenceHeader {
        &self.header
    }

    pub fn header_content(&self) -> &str {
        self.header.header()
    }

    pub fn sequence(&self) -> &SequenceBody {
        &self.sequence
    }

    pub fn sequence_content(&self) -> &str {
        self.sequence.seq()
    }

    pub fn to_fasta(&self) -> String {
        format!(">{}\n{}\n", self.header.header(), self.sequence.seq())
    }
}
