//! The flags the data gives every spell, which five ported effects read.
//!
//! Source: `--dump-spells` from the instrumented reference (`harness.cpp`), which now writes `spell_type::flags` - the raw strings the JSON carries. Before this the
//! port could not tell a silent bash from a loud one, a line that passes walls from one that stops, or a charm that stacks from one that does not, and said so as a gap.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

fn dump() -> Vec<cdda_content::spells::Spell> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the spell dump is at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("the dump is JSON");
    cdda_content::spells::load_dump(&value).expect("every entry reads")
}

#[test]
fn the_dump_carries_the_flag_every_ported_effect_asks_about() {
    let spells = dump();
    assert_eq!(spells.len(), 245, "the dump holds 245 spells");
    let flagged = spells.iter().filter(|spell| !spell.flags.is_empty()).count();
    assert_eq!(flagged, 233, "**233 of them carry at least one flag**");
    let count = |flag: &str| spells.iter().filter(|spell| spell.has_flag(flag)).count();
    // The counts are the data's, not a sample: a flag nobody has would make its rule dead code, and a count that moved would mean the data moved.
    assert_eq!(count("SILENT"), 126, "126 spells are silent");
    assert_eq!(count("IGNORE_WALLS"), 7, "7 pass through walls");
    assert_eq!(count("NO_PROJECTILE"), 54, "54 are not stopped short by a wall");
    assert_eq!(count("PERMANENT"), 29, "29 summon something for good");
    assert_eq!(count("UNSAFE_TELEPORT"), 1, "and one teleport may land somewhere deadly");
    assert_eq!(count("RECHARM"), 0, "while no spell in this content may re-charm - which is why the charm gate's second cast does nothing");
}

/// **A spell's flags are the ones its id names**, read as names rather than positions.
#[test]
fn a_flagged_spell_carries_the_flags_its_entry_states() {
    let spells = dump();
    let silent = spells
        .iter()
        .find(|spell| spell.id == "dragon_inflatable")
        .expect("the data defines dragon_inflatable");
    assert!(
        silent.has_flag("SILENT") && silent.has_flag("NO_EXPLOSION_SFX"),
        "**both of the entry's flags are on it**: {:?}",
        silent.flags
    );
    assert!(!silent.has_flag("IGNORE_WALLS"), "and nothing it did not state");
}
