use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum Tag {
    /// Taxid tag.
    Taxid(u32),

    /// The scientific name of the organism.
    SciName(String),

    /// The tag is a rank tag.
    Rank(String),

    /// A tag for genes.
    Gene(String),

    /// The method used to infer the phylogeny.
    InferenceMethod(String),

    /// The tag is a simple tag.
    Note(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// The clade ID to which the annotation belongs.
    pub clade: u32,

    /// A simple list of tags associated with the annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Vec<Tag>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_annotation() {
        let annotation = Annotation {
            clade: 1,
            meta: Some(vec![
                Tag::Taxid(9606),
                Tag::SciName("Colletotrichum higginsianum".to_string()),
                Tag::Note("any other tag".to_string()),
            ]),
        };

        let annotations = vec![annotation.clone(), annotation.clone()];

        let yaml = serde_yaml::to_string(&annotations).unwrap();

        // Save to file
        let path = std::path::PathBuf::from("/tmp/annotation.yaml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();
    }
}
