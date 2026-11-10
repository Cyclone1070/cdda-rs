//! How loud a spell is, and why its damage is truncated rather than rounded.
//!
//! Reference: `spell::sound_volume` (`magic.cpp:1630-1640`) and `spell::damage` (`:816-840`) through `min_leveled_damage`, which casts the scaled number to `int` - truncation
//! toward zero. The two rules meet in `spell_effect::noise`, which passes the damage straight through as a loudness.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use std::collections::BTreeSet;

fn a_spell(damage: f64, flags: &[&str]) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "noise".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(damage),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect::<BTreeSet<_>>(),
    }
}

/// **`abs( damage ) / 3`, with `LOUD` adding `1 + damage / 3`, and `SILENT` muting the whole thing.**
#[test]
fn a_spells_loudness_is_its_damage_in_thirds() {
    use cdda_sim::spellcasting::sound_volume;
    assert_eq!(sound_volume(&a_spell(9.0, &[]), 0), 3, "nine damage is three of sound");
    assert_eq!(sound_volume(&a_spell(11.0, &[]), 0), 3, "and **integer division drops the remainder**");
    assert_eq!(sound_volume(&a_spell(2.0, &[]), 0), 0, "two damage is silence");
    assert_eq!(sound_volume(&a_spell(-9.0, &[]), 0), 3, "**a negative spell is as loud as a positive one**, because the rule takes the absolute value");
    assert_eq!(
        sound_volume(&a_spell(9.0, &["LOUD"]), 0),
        7,
        "`LOUD` adds one and then the damage again in thirds: 3 + 1 + 3"
    );
    assert_eq!(
        sound_volume(&a_spell(9.0, &["SILENT"]), 0),
        0,
        "**and `SILENT` silences the cast**, which is 126 of the data's 245 spells"
    );
}

/// **The damage is the truncated integer, not the rounded one.**
#[test]
fn damage_truncates_toward_zero_like_the_reference_int() {
    use cdda_sim::spellcasting::damage_of;
    assert_eq!(damage_of(&a_spell(2.5, &[]), 0), Some(2), "**a half is dropped, never raised**");
    assert_eq!(damage_of(&a_spell(-2.5, &[]), 0), Some(-2), "and toward zero, so minus two");
    assert_eq!(damage_of(&a_spell(7.0, &[]), 0), Some(7), "while a whole number is itself");
}
