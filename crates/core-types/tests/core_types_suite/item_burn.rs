//! What a burning item gives the fire that is eating it.
//!
//! Reference: `item::simulate_burn` (`src/item.cpp:12543-12596`) and `item::burn` (`:12598-12640`).
//!
//! The rule is *volume-proportional*: a material burns at a fixed volume per turn, so what an item contributes is that rate divided by the item's own volume.
//! A plank of 4130 ml made of wood, burning in a level-one fire, therefore gives `1 * (200 / 4130)` of a turn's fuel - which is why one turn in twenty it
//! gives a whole turn and the other nineteen it gives nothing, and why the reference's own trace shows a newborn fire's age going 120, 119, 120: it ate one
//! tenth of a plank's worth of fuel on the turn the roll went its way.
//!
//! Two cases escape the division: a material with `volume_per_turn` of zero burns at a flat rate whatever the item's size (which is how the reference keeps
//! big fires burning), and a **contained** fire (a brazier, an oven) burns everything flat, because the fire is being fed on purpose.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{MaterialBurn, simulate_burn};

/// Wood at the three intensities the reference's own material data gives (`corpus/prototypes/materials.json`).
fn wood() -> &'static [MaterialBurn] {
    &WOOD
}

/// The same table, named so tests can borrow it.
static WOOD: [MaterialBurn; 3] = [
    MaterialBurn {
        immune: false,
        volume_per_turn_ml: 200.0,
        fuel: 1.0,
        smoke: 1.0,
        burn: 0.01,
    },
    MaterialBurn {
        immune: false,
        volume_per_turn_ml: 825.0,
        fuel: 2.0,
        smoke: 1.0,
        burn: 0.02,
    },
    MaterialBurn {
        immune: false,
        volume_per_turn_ml: 0.0,
        fuel: 3.0,
        smoke: 1.0,
        burn: 0.03,
    },
];

fn plank() -> Vec<(&'static [MaterialBurn], i32)> {
    vec![(wood(), 1)]
}

#[test]
fn a_plank_feeds_a_fire_in_proportion_to_its_own_volume() {
    let burn = simulate_burn(&plank(), 4130.0, 1, false);
    assert!(
        (burn.fuel - 200.0 / 4130.0).abs() < 1e-9,
        "one turn of wood is 200 ml, and the plank is 4130 ml: {}",
        burn.fuel
    );
    assert!((burn.smoke - 200.0 / 4130.0).abs() < 1e-9);
}

#[test]
fn a_material_with_no_volume_per_turn_burns_flat() {
    // Wood's third level: no volume per turn, so the item's size stops mattering and the fire gets the whole fuel value.
    let burn = simulate_burn(&[(wood(), 1)], 4130.0, 3, false);
    assert!(
        (burn.fuel - 3.0).abs() < 1e-9,
        "three, not three times a rate"
    );
    assert!((burn.burn - 0.03).abs() < 1e-9);
}

#[test]
fn a_contained_fire_burns_everything_flat() {
    // `frd.contained` forces the flat branch **and** level three's numbers (`effective_intensity = frd.contained ? 3 : frd.fire_intensity`), so a
    // contained fire eats an item as though it were raging, without dividing by the item's volume.
    let burn = simulate_burn(&plank(), 4130.0, 1, true);
    assert!(
        (burn.fuel - 3.0).abs() < 1e-9,
        "level three's fuel, undivided"
    );
    assert!((burn.smoke - 1.0).abs() < 1e-9);
}

#[test]
fn an_immune_material_contributes_nothing() {
    let immune = [MaterialBurn {
        immune: true,
        ..WOOD[0]
    }];
    let burn = simulate_burn(&[(&immune, 1)], 4130.0, 2, false);
    assert!(
        burn.fuel.abs() < f64::EPSILON,
        "an item made to shield from fire does not feed one"
    );
    assert!(burn.burn.abs() < f64::EPSILON);
    assert!(burn.smoke.abs() < f64::EPSILON);
}

#[test]
fn two_materials_are_averaged_by_their_portions() {
    // `time_added /= mat_total` - the item has two portions, so each material's contribution is halved.
    let pair = vec![(wood(), 1), (wood(), 2)];
    let burn = simulate_burn(&pair, 4130.0, 1, false);
    let each = 200.0 / 4130.0;
    assert!(
        (burn.fuel - (each + 2.0 * each) / 3.0).abs() < 1e-9,
        "three portions in total, not two materials: {}",
        burn.fuel
    );
}

#[test]
fn portion_counts_scale_each_material_before_the_average() {
    // A two-to-one mix: wood twice over in one slot contributes twice as much, and the divisor counts portions, not slots.
    let pair = vec![(wood(), 2), (wood(), 1)];
    let burn = simulate_burn(&pair, 4130.0, 1, false);
    let each = 200.0 / 4130.0;
    assert!((burn.fuel - (2.0 * each + each) / 3.0).abs() < 1e-9);
}
