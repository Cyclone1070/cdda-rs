//! Which classes of thing each spell may be aimed at, straight from the dump.
//!
//! Source: `--dump-spells` from the instrumented reference, which now writes `spell_type::valid_targets` by name plus the three filters (`targeted_monster_ids`,
//! `targeted_species_ids`, `ignored_species_ids`). Reference: `spell::is_valid_target` (`magic.cpp:1671-1760`).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

fn dump() -> Vec<cdda_content::spells::Spell> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the spell dump is at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("the dump is JSON");
    cdda_content::spells::load_dump(&value).expect("every entry reads")
}

#[test]
fn the_dump_says_which_classes_each_spell_may_be_aimed_at() {
    let spells = dump();
    assert_eq!(spells.len(), 245, "the dump holds 245 spells");
    let count = |class: &str| {
        spells
            .iter()
            .filter(|spell| spell.valid_targets.contains(class))
            .count()
    };
    // The counts are the data's: a class nobody uses would make its rule dead code, and a count that moved would mean the data moved.
    assert_eq!(
        count("self"),
        147,
        "**147 spells may be cast on the caster**"
    );
    assert_eq!(count("hostile"), 140, "140 on something hostile");
    assert_eq!(count("ground"), 104, "104 on a bare square");
    assert_eq!(count("ally"), 88, "88 on an ally");
    assert_eq!(count("field"), 2, "2 on a field");
    assert_eq!(count("item"), 1, "1 on an item");
    assert_eq!(
        count("none"),
        1,
        "and one that may be aimed at nothing at all"
    );
}

/// **The filters are rare, and the two species filters are empty in this content.**
#[test]
fn nine_spells_name_the_monsters_they_want_and_none_names_a_species() {
    let spells = dump();
    let named = spells
        .iter()
        .filter(|spell| !spell.targeted_monster_ids.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        named.len(),
        9,
        "**nine spells name the monsters they may affect**"
    );
    let one = named
        .iter()
        .find(|spell| spell.id == "death_kill_vines")
        .expect("the data defines it");
    assert_eq!(
        one.targeted_monster_ids.iter().cloned().collect::<Vec<_>>(),
        vec!["mon_creeper_vine".to_owned()],
        "**a spell whose list names exactly one monster** is the sharpest case: it may kill that vine and nothing else"
    );
    assert!(
        spells
            .iter()
            .all(|spell| spell.targeted_species_ids.is_empty()),
        "**and no spell in this content filters by species**, which is why that rule's gate is written from the reference rather than from the data"
    );
    assert!(
        spells
            .iter()
            .all(|spell| spell.ignored_species_ids.is_empty())
    );
}
