//! How much of the spell machine the port can actually run, counted against the oracle's dump.
//!
//! The dump says which of the 33 effect kinds each of the 245 spells names (`spells::the_dump_says_how_many_spells_there_are`
//! pins the census). This test compares that list with the port's dispatch table, so "spell effects are 4 of 33" is a
//! measurement rather than a memory - and so the next effect to port is chosen by how many spells it unlocks.

#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeMap;

fn spells_by_effect() -> BTreeMap<String, usize> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(&path).expect("the spell dump is in the repository");
    let value = cdda_json::parse(text.as_bytes()).expect("the spell dump is json");
    let cdda_json::Value::Object(root) = &value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(spells)) = root.get("spells") else {
        panic!("the dump carries a list of spells");
    };
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    for spell in spells {
        let cdda_json::Value::Object(fields) = spell else {
            continue;
        };
        let Some(cdda_json::Value::Str(effect)) = fields.get("effect_name") else {
            continue;
        };
        *counted.entry(effect.clone()).or_insert(0) += 1;
    }
    counted
}

/// The count of kinds the port runs, and the count of spells those kinds belong to.
#[test]
fn the_port_runs_four_of_the_data_s_effect_kinds() {
    let effects = spells_by_effect();
    let ported: Vec<(&String, &usize)> = effects
        .iter()
        .filter(|(name, _)| cdda_sim::spell_effects::is_ported(name))
        .collect();
    let spells_covered: usize = ported.iter().map(|(_, count)| *count).sum();

    let mut listed: Vec<String> = ported
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    listed.sort();
    println!(
        "effect kinds: {} in the data, {} ported; spells covered {spells_covered} of 245 ({listed:?})",
        effects.len(),
        ported.len()
    );

    assert_eq!(
        effects.len(),
        32,
        "the data's spells name 32 distinct effect kinds"
    );
    assert_eq!(ported.len(), 4, "the port runs four of them");
    assert_eq!(
        spells_covered, 39,
        "and those four are what 39 of the 245 spells name"
    );
}
