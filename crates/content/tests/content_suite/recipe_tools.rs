//! Tools and qualities, the other two thirds of whether a recipe can be made.
//!
//! Transcribed from `tool_comp::has` and `quality_requirement::has`
//! (`src/requirements.cpp:932-990`).

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::recipes::{Component, Quality, Recipe, Tool};

fn recipe(
    components: Vec<Vec<Component>>,
    tools: Vec<Vec<Tool>>,
    qualities: Vec<Vec<Quality>>,
) -> Recipe {
    Recipe {
        id: "test".to_owned(),
        result: "thing".to_owned(),
        blacklisted: false,
        components,
        tools,
        qualities,
        skill: String::new(),
        difficulty: 0,
            time_moves: 0,
    }
}

fn tool(item: &str, count: i32) -> Tool {
    Tool {
        item: item.to_owned(),
        count,
    }
}

fn quality(id: &str, level: i32, count: i32) -> Quality {
    Quality {
        quality: id.to_owned(),
        level,
        count,
    }
}

#[test]
fn a_count_of_minus_one_means_held_not_a_negative_charge() {
    let r = recipe(Vec::new(), vec![vec![tool("hammer", -1)]], Vec::new());
    let held = |id: &str| i32::from(id == "hammer");
    let no_charges = |_: &str| 0;
    assert!(
        r.tools_available(1, held, no_charges),
        "a hammer is not spent"
    );
}

#[test]
fn a_tool_that_spends_charges_needs_them_per_batch() {
    let r = recipe(
        Vec::new(),
        vec![vec![tool("soldering_iron", 10)]],
        Vec::new(),
    );
    let held = |_: &str| 1;
    let twenty = |_: &str| 20;
    assert!(
        r.tools_available(2, held, twenty),
        "two batches need twenty charges"
    );
    let fifteen = |_: &str| 15;
    assert!(
        !r.tools_available(2, held, fifteen),
        "fifteen is not twenty"
    );
}

#[test]
fn one_tool_of_a_group_is_enough() {
    // A hammer or a stone hammer: either satisfies the group.
    let r = recipe(
        Vec::new(),
        vec![vec![tool("hammer", -1), tool("stone_hammer", -1)]],
        Vec::new(),
    );
    let stone = |id: &str| i32::from(id == "stone_hammer");
    assert!(r.tools_available(1, stone, |_| 0));
}

#[test]
fn a_quality_at_or_above_the_level_counts() {
    let r = recipe(Vec::new(), Vec::new(), vec![vec![quality("SEW", 2, 1)]]);
    // The lookup reports how many items reach the asked level, so "3 items at level 3" answers 2.
    let plenty = |_id: &str, level: i32| if level <= 2 { 3 } else { 0 };
    assert!(r.qualities_available(1, plenty));
    let none = |_: &str, _: i32| 0;
    assert!(!r.qualities_available(1, none));
}

#[test]
fn a_quality_needing_two_items_is_not_satisfied_by_one() {
    let r = recipe(Vec::new(), Vec::new(), vec![vec![quality("HAMMER", 1, 2)]]);
    let one = |_: &str, _: i32| 1;
    assert!(!r.qualities_available(1, one));
    let two = |_: &str, _: i32| 2;
    assert!(r.qualities_available(1, two));
}

#[test]
fn a_batch_does_not_multiply_a_quality() {
    let r = recipe(Vec::new(), Vec::new(), vec![vec![quality("SEW", 1, 1)]]);
    let one = |_: &str, _: i32| 1;
    assert!(
        r.qualities_available(5, one),
        "qualities are not spent, so a batch changes nothing"
    );
}

#[test]
fn everything_must_be_present_at_once() {
    let r = recipe(
        vec![vec![Component {
            item: "rag".to_owned(),
            count: 1,
        }]],
        vec![vec![tool("scissors", -1)]],
        vec![vec![quality("SEW", 1, 1)]],
    );
    let all = |_: &str| 1;
    let no_scissors = |id: &str| i32::from(id == "rag");
    let any_quality = |_: &str, _: i32| 1;
    assert!(r.can_make(1, all, all, all, any_quality));
    assert!(
        !r.can_make(1, all, no_scissors, all, any_quality),
        "no scissors"
    );
    assert!(!r.can_make(1, all, all, all, |_, _| 0), "no sewing");
}

#[test]
fn every_real_recipe_is_makeable_when_holding_everything() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let text = std::fs::read_to_string(path).expect("the recipe dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let (all, _) = cdda_content::recipes::from_dump(&value).expect("the dump loads");
    let unlimited = |_: &str| i32::MAX / 2;
    let any_quality = |_: &str, _: i32| i32::MAX / 2;
    let broken: Vec<&str> = all
        .iter()
        .filter(|recipe| !recipe.can_make(1, unlimited, unlimited, unlimited, any_quality))
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(
        broken.is_empty(),
        "recipes unmakable with everything in hand: {broken:?}"
    );
}
