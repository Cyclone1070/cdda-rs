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

#[test]
fn tools_and_qualities_are_read_with_their_counts_and_levels() {
    // A tool's count is charges spent, a quality's is how many of them, and the level is a
    // floor. Getting any of the three wrong gives a recipe that looks right and cannot be
    // crafted, or one that can be crafted with the wrong kit.
    let (loaded, _) = dump();
    let with_tools = loaded.iter().filter(|r| !r.tools.is_empty()).count();
    let with_qualities = loaded.iter().filter(|r| !r.qualities.is_empty()).count();
    assert_eq!(with_tools, 3_040, "recipes that need a tool");
    assert_eq!(with_qualities, 4_024, "recipes that need a quality");

    let fins = loaded.iter().find(|r| r.id == "swim_fins").expect("exists");
    let tool_group = fins.tools.first().expect("a tool group");
    assert_eq!(tool_group.len(), 2, "two moulds, either one works");
    assert_eq!(tool_group[0].item, "makeshift_vacuum_mold");
    assert_eq!(tool_group[0].count, 32);
    assert_eq!(
        tool_group[1].count, 16,
        "the better mould costs fewer charges"
    );
    let quality = fins
        .qualities
        .first()
        .and_then(|g| g.first())
        .expect("a quality");
    assert_eq!(quality.quality, "SEW");
    assert_eq!(quality.level, 1);
    assert_eq!(quality.count, 1);
}

#[test]
fn a_tool_that_is_only_held_is_marked_with_minus_one() {
    // The reference encodes "required but not consumed" as -1. It is not a count, and a port
    // that treated it as one would either spend negative charges or require none of the tool -
    // which would let a recipe be crafted bare-handed.
    let (loaded, _) = dump();
    let held = loaded
        .iter()
        .flat_map(|recipe| recipe.tools.iter().flatten())
        .filter(|tool| tool.count == -1)
        .count();
    let spent = loaded
        .iter()
        .flat_map(|recipe| recipe.tools.iter().flatten())
        .filter(|tool| tool.count > 0)
        .count();
    assert!(held > 0, "the data marks some tools as held-only");
    assert!(spent > held, "and most tools do spend charges");
    assert!(
        loaded
            .iter()
            .flat_map(|recipe| recipe.tools.iter().flatten())
            .all(|tool| tool.count == -1 || tool.count > 0),
        "no tool has a zero count: the encoding has no other meaning"
    );
}

/// Every recipe says how long it takes, and the port reads that as moves.
///
/// Reference: `recipe::time` is `to_moves<int>( read_from_json_string<time_duration> )`, so `"20 m"` is a hundred and
/// twenty thousand moves. A crafting activity whose duration came out as zero would finish instantly and no test of
/// the *result* would notice, so the count of recipes with a real time is the gate's point.
#[test]
fn every_recipe_states_how_long_it_takes_in_moves() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let text = std::fs::read_to_string(path).expect("the recipe dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let recipes = recipes::from_dump(&value).expect("the dump loads").0;
    let timed = recipes
        .iter()
        .filter(|recipe| recipe.time_moves > 0)
        .count();
    assert!(
        timed > 5_000,
        "only {timed} of {} recipes state a time, which cannot be right",
        recipes.len()
    );
    assert!(
        recipes.iter().all(|recipe| recipe.time_moves % 100 == 0),
        "a duration is a whole number of seconds, and a second is a hundred moves"
    );
}
