//! Recipes and the flags that decide whether they can be learned.

#![allow(clippy::panic, clippy::expect_used)]

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
