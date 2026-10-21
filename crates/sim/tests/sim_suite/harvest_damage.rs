//! What a damaged corpse still gives up.

#![allow(clippy::panic)]

use cdda_sim::{CorpseDamage, HarvestEntry, harvest_after_damage};

#[test]
fn an_unmarked_corpse_gives_its_full_weight() {
    for entry in [
        HarvestEntry::Flesh,
        HarvestEntry::Offal,
        HarvestEntry::Skin,
        HarvestEntry::Bone,
    ] {
        assert_eq!(harvest_after_damage(100, entry, CorpseDamage::Light), 100);
    }
}

#[test]
fn a_damaged_corpse_loses_flesh_last() {
    // Flesh at 90 percent, hide and offal at 75: the parts that spoil first are not the meat.
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Flesh, CorpseDamage::Damaged),
        90
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Skin, CorpseDamage::Damaged),
        75
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Offal, CorpseDamage::Damaged),
        75
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Bone, CorpseDamage::Damaged),
        100
    );
}

#[test]
fn a_mangled_corpse_has_no_offal_left_at_all() {
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Offal, CorpseDamage::Mangled),
        0
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Skin, CorpseDamage::Mangled),
        50
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Bone, CorpseDamage::Mangled),
        90
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Flesh, CorpseDamage::Mangled),
        75
    );
}

#[test]
fn a_pulped_corpse_is_only_bone() {
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Offal, CorpseDamage::Pulped),
        0
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Skin, CorpseDamage::Pulped),
        0
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Bone, CorpseDamage::Pulped),
        75
    );
    assert_eq!(
        harvest_after_damage(100, HarvestEntry::Flesh, CorpseDamage::Pulped),
        50
    );
}

#[test]
fn rounding_goes_to_the_nearest_and_not_down() {
    // 90 percent of 5 is 4.5, which std::round makes 5; truncation would make 4.
    assert_eq!(
        harvest_after_damage(5, HarvestEntry::Flesh, CorpseDamage::Damaged),
        5
    );
    // 75 percent of 2 is 1.5, which rounds to 2.
    assert_eq!(
        harvest_after_damage(2, HarvestEntry::Skin, CorpseDamage::Damaged),
        2
    );
}

#[test]
fn a_pulped_corpse_of_nothing_is_still_nothing() {
    for entry in [HarvestEntry::Flesh, HarvestEntry::Bone] {
        assert_eq!(harvest_after_damage(0, entry, CorpseDamage::Pulped), 0);
    }
}
