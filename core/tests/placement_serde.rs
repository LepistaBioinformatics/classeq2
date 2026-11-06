use classeq_core::domain::dtos::{
    adherence_test::AdherenceTest,
    annotation::{Annotation, Tag},
    clade::{Clade, NodeType},
    placement_response::{PlacementResponse, PlacementStatus},
};
use mycelium_base::dtos::UntaggedParent;
use serde_json::json;

fn clade_record() -> Clade {
    Clade {
        id: 2,
        parent: Some(1),
        kind: NodeType::Node,
        name: None,
        support: Some(99.0),
        length: Some(0.08),
        children: Some(vec![Clade {
            id: 3,
            parent: Some(2),
            kind: NodeType::Leaf,
            name: Some("A1".to_string()),
            support: None,
            length: Some(0.01),
            children: None,
        }]),
    }
}

fn identity_found_by_id() -> PlacementStatus {
    PlacementStatus::IdentityFound(AdherenceTest {
        clade: UntaggedParent::Id(2),
        one: 10,
        rest: 3,
    })
}

fn inconclusive() -> PlacementStatus {
    PlacementStatus::Inconclusive(
        vec![
            AdherenceTest {
                clade: UntaggedParent::Id(2),
                one: 5,
                rest: 1,
            },
            AdherenceTest {
                clade: UntaggedParent::Id(5),
                one: 5,
                rest: 1,
            },
        ],
        "Multiple proposals".to_string(),
    )
}

fn max_resolution() -> PlacementStatus {
    PlacementStatus::MaxResolutionReached(7, "LCA Accepted".to_string())
}

fn unclassifiable() -> PlacementStatus {
    PlacementStatus::Unclassifiable("no kmers".to_string())
}

// ? ---------------------------------------------------------------------------
// ? PlacementStatus shapes
// ? ---------------------------------------------------------------------------

#[test]
fn status_to_string_is_the_response_code() {
    assert_eq!(unclassifiable().to_string(), "Unclassifiable: no kmers");
    assert_eq!(identity_found_by_id().to_string(), "IdentityFound");
    assert_eq!(
        max_resolution().to_string(),
        "MaxResolutionReached: LCA Accepted"
    );
    assert_eq!(
        inconclusive().to_string(),
        "Inconclusive: Multiple proposals"
    );
}

#[test]
fn status_json_shapes() {
    assert_eq!(
        serde_json::to_value(unclassifiable()).unwrap(),
        json!({ "status": "Unclassifiable", "message": "no kmers" })
    );
    assert_eq!(
        serde_json::to_value(identity_found_by_id()).unwrap(),
        json!({ "status": "IdentityFound", "clade": 2, "one": 10, "rest": 3 })
    );
    assert_eq!(
        serde_json::to_value(max_resolution()).unwrap(),
        json!({
            "status": "MaxResolutionReached",
            "clade": 7,
            "message": "LCA Accepted"
        })
    );
    assert_eq!(
        serde_json::to_value(inconclusive()).unwrap(),
        json!({
            "status": "Inconclusive",
            "proposals": [
                { "clade": 2, "one": 5, "rest": 1 },
                { "clade": 5, "one": 5, "rest": 1 }
            ],
            "message": "Multiple proposals"
        })
    );
}

#[test]
fn status_identity_found_with_record_serializes_the_clade_object() {
    let status = PlacementStatus::IdentityFound(AdherenceTest {
        clade: UntaggedParent::Record(clade_record()),
        one: 10,
        rest: 3,
    });

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        r#"{"status":"IdentityFound","clade":{"id":2,"parent":1,"kind":"NODE","support":99.0,"length":0.08,"children":[{"id":3,"parent":2,"kind":"LEAF","name":"A1","length":0.01}]},"one":10,"rest":3}"#
    );
}

#[test]
fn status_yaml_shapes() {
    assert_eq!(
        serde_saphyr::to_string(&identity_found_by_id()).unwrap(),
        "status: IdentityFound\nclade: 2\none: 10\nrest: 3\n"
    );
    assert_eq!(
        serde_saphyr::to_string(&max_resolution()).unwrap(),
        "status: MaxResolutionReached\nclade: 7\nmessage: LCA Accepted\n"
    );
    assert_eq!(
        serde_saphyr::to_string(&inconclusive()).unwrap(),
        "status: Inconclusive\n\
         proposals:\n\
         - clade: 2\n  \
           one: 5\n  \
           rest: 1\n\
         - clade: 5\n  \
           one: 5\n  \
           rest: 1\n\
         message: Multiple proposals\n"
    );
    assert_eq!(
        serde_saphyr::to_string(&unclassifiable()).unwrap(),
        "status: Unclassifiable\nmessage: no kmers\n"
    );
}

#[test]
fn status_identity_found_round_trips() {
    let status = identity_found_by_id();
    let json = serde_json::to_string(&status).unwrap();

    assert_eq!(
        serde_json::from_str::<PlacementStatus>(&json).unwrap(),
        status
    );
}

fn every_status() -> [PlacementStatus; 5] {
    [
        unclassifiable(),
        identity_found_by_id(),
        PlacementStatus::IdentityFound(AdherenceTest {
            clade: UntaggedParent::Record(clade_record()),
            one: 10,
            rest: 3,
        }),
        max_resolution(),
        inconclusive(),
    ]
}

#[test]
fn status_every_variant_round_trips_through_json() {
    for status in every_status() {
        let json = serde_json::to_string(&status).unwrap();
        let back: Result<PlacementStatus, _> = serde_json::from_str(&json);
        assert_eq!(back.ok(), Some(status.clone()), "{json}");
    }
}

#[test]
fn status_every_variant_round_trips_through_yaml() {
    for status in every_status() {
        let yaml = serde_saphyr::to_string(&status).unwrap();
        let back: Result<PlacementStatus, _> = serde_saphyr::from_str(&yaml);
        assert_eq!(back.ok(), Some(status.clone()), "{yaml}");
    }
}

#[test]
fn status_without_tag_is_rejected() {
    assert!(serde_json::from_value::<PlacementStatus>(json!(7)).is_err());
    assert!(serde_json::from_value::<PlacementStatus>(json!(
        "Unclassifiable: no kmers"
    ))
    .is_err());
    assert!(serde_json::from_value::<PlacementStatus>(json!({
        "clade": 2, "one": 10, "rest": 3
    }))
    .is_err());
}

// ? ---------------------------------------------------------------------------
// ? PlacementResponse shapes
// ? ---------------------------------------------------------------------------

fn annotated_response() -> PlacementResponse<PlacementStatus> {
    let status = identity_found_by_id();
    PlacementResponse::new("q1".to_string(), status.to_string(), Some(status))
        .with_annotation(Some(vec![
            Annotation {
                clade: 0,
                meta: None,
            },
            Annotation {
                clade: 2,
                meta: Some(vec![
                    Tag::Taxid(5),
                    Tag::SciName("Genus species".to_string()),
                    Tag::Rank("species".to_string()),
                ]),
            },
        ]))
}

#[test]
fn response_json_shape() {
    assert_eq!(
        serde_json::to_string(&annotated_response()).unwrap(),
        r#"{"query":"q1","code":"IdentityFound","annotations":[{"clade":0},{"clade":2,"meta":[{"Taxid":5},{"SciName":"Genus species"},{"Rank":"species"}]}],"placement":{"status":"IdentityFound","clade":2,"one":10,"rest":3}}"#
    );
}

/// Annotation tags are written as single-key maps (`Taxid: 5`), the same
/// shape as JSON. serde_yaml (up to v0.11) wrote YAML tags (`!Taxid 5`).
#[test]
fn response_yaml_shape() {
    assert_eq!(
        serde_saphyr::to_string(&annotated_response()).unwrap(),
        "query: q1\n\
         code: IdentityFound\n\
         annotations:\n\
         - clade: 0\n\
         - clade: 2\n  \
           meta:\n  \
           - Taxid: 5\n  \
           - SciName: Genus species\n  \
           - Rank: species\n\
         placement:\n  \
           status: IdentityFound\n  \
           clade: 2\n  \
           one: 10\n  \
           rest: 3\n"
    );
}

/// Annotation files written by hand or by v0.11 and earlier use YAML tags;
/// they must keep loading.
#[test]
fn annotations_read_both_yaml_tags_and_maps() {
    let tagged: Vec<Annotation> = serde_saphyr::from_str(
        "- clade: 2\n  meta:\n  - !Taxid 5\n  - !SciName Genus species\n  - !Note |\n    multi\n    line\n",
    )
    .unwrap();
    let mapped: Vec<Annotation> = serde_saphyr::from_str(
        "- clade: 2\n  meta:\n  - Taxid: 5\n  - SciName: Genus species\n  - Note: \"multi\\nline\\n\"\n",
    )
    .unwrap();

    assert_eq!(tagged, mapped);
    assert_eq!(
        tagged[0].meta.as_ref().unwrap()[0],
        Tag::Taxid(5)
    );
}

#[test]
fn response_without_placement_or_annotations_omits_both_keys() {
    let response: PlacementResponse<PlacementStatus> = PlacementResponse::new(
        "q2".to_string(),
        unclassifiable().to_string(),
        None,
    );

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({ "query": "q2", "code": "Unclassifiable: no kmers" })
    );
    assert_eq!(response.placement(), None);
}

#[test]
fn response_with_max_resolution_round_trips_with_tagged_placement() {
    let status = max_resolution();
    let response = PlacementResponse::new(
        "q3".to_string(),
        status.to_string(),
        Some(status),
    );

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({
            "query": "q3",
            "code": "MaxResolutionReached: LCA Accepted",
            "placement": {
                "status": "MaxResolutionReached",
                "clade": 7,
                "message": "LCA Accepted"
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<PlacementResponse<PlacementStatus>>(
            serde_json::to_value(&response).unwrap()
        )
        .unwrap(),
        response
    );
}

#[test]
fn response_with_identity_found_round_trips() {
    let response = annotated_response();
    let json = serde_json::to_string(&response).unwrap();

    assert_eq!(
        serde_json::from_str::<PlacementResponse<PlacementStatus>>(&json)
            .unwrap(),
        response
    );
}

// ? ---------------------------------------------------------------------------
// ? AdherenceTest and Annotation shapes
// ? ---------------------------------------------------------------------------

#[test]
fn adherence_test_round_trips_with_id_and_record() {
    for clade in [
        UntaggedParent::Id(4),
        UntaggedParent::Record(clade_record()),
    ] {
        let test = AdherenceTest {
            clade,
            one: 1,
            rest: 2,
        };
        let json = serde_json::to_string(&test).unwrap();
        assert_eq!(serde_json::from_str::<AdherenceTest>(&json).unwrap(), test);
    }
}

#[test]
fn annotation_tags_are_externally_tagged_pascal_case() {
    let annotation = Annotation {
        clade: 1,
        meta: Some(vec![
            Tag::Gene("gyrB".to_string()),
            Tag::InferenceMethod("ML".to_string()),
            Tag::Note("n".to_string()),
        ]),
    };

    let value = serde_json::to_value(&annotation).unwrap();

    assert_eq!(
        value,
        json!({
            "clade": 1,
            "meta": [{ "Gene": "gyrB" }, { "InferenceMethod": "ML" }, { "Note": "n" }]
        })
    );
    assert_eq!(
        serde_json::from_value::<Annotation>(value).unwrap(),
        annotation
    );
}
