//! What the game's spell data actually says, counted rather than remembered.
//!
//! The port's spell dump (`corpus/spells.json`) carries a spell's levels, damage, range and duration, but not its **effect**,
//! which is the word that says whether the spell attacks, summons, or is an effect-on-condition. The only trustworthy answer
//! to "how many spells are programs?" is therefore a scan of the source data, and this is that scan. It exists because a
//! number in a document that nothing checked had drifted: the ledger said 63 spells name an effect-on-condition, and the
//! data says 54.

#![allow(clippy::panic)]

use std::collections::BTreeMap;

/// Every spell object at any depth under the source data, by the `effect` it names.
fn count_spell_effects() -> (usize, BTreeMap<String, usize>) {
    fn walk(value: &cdda_json::Value, spells: &mut usize, effects: &mut BTreeMap<String, usize>) {
        match value {
            cdda_json::Value::Object(object) => {
                let looks_like_a_spell = matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "SPELL")
                    || (object.get("id").is_some()
                        && object.get("max_level").is_some()
                        && object.get("effect").is_some());
                if looks_like_a_spell {
                    *spells += 1;
                    let effect = match object.get("effect") {
                        Some(cdda_json::Value::Str(effect)) => effect.clone(),
                        _ => "(none)".to_owned(),
                    };
                    *effects.entry(effect).or_insert(0) += 1;
                }
                for (_, member) in object.members() {
                    walk(member, spells, effects);
                }
            }
            cdda_json::Value::Array(entries) => {
                for entry in entries {
                    walk(entry, spells, effects);
                }
            }
            _ => {}
        }
    }

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut spells = 0;
    let mut effects = BTreeMap::new();
    let mut stack = vec![data];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            walk(&value, &mut spells, &mut effects);
        }
    }
    (spells, effects)
}

/// The counts the data states, pinned so the documents cannot drift from the game again.
#[test]
fn the_data_says_how_many_spells_there_are_and_what_each_one_does() {
    let (spells, effects) = count_spell_effects();
    let top: Vec<(String, usize)> = {
        let mut listed: Vec<(String, usize)> = effects
            .iter()
            .map(|(name, count)| (name.clone(), *count))
            .collect();
        listed.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        listed
    };
    println!("spells: {spells}; effects: {top:?}");

    assert_eq!(spells, 245, "the data defines 245 spells");
    assert_eq!(
        effects.get("attack").copied(),
        Some(77),
        "77 of them attack"
    );
    assert_eq!(
        effects.get("effect_on_condition").copied(),
        Some(54),
        "and 54 are programs, which the ledger had counted as 63"
    );
}
