//! Recipes and the flags that decide whether they can be learned.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

/// A recipe the reference refuses to teach is marked, and the loader keeps the mark.
///
/// Reference: `recipe::never_learn` (`recipe.h:173`), read by `learn_recipe` (`character_crafting.cpp:32-40`) which **returns without teaching** when it is set. The count is the gate's non-vacuity:
/// the reference's own dump marks **1,114 of its 6,291 recipes**, and a loader that dropped the field would leave every one of them learnable.
#[test]
fn the_never_learn_flag_survives_the_dump() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the recipe dump parses");
    let (recipes, count) = cdda_content::recipes::from_dump(&value).expect("the recipe dump loads");
    assert_eq!(recipes.len(), count, "the dump agrees with its own count");

    let never = recipes.iter().filter(|recipe| recipe.never_learn).count();
    assert_eq!(never, 1114, "**1,114 recipes can never be learned** - the reference's own number");
    assert!(
        recipes.iter().any(|recipe| !recipe.never_learn),
        "and most can be, so the flag is not simply true everywhere"
    );
}

/// A practice recipe carries the range its difficulty is clamped into, and most recipes carry none.
///
/// Reference: `recipe::practice_data` (`recipe.h:72-84`) - `min_difficulty`, `max_difficulty` and `skill_limit` - which `get_difficulty` and `get_skill_cap` read (`recipe.cpp:61-77`). The reference's own
/// dump marks **124 of 6,291** recipes as practice recipes, so the count is the gate's non-vacuity: a loader that invented a block for every recipe, or dropped it for all of them, fails.
#[test]
fn practice_recipes_carry_their_range() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the recipe dump parses");
    let (recipes, _) = cdda_content::recipes::from_dump(&value).expect("the recipe dump loads");

    let practice: Vec<_> = recipes.iter().filter(|recipe| recipe.practice.is_some()).collect();
    assert_eq!(practice.len(), 124, "**exactly 124 practice recipes**, the reference's own number");
    let first = practice.first().and_then(|recipe| recipe.practice).expect("one of them is first");
    assert!(first.min_difficulty <= first.max_difficulty, "the range is a range: {first:?}");
    assert!(
        practice.iter().any(|recipe| recipe.practice.is_some_and(|block| block.skill_limit < 20)),
        "and at least one of them states a real skill limit rather than the maximum"
    );
    assert!(
        recipes.iter().any(|recipe| recipe.practice.is_none()),
        "while an ordinary recipe has no block at all"
    );
}
