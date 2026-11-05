//! What a monster brings to a fight, read from the reference's own types.
//!
//! Source: `corpus/monsters/monsters.json` is `--dump-monsters` version two, which adds the four numbers an
//! attack needs beyond a name and a glyph: `melee_dice`, `melee_sides`, `melee_dice_ap` and the two armour
//! values. Without them a spawned monster can walk and be hit but cannot hit back, which is exactly the shape
//! the port had before this.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::monsters::MonsterType;
use cdda_json::{Value, parse};
use std::path::Path;

fn loaded() -> Vec<MonsterType> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    cdda_content::monsters::load_objects(prototypes).expect("the dump loads")
}

#[test]
fn a_monster_type_carries_the_dice_it_attacks_with() {
    let types = loaded();
    assert!(
        types.len() > 500,
        "the dump holds {} types, which is too few to be the corpus",
        types.len()
    );

    let zombie = types
        .iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    assert_eq!(zombie.hp, 80, "a zombie has eighty hit points");
    assert_eq!(zombie.speed, 70, "and moves at seventy");
    assert_eq!(zombie.melee_skill, 4, "with a skill of four");
    assert_eq!(zombie.melee_dice, 2, "and two dice of bonus bash");
    assert_eq!(zombie.melee_sides, 3, "with three faces");
    assert_eq!(zombie.melee_dice_ap, 0, "and no penetration");
    assert_eq!(zombie.armor_bash, 0, "and no armour of its own");
    assert_eq!(zombie.armor_cut, 0);
}

#[test]
fn a_type_that_never_states_armour_has_none() {
    let types = loaded();
    let nonesuch = types
        .iter()
        .find(|one| one.id == "mon_null")
        .expect("the corpus has the null monster");
    assert_eq!(nonesuch.melee_dice, 0, "no dice");
    assert_eq!(nonesuch.armor_bash, 0, "no armour");
    assert_eq!(nonesuch.hp, 1, "and one hit point, as the reference's default");
}
