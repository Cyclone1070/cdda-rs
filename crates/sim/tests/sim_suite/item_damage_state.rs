//! How one item's damage moves, transcribed from `item::mod_damage`, `set_damage` and `set_degradation`.
//!
//! Reference: `src/item.cpp:9440-9468` (the three-way branch and the destruction test), `src/item.cpp:967-979` (the two setters and the invariant between them), and
//! `get_degrade_amount` (`src/item.cpp:9428-9439`), whose `-1` for undamaged is the reason the first hit degrades an item twice as fast as the second.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::{DamageLimits, ItemDamage, degrade_amount, mod_damage_charges};

/// A vehicle part: the only thing in the game that degrades, 4,000 health, 50 increments.
fn vehicle_part() -> DamageLimits {
    DamageLimits {
        max_damage: 4_000,
        degrade_increments: 50,
        stack_size: 1,
        count_by_charges: false,
        unbreakable: false,
    }
}

/// Everything else: 4,000 health and no degradation at all.
fn ordinary() -> DamageLimits {
    DamageLimits {
        degrade_increments: 0,
        ..vehicle_part()
    }
}

#[test]
fn damage_adds_degradation_by_the_bands_it_crosses() {
    // **The first hit degrades twice as fast as the second.** `factor` answers `-1` for undamaged, not 0 (`src/item.cpp:9433-9434`), so the jump from nothing to
    // 1,000 health spent crosses two bands and the jump from 1,000 to 2,000 crosses one.
    let limits = vehicle_part();
    let fresh = ItemDamage::default();
    let (after, destroyed) = fresh.mod_damage(1_000, &limits);
    assert!(!destroyed, "1,000 of 4,000 is not destruction");
    assert_eq!(after.damage, 1_000);
    assert_eq!(
        after.degradation, 160,
        "two bands of four, at 4,000 / 50 per increment: 2 * 80"
    );

    let (again, _) = after.mod_damage(1_000, &limits);
    assert_eq!(again.damage, 2_000);
    assert_eq!(again.degradation, 240, "one more band: 160 + 80");

    // And a type that does not degrade keeps its degradation at zero however hard it is hit, while still taking the damage.
    let (plain, _) = fresh.mod_damage(1_000, &ordinary());
    assert_eq!(plain.damage, 1_000);
    assert_eq!(plain.degradation, 0, "no increments, no degradation");
}

#[test]
fn the_hit_that_reaches_the_maximum_is_not_the_hit_that_destroys() {
    // **The destruction test is made before the damage is applied** (`( damage_ + qty ) > max_damage()`, `src/item.cpp:9449`), and `set_damage` clamps to the
    // maximum — so an item at 4,000 is at its last point of health and one more hit kills it. Damage never passes the maximum, and degradation is a floor the damage
    // cannot be repaired below.
    let limits = vehicle_part();
    let (full, destroyed) = ItemDamage::default().mod_damage(5_000, &limits);
    assert!(destroyed, "5,000 takes it past 4,000");
    assert_eq!(
        full.damage, 4_000,
        "and the damage is clamped at the maximum, not left above it"
    );

    // Exactly to the maximum is survivable.
    let (exact, destroyed) = ItemDamage::default().mod_damage(4_000, &limits);
    assert!(!destroyed, "4,000 is not past 4,000");
    assert_eq!(exact.damage, 4_000);

    // Repairs cannot go below the degradation: `set_damage( 0 )` on a degraded item leaves the degradation in place.
    let mut degraded = exact;
    degraded.set_degradation(1_000, &limits);
    degraded.set_damage(0, &limits);
    assert_eq!(
        degraded.damage, 1_000,
        "the clamp's lower bound is the degradation, so a repair cannot undo it"
    );
    assert_eq!(
        degrade_amount(&ItemDamage::default(), 0, &limits),
        0,
        "and an undamaged item crosses no bands, so a repair of nothing degrades nothing"
    );
}

#[test]
fn a_pile_counted_in_charges_loses_charges_and_not_health() {
    // Reference: `src/item.cpp:9444-9447`. A pile has no health at all; the damage scale is divided by the stack size to decide how many units go.
    let nails = DamageLimits {
        max_damage: 0,
        degrade_increments: 0,
        stack_size: 20,
        count_by_charges: true,
        unbreakable: false,
    };
    // 20 nails per unit of damage at the scale of 1,000: 1,000 damage takes the whole stack size's worth, twice over would take forty.
    let (left, emptied) = mod_damage_charges(200, 1_000, nails.stack_size);
    assert_eq!(left, 180, "a thousand damage takes twenty nails");
    assert!(!emptied, "and leaves a hundred and eighty");

    let (none, emptied) = mod_damage_charges(10, 1_000, nails.stack_size);
    assert_eq!(none, 0, "a pile of ten cannot lose twenty; it loses what it has");
    assert!(emptied, "and that empties it");

    // The health state of a charge-counted type never moves, whatever happens to the pile.
    let (state, destroyed) = ItemDamage::default().mod_damage(1_000, &nails);
    assert_eq!(state, ItemDamage::default());
    assert!(!destroyed, "the pile's fate is its charges', not its health's");

    // And an unbreakable item refuses damage outright rather than taking none of it.
    let tough = DamageLimits {
        unbreakable: true,
        ..vehicle_part()
    };
    let (untouched, destroyed) = ItemDamage::default().mod_damage(9_999, &tough);
    assert_eq!(untouched, ItemDamage::default());
    assert!(!destroyed, "UNBREAKABLE never breaks");
}
