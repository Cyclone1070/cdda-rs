//! Whether the crafter is skilled enough.
//!
//! Transcribed from `Character::has_recipe_requirements` and the difficulty check in
//! `Character::can_craft` (`src/crafting.cpp`).

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::recipes::{self, Recipe};

fn recipe(skill: &str, difficulty: i32) -> Recipe {
    Recipe {
        id: "test".to_owned(),
        result: "thing".to_owned(),
        blacklisted: false,
        components: Vec::new(),
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: skill.to_owned(),
        difficulty,
    }
}

fn dump() -> Vec<Recipe> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let text = std::fs::read_to_string(path).expect("the recipe dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    recipes::from_dump(&value).expect("the dump loads").0
}

#[test]
fn a_crafter_at_the_difficulty_can_make_it() {
    let r = recipe("fabrication", 3);
    assert!(
        r.skilled_enough(true, 3),
        "exactly the difficulty is enough"
    );
    assert!(r.skilled_enough(true, 9));
}

#[test]
fn one_level_short_is_short() {
    let r = recipe("fabrication", 3);
    assert!(!r.skilled_enough(true, 2));
}

#[test]
fn being_skilled_does_not_mean_knowing_the_recipe() {
    let r = recipe("fabrication", 0);
    assert!(
        !r.skilled_enough(false, 10),
        "an unlearned recipe is not craftable however good you are"
    );
}

#[test]
fn a_difficulty_of_zero_still_needs_the_recipe() {
    let r = recipe("cooking", 0);
    assert!(r.skilled_enough(true, 0));
    assert!(
        !r.skilled_enough(true, -1),
        "a negative level cannot reach zero"
    );
}

#[test]
fn every_real_recipe_names_a_skill_and_the_levels_are_in_range() {
    let all = dump();
    assert_eq!(all.len(), 6_291);
    let without: Vec<&str> = all
        .iter()
        .filter(|recipe| recipe.skill.is_empty())
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(without.is_empty(), "recipes with no skill: {without:?}");
    let fabrication = all
        .iter()
        .filter(|recipe| recipe.skill == "fabrication")
        .count();
    assert_eq!(fabrication, 2_960, "the reference's own count");
    let out_of_range: Vec<&str> = all
        .iter()
        .filter(|recipe| !(0..=10).contains(&recipe.difficulty))
        .map(|recipe| recipe.id.as_str())
        .collect();
    assert!(
        out_of_range.is_empty(),
        "difficulties outside 0..=10: {out_of_range:?}"
    );
}

#[test]
fn a_fresh_crafter_can_make_the_recipes_that_ask_nothing_of_them() {
    // 1,396 recipes are difficulty zero, and a character with the skill at level zero and the recipes
    // known can make exactly those.
    let all = dump();
    let can = all
        .iter()
        .filter(|recipe| recipe.skilled_enough(true, 0))
        .count();
    assert_eq!(can, 1_396, "zero-difficulty recipes");
}
