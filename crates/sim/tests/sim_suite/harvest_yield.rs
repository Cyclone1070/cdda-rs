//! How much a butchered corpse actually gives.
//!
//! The entries here are transcribed from real harvest definitions in the reference tree (`data/json/harvest.json`):
//! a fish that yields a share of its mass in flesh, and a plant that drops a count of seeds.

#![allow(clippy::panic)]

use cdda_sim::{CorpseDamage, HarvestEntry, HarvestSpec, harvest_yield};

fn fish_flesh() -> HarvestSpec {
    // {"drop":"fish","type":"flesh","mass_ratio":0.32}
    HarvestSpec {
        part: Some(HarvestEntry::Flesh),
        base_num: (0, 0),
        scale_num: (0.0, 0.0),
        mass_ratio: Some(0.32),
        max: None,
        is_group: false,
    }
}

fn seeds() -> HarvestSpec {
    // {"drop":"seed_spurge","base_num":[1,2]}
    HarvestSpec {
        part: None,
        base_num: (1, 2),
        scale_num: (0.0, 0.0),
        mass_ratio: None,
        max: None,
        is_group: false,
    }
}

#[test]
fn a_share_of_the_mass_is_what_the_corpse_weighs_times_the_ratio() {
    // 400 grams of fish at 32 percent: 128.
    assert_eq!(
        harvest_yield(
            fish_flesh(),
            0,
            400.0,
            CorpseDamage::Light,
            false,
            false,
            0.5
        ),
        128
    );
}

#[test]
fn a_damaged_corpse_gives_less_of_that_share() {
    // 128 * 0.9 for flesh at the damaged level: 115.2, which rounds to 115.
    assert_eq!(
        harvest_yield(
            fish_flesh(),
            0,
            400.0,
            CorpseDamage::Damaged,
            false,
            false,
            0.5
        ),
        115
    );
    // Pulped flesh keeps half: 64.
    assert_eq!(
        harvest_yield(
            fish_flesh(),
            0,
            400.0,
            CorpseDamage::Pulped,
            false,
            false,
            0.5
        ),
        64
    );
}

#[test]
fn a_count_entry_rolls_between_its_numbers() {
    assert_eq!(
        harvest_yield(seeds(), 0, 0.0, CorpseDamage::Light, false, false, 0.0),
        1
    );
    assert_eq!(
        harvest_yield(seeds(), 0, 0.0, CorpseDamage::Light, false, false, 1.0),
        2
    );
}

#[test]
fn skill_scales_the_count_between_the_two_ends() {
    let mut scaled = seeds();
    scaled.base_num = (1, 2);
    scaled.scale_num = (0.5, 1.0);
    // At butchery 4 the range is 3 to 6, so a full draw gives 6.
    assert_eq!(
        harvest_yield(scaled, 4, 0.0, CorpseDamage::Light, false, false, 1.0),
        6
    );
}

#[test]
fn a_damaged_count_cannot_fall_below_what_the_definition_promises() {
    // The floor is applied after the damage, so a mangled corpse still gives its minimum of one.
    assert_eq!(
        harvest_yield(seeds(), 0, 0.0, CorpseDamage::Mangled, false, false, 0.0),
        1
    );
}

#[test]
fn a_ceiling_caps_the_count() {
    let mut capped = seeds();
    capped.base_num = (5, 20);
    capped.max = Some(6);
    assert_eq!(
        harvest_yield(capped, 0, 0.0, CorpseDamage::Light, false, false, 1.0),
        6
    );
}

#[test]
fn a_corpse_smashed_apart_has_half_its_flesh_and_bone() {
    // Light damage, so nothing else interferes: 128 becomes 64.
    assert_eq!(
        harvest_yield(
            fish_flesh(),
            0,
            400.0,
            CorpseDamage::Light,
            true,
            false,
            0.5
        ),
        64
    );
}

#[test]
fn a_skinned_corpse_gives_no_skin_even_undamaged() {
    let skin = HarvestSpec {
        part: Some(HarvestEntry::Skin),
        base_num: (10, 10),
        scale_num: (0.0, 0.0),
        mass_ratio: None,
        max: None,
        is_group: false,
    };
    assert_eq!(
        harvest_yield(skin, 0, 0.0, CorpseDamage::Light, false, true, 0.5),
        0
    );
    assert_eq!(
        harvest_yield(skin, 0, 0.0, CorpseDamage::Light, false, false, 0.5),
        10,
        "and the whole amount when it was not skinned"
    );
}

#[test]
fn blood_is_not_reduced_by_damage() {
    let blood = HarvestSpec {
        part: Some(HarvestEntry::Blood),
        base_num: (0, 0),
        scale_num: (0.0, 0.0),
        mass_ratio: Some(0.1),
        max: None,
        is_group: false,
    };
    assert_eq!(
        harvest_yield(blood, 0, 1000.0, CorpseDamage::Pulped, false, false, 0.5),
        100,
        "a pulped body still had blood in it"
    );
}

/// **A group entry places nothing, even when its `drop` names a real item.**
///
/// Reference: `butchery.cpp:713-716` resolves a drop only `if( entry.type->is_itype() )`, and every placement sits inside `if( drop != nullptr )` (`:829-914`) - so a
/// `mutagen_group` or `bionic_group` entry walks the whole loop, consumes no weight, and leaves the floor empty. This gate is the one that would have been vacuous on the
/// corpus (no group id is an item id), so it uses the sharpest case the rule allows: a group entry whose drop **is** a type the world knows.
#[test]
fn a_group_entry_drops_nothing_even_when_its_drop_is_a_real_item() {
    let real = HarvestSpec {
        part: Some(HarvestEntry::Other),
        base_num: (5, 5),
        scale_num: (0.0, 0.0),
        mass_ratio: None,
        max: None,
        is_group: false,
    };
    let grouped = HarvestSpec {
        is_group: true,
        ..real
    };
    assert_eq!(
        cdda_sim::harvest_yield(
            real,
            0,
            81_500.0,
            cdda_sim::CorpseDamage::Light,
            false,
            false,
            0.5
        ),
        5,
        "an ordinary entry gives its five"
    );
    assert_eq!(
        cdda_sim::harvest_yield(
            grouped,
            0,
            81_500.0,
            cdda_sim::CorpseDamage::Light,
            false,
            false,
            0.5
        ),
        0,
        "**and the same entry marked a group gives none at all**"
    );
}
