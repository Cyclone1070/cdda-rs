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

#[test]
fn a_definition_that_names_no_leftovers_leaves_ruined_chunks() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/src/data/json/harvest.json")
        .to_string_lossy()
        .into_owned();
    let value = cdda_content::read_json(std::path::Path::new(&path)).expect("the harvest file");
    let definitions = cdda_content::harvest::load_harvests(&value);
    let humanoid = definitions
        .get("zombie_humanoid")
        .expect("the commonest body");
    assert_eq!(
        humanoid.leftovers, "ruined_chunks",
        "**no definition in the shipped data names one**, and the reference's default is `ruined_chunks` (`harvest.h:102`) - so every body in the game leaves ruined chunks"
    );
    let named = definitions
        .values()
        .filter(|definition| definition.leftovers != "ruined_chunks")
        .count();
    assert_eq!(
        named, 0,
        "and the shipped data names none at all, which is what makes the default the whole rule"
    );
}

#[test]
fn a_drop_type_says_whether_it_names_an_item_group() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/src/data/json/harvest_drop_type.json")
        .to_string_lossy()
        .into_owned();
    let types = cdda_content::harvest_drops::load(&[&path]).expect("the drop types");
    let groups: Vec<&str> = types
        .values()
        .filter(|drop_type| drop_type.group)
        .map(|drop_type| drop_type.id.as_str())
        .collect();
    assert_eq!(
        groups,
        vec!["bionic_group", "mutagen_group"],
        "**exactly two drop types name an item group**, and neither can drop an item the way an itype entry does"
    );
    assert!(
        !types.get("flesh").expect("flesh").group,
        "and flesh, which is what most of a body gives, is an item type"
    );
}

#[test]
fn the_data_defines_ninety_one_morale_types() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/src/data/json/morale_types.json")
        .to_string_lossy()
        .into_owned();
    let kinds = cdda_content::morale_types::load(&[&path]).expect("the morale types");
    assert_eq!(
        kinds.len(),
        91,
        "**ninety-one morale types**, which is what a spell's `effect_str` is checked against"
    );
    assert!(
        kinds.contains("morale_feeling_good") && kinds.contains("morale_scream"),
        "including the two the content's spells name"
    );
}
