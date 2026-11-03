//! Whether what is on hand satisfies a recipe.
//!
//! Transcribed from `requirement_data::has_comps` (`src/requirements.cpp:895-940`).

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::recipes::{self, Component, Recipe};
use cdda_json::parse;

fn recipe(components: Vec<Vec<Component>>) -> Recipe {
    Recipe {
        id: "test".to_owned(),
        result: "thing".to_owned(),
        blacklisted: false,
        components,
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
            time_moves: 0,
            batch_rscale: 0.0,
            batch_rsize: 0,
            made_per_batch: 1,
            byproduct_group: Vec::new(),
            container: None,
            byproducts: Vec::new(),
    }
}

fn comp(item: &str, count: i32) -> Component {
    Component {
        item: item.to_owned(),
        count,
    }
}

#[test]
fn one_of_each_group_is_enough() {
    let r = recipe(vec![vec![comp("rag", 2)], vec![comp("stick", 1)]]);
    let stock = |id: &str| match id {
        "rag" => 5,
        "stick" => 1,
        _ => 0,
    };
    assert!(r.components_available(1, stock));
}

#[test]
fn a_missing_group_fails_the_recipe_even_when_everything_else_is_present() {
    let r = recipe(vec![vec![comp("rag", 2)], vec![comp("stick", 1)]]);
    let stock = |id: &str| if id == "rag" { 99 } else { 0 };
    assert!(!r.components_available(1, stock));
}

#[test]
fn alternatives_are_a_choice_not_a_sum() {
    // A rag *or* a leather patch: one rag and one patch is not two of anything.
    let r = recipe(vec![vec![comp("rag", 2), comp("patch", 2)]]);
    let one_each = |_id: &str| 1;
    assert!(
        !r.components_available(1, one_each),
        "one of each is not two of either"
    );
    let two_rags = |id: &str| if id == "rag" { 2 } else { 0 };
    assert!(r.components_available(1, two_rags), "two rags satisfy it");
}

#[test]
fn a_batch_multiplies_every_requirement() {
    let r = recipe(vec![vec![comp("rag", 2)]]);
    let stock = |_id: &str| 5;
    assert!(
        r.components_available(2, stock),
        "a batch of two needs four, and five are there"
    );
    let thin = |_id: &str| 3;
    assert!(!r.components_available(2, thin), "three is not four");
}

#[test]
fn a_batch_of_zero_is_treated_as_one_rather_than_needing_nothing() {
    let r = recipe(vec![vec![comp("rag", 2)]]);
    let stock = |_id: &str| 2;
    assert!(r.components_available(0, stock));
    let none = |_id: &str| 0;
    assert!(
        !r.components_available(0, none),
        "zero batch is not a free recipe"
    );
}

#[test]
fn a_recipe_with_no_requirements_can_always_be_made() {
    let r = recipe(Vec::new());
    assert!(r.components_available(1, |_| 0));
}

#[test]
fn every_real_recipe_is_satisfiable_when_the_stock_is_unlimited() {
    // A rule that rejects a group it should accept shows up here for every recipe at once, which is
    // the cheapest way to catch a mistaken nesting level across 6,291 recipes.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("corpus/recipes.json");
    let text = std::fs::read_to_string(&path).expect("the recipe dump");
    let value = parse(text.as_bytes()).expect("valid json");
    let (all, _) = recipes::from_dump(&value).expect("the dump loads");
    assert_eq!(all.len(), 6_291);
    let unlimited = |_: &str| i32::MAX / 2;
    let broken: Vec<&str> = all
        .iter()
        .filter(|recipe| !recipe.components_available(1, unlimited))
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(
        broken.is_empty(),
        "recipes that cannot be made even with everything: {broken:?}"
    );
}

#[test]
fn nothing_can_be_made_from_an_empty_stock() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("corpus/recipes.json");
    let text = std::fs::read_to_string(&path).expect("the recipe dump");
    let value = parse(text.as_bytes()).expect("valid json");
    let (all, _) = recipes::from_dump(&value).expect("the dump loads");
    let nothing = |_: &str| 0;
    let free: Vec<&str> = all
        .iter()
        .filter(|recipe| !recipe.components.is_empty() && recipe.components_available(1, nothing))
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(free.is_empty(), "recipes made from nothing: {free:?}");
}
