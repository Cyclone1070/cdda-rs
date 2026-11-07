//! The weak-point sets a monster can have, read from the reference's own dump.
//!
//! Source: `corpus/weakpoints/weakpoint-sets.json` is `--dump-weakpoint-sets`, which walks `weakpoints::get_all()` and writes every entry's coverage, direction, four multiplier maps and
//! per-attack-type difficulty. **A weak-point set is data rather than code**: the reference has 48 of them, and a rule that wrote one down by hand would be wrong the day the data changed.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_content::weakpoints::{WeakpointSet, load_objects};
use cdda_json::{Value, parse};
use std::path::Path;

fn loaded() -> Vec<WeakpointSet> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weakpoints/weakpoint-sets.json");
    let text = std::fs::read(path).expect("the weakpoint dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(sets)) = root.get("sets") else {
        panic!("the dump has no sets");
    };
    load_objects(sets).expect("the dump loads")
}

/// Every set loads, and the figures the selection rule needs are the ones the dump states.
///
/// Reference: `struct weakpoints` (`weakpoint.h:180-205`) and `struct weakpoint` (`:132-166`). **The count is the gate's non-vacuity**: 48 sets, every one with a default weak point, and the
/// entries in the data's own order because that order is the order the walk tries them in.
#[test]
fn every_weakpoint_set_loads() {
    let sets = loaded();
    assert_eq!(sets.len(), 48, "**the reference defines forty-eight weak-point sets**");
    assert!(
        sets.iter().all(|set| !set.id.is_empty()),
        "every set states an id a monster can name"
    );
    // **A set's default is a full-coverage body hit**, not an empty one: the reference's default weak point states coverage 100 and difficulty -100 for every attack type, which is what makes
    // "the roll found no weak point" a plain hit rather than a miss. The assertion is written from the dump for that reason.
    let default = &sets[0].default;
    assert!(default.id.is_empty(), "the default weak point has no id");
    assert_eq!(default.coverage, 100.0, "**a default weak point covers the whole body**");
    assert!(
        default.armour_multiplier.is_empty() && default.damage_multiplier.is_empty(),
        "and it changes nothing about the blow"
    );
    assert!(
        sets.iter().any(|set| !set.weakpoints.is_empty()),
        "and some sets have entries to walk"
    );

    // The figures a rule reads: a coverage, a direction, and the two multiplier maps that make a weak point worth hitting.
    let with_crit = sets
        .iter()
        .flat_map(|set| set.weakpoints.iter())
        .find(|entry| !entry.crit_multiplier.is_empty())
        .expect("some weak point multiplies a critical hit");
    assert!(
        with_crit.helps_attacker,
        "**a weak point that multiplies a crit helps the attacker**: {}",
        with_crit.id
    );
    let with_armour = sets
        .iter()
        .flat_map(|set| set.weakpoints.iter())
        .find(|entry| !entry.armour_multiplier.is_empty())
        .expect("some weak point changes the armour it meets");
    assert!(!with_armour.id.is_empty());

    // The difficulty is stated per attack type, which is five figures in the reference's own order.
    let with_difficulty = sets
        .iter()
        .flat_map(|set| set.weakpoints.iter())
        .find(|entry| entry.difficulty.len() == 5)
        .expect("a weak point states a difficulty for every attack type");
    assert_eq!(with_difficulty.difficulty.len(), 5, "none, bash, cut, stab and projectile");
}

/// A multiplier stated as `all` reaches every damage type, and a stated type still wins.
///
/// Reference: `finalize_damage_map` (`damage.cpp:789-829`) - a map's `all` entry is the default for every damage type, `physical` and `non_physical` fill their own halves, and **a type named
/// outright keeps its own figure**. The dump writes the maps **before** that expansion (the harness runs `check_mod_data`, which does not finalize them), so the expansion is the port's job -
/// and without it a weak point whose data says `{"all": 0.5}` would change nothing at all, because no rule ever asks about a damage type called `all`.
#[test]
fn an_all_multiplier_reaches_every_damage_type() {
    use cdda_content::weakpoints::expand_all;

    let all = std::collections::BTreeMap::from([("all".to_owned(), 0.5)]);
    let expanded = expand_all(&all, 1.0);
    assert_eq!(expanded.get("bash").copied(), Some(0.5), "**the all entry is every type's default**");
    assert_eq!(expanded.get("cut").copied(), Some(0.5));
    assert_eq!(expanded.get("stab").copied(), Some(0.5));
    assert!(!expanded.contains_key("all"), "and the keyword itself is gone once it has been spent");

    let mixed = std::collections::BTreeMap::from([("all".to_owned(), 0.5), ("bash".to_owned(), 2.0)]);
    let expanded = expand_all(&mixed, 1.0);
    assert_eq!(expanded.get("bash").copied(), Some(2.0), "**a type named outright keeps its own figure**");
    assert_eq!(expanded.get("cut").copied(), Some(0.5), "and the rest take the default");

    let empty = std::collections::BTreeMap::new();
    let expanded = expand_all(&empty, 0.0);
    assert_eq!(
        expanded.get("bash").copied(),
        Some(0.0),
        "an unstated map is the caller's default, which for a penalty is nothing rather than nothing-times-nothing"
    );
}
