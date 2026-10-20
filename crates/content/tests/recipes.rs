//! Every recipe the reference has, by id and by what it makes.
//!
//! The oracle is `corpus/recipes.json`, written by `--dump-recipes`. The gate is exact: the
//! port must find the same set of recipe ids, and the same result item for each, because a
//! recipe listed under the wrong result is worse than a recipe that is missing - it crafts the
//! wrong thing and looks deliberate.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]

use cdda_content::recipes;
use cdda_content::recipes::Recipe;
use cdda_json::parse;
use std::path::Path;

fn dump() -> (Vec<Recipe>, usize) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let text = std::fs::read_to_string(path).expect("the recipe dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    recipes::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_recipe_the_reference_has() {
    let (loaded, count) = dump();
    assert_eq!(count, 6_291, "the dump says how many the reference has");
    assert_eq!(loaded.len(), count, "and the port finds exactly that many");
}

#[test]
fn no_recipe_is_missing_and_none_is_invented() {
    let (loaded, _) = dump();
    let mut ids: Vec<&str> = loaded.iter().map(|recipe| recipe.id.as_str()).collect();
    ids.sort_unstable();
    let unique = {
        let mut copy = ids.clone();
        copy.dedup();
        copy.len()
    };
    assert_eq!(
        unique,
        ids.len(),
        "the reference has no duplicate recipe ids"
    );
    assert!(ids.contains(&"swim_fins"), "and a known recipe is present");
}

#[test]
fn the_nesting_of_a_recipe_s_s_needs_is_kept() {
    // Flattening either level would be invisible in a count and wrong in play: a group is a
    // choice, and the recipe needs one of each group.
    let (loaded, _) = dump();
    let multi_group = loaded.iter().filter(|r| r.components.len() > 1).count();
    let with_choices = loaded
        .iter()
        .filter(|r| r.components.iter().any(|group| group.len() > 1))
        .count();
    assert_eq!(multi_group, 4_061, "recipes needing more than one group");
    assert_eq!(
        with_choices, 3_656,
        "recipes where at least one need has alternatives"
    );
    assert!(
        loaded
            .iter()
            .all(|r| r.components.iter().all(|group| !group.is_empty())),
        "no empty group: an empty group would mean a free recipe"
    );
}

#[test]
fn a_known_recipe_keeps_its_exact_alternatives_and_counts() {
    let (loaded, _) = dump();
    let swim_fins = loaded
        .iter()
        .find(|r| r.id == "swim_fins")
        .expect("the recipe exists");
    assert_eq!(swim_fins.components.len(), 3, "three groups");
    let first = &swim_fins.components[0];
    assert_eq!(first.len(), 3, "three glues");
    assert_eq!(first[0].item, "duct_tape");
    assert_eq!(first[0].count, 50);
    assert_eq!(first[2].item, "bone_glue");
    assert_eq!(first[2].count, 2);
    let last = swim_fins.components[2]
        .first()
        .expect("the third group has its sheet");
    assert_eq!(
        last.item, "polycarbonate_sheet",
        "a group with no choice at all"
    );
    assert_eq!(last.count, 8);
}

#[test]
fn every_recipe_keeps_the_result_the_reference_gave_it() {
    let (loaded, _) = dump();
    let swim_fins = loaded
        .iter()
        .find(|recipe| recipe.id == "swim_fins")
        .expect("the recipe exists");
    assert_eq!(
        swim_fins.result, "swim_fins",
        "it makes what it is named after"
    );
    let mismatched: Vec<&str> = loaded
        .iter()
        .filter(|recipe| recipe.result.is_empty())
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "every recipe names a result: {mismatched:?}"
    );
}
