//! What each harvest drop type is judged on: the skills that dissecting it practises and measures.
//!
//! Reference: `harvest_drop_type.json` and `butchery.cpp:550-564`. A drop type may name `harvest_skills`, and the dissection of a body's entries is judged on **their** levels plus the blade - which is
//! why a body of flesh and bone is judged on the blade alone, and a body carrying mutagens is not.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::harvest_drops;

fn data() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/src/data/json/harvest_drop_type.json")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn the_ordinary_tissues_name_no_skills_and_a_mutagen_names_two() {
    let types = harvest_drops::load(&[&data()]).expect("the drop types");
    let by_id = |id: &str| {
        types
            .get(id)
            .map_or_else(Vec::new, |each| each.harvest_skills.clone())
    };
    for tissue in ["flesh", "bone", "skin", "blood", "offal"] {
        assert!(
            by_id(tissue).is_empty(),
            "**{tissue} is judged on the blade alone**, which is what an empty list means"
        );
    }
    assert_eq!(
        by_id("mutagen"),
        vec!["chemistry".to_owned(), "firstaid".to_owned()],
        "a mutagen is judged on the two skills the data names, in the data's order"
    );
    assert!(
        types.len() > 5,
        "and the file really was read: {} drop types",
        types.len()
    );
}

#[test]
fn a_drop_type_keeps_its_own_spelling_of_the_messages_it_names() {
    let types = harvest_drops::load(&[&data()]).expect("the drop types");
    let flesh = types.get("flesh").expect("flesh");
    assert_eq!(
        flesh.messages.dissect_fail.as_deref(),
        Some("harvest_drop_tissue_dissect_failed"),
        "the failure message a dissection shows for flesh"
    );
}
