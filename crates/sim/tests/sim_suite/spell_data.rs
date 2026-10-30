//! What the game's spells do, read from the oracle's own dump.
//!
//! The dump (`corpus/spells.json`) is the reference engine's answer, taken after it has loaded, inherited and merged
//! everything: 245 spells, and an `effect_name` for each. A hand scan of the source JSON agreed on 245 spells and on every
//! effect kind **except one** - it found 54 `effect_on_condition` where the engine has 63, because nine of them reach their
//! effect through `copy-from` and the scan's idea of "this object is a spell" was narrower than the loader's. The engine wins;
//! this test pins the engine's numbers and says why.
//!
//! The lesson is worth the file: a scan is a model of the loader, and a model that disagrees with the loader is wrong until
//! proven otherwise - the direction a port must never assume.

#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeMap;

/// Every spell in the dump, by the effect it names.
fn spells_by_effect() -> (usize, BTreeMap<String, usize>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(&path).expect("the spell dump is in the repository");
    let value = cdda_json::parse(text.as_bytes()).expect("the spell dump is json");
    let cdda_json::Value::Object(root) = &value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(spells)) = root.get("spells") else {
        panic!("the dump carries a list of spells");
    };
    let mut effects: BTreeMap<String, usize> = BTreeMap::new();
    for spell in spells {
        let effect = match spell {
            cdda_json::Value::Object(fields) => match fields.get("effect_name") {
                Some(cdda_json::Value::Str(effect)) => effect.clone(),
                _ => "(none)".to_owned(),
            },
            _ => "(none)".to_owned(),
        };
        *effects.entry(effect).or_insert(0) += 1;
    }
    (spells.len(), effects)
}

/// The counts the reference states, pinned so a document cannot drift from the engine again.
#[test]
fn the_dump_says_how_many_spells_there_are_and_what_each_one_does() {
    let (spells, effects) = spells_by_effect();
    let mut top: Vec<(String, usize)> = effects
        .iter()
        .map(|(name, count)| (name.clone(), *count))
        .collect();
    top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("spells: {spells}; effects: {top:?}");

    assert_eq!(spells, 245, "the engine has 245 spells");
    assert_eq!(effects.get("attack").copied(), Some(77), "77 of them attack");
    assert_eq!(
        effects.get("effect_on_condition").copied(),
        Some(63),
        "and 63 are programs: nine more than a source scan finds, because nine inherit their effect"
    );
    assert_eq!(effects.get("summon").copied(), Some(32));
}
