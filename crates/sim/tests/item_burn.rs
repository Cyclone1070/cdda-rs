//! How much of an item a fire consumes in a turn.
//!
//! Transcribed from `item::simulate_burn` (`src/item.cpp:12543-12592`), with the cases that the
//! reference's branches describe: immune materials, volume scaling, contained fires, liquids that
//! smother, several materials averaged, no materials at all, and stacks.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::rng::Rolls;
use cdda_sim::{BurnDataView, Burnable, MaterialPortion, simulate_burn};

fn wood(intensity_levels: [(f64, f64, f64); 3]) -> MaterialPortion {
    let mut levels = [BurnDataView {
        immune: false,
        volume_per_turn: 0.0,
        fuel: 0.0,
        smoke: 0.0,
        burn: 0.0,
    }; 3];
    for (index, (fuel, smoke, burn)) in intensity_levels.iter().enumerate() {
        levels[index] = BurnDataView {
            immune: false,
            volume_per_turn: 0.0,
            fuel: *fuel,
            smoke: *smoke,
            burn: *burn,
        };
    }
    MaterialPortion {
        portions: 1,
        levels,
    }
}

fn item(materials: Vec<MaterialPortion>, volume_ml: f64) -> Burnable {
    Burnable {
        materials,
        volume_ml,
        liquid: false,
        material_total: 1,
        by_charges: false,
        charges: 0,
        stack_size: 1,
    }
}

#[test]
fn a_plain_wooden_item_feeds_the_fire_its_material_does() {
    let logs = item(
        vec![wood([(1.0, 1.0, 0.01), (2.0, 1.0, 0.02), (3.0, 1.0, 0.03)])],
        1000.0,
    );
    let mut rolls = Rolls::new(1);
    let burn = simulate_burn(&logs, 2, false, &mut rolls);
    assert!(
        (burn.fuel - 2.0).abs() < 1e-9,
        "wood at intensity 2 fuels 2"
    );
    assert!((burn.smoke - 1.0).abs() < 1e-9);
    assert!((burn.burn - 0.02).abs() < 1e-9);
}

#[test]
fn an_immune_material_stops_the_whole_item() {
    let mut levels = wood([(5.0, 5.0, 0.5); 3]).levels;
    for level in &mut levels {
        level.immune = true;
    }
    let nomex = MaterialPortion {
        portions: 1,
        levels,
    };
    let suit = item(vec![nomex], 500.0);
    let mut rolls = Rolls::new(1);
    assert_eq!(
        simulate_burn(&suit, 3, false, &mut rolls),
        cdda_sim::BurnResult {
            fuel: 0.0,
            smoke: 0.0,
            burn: 0.0
        },
        "fireproof means fireproof"
    );
}

#[test]
fn a_contained_fire_burns_at_three_and_ignores_volume() {
    let mut material = wood([(1.0, 0.0, 0.1); 3]);
    material.levels[2].volume_per_turn = 100.0;
    let thing = item(vec![material], 50.0);
    let mut rolls = Rolls::new(1);
    let contained = simulate_burn(&thing, 1, true, &mut rolls);
    assert!(
        (contained.fuel - 1.0).abs() < 1e-9,
        "uses intensity 3's data without volume scaling"
    );
}

#[test]
fn a_volume_limited_material_scales_with_the_items_size() {
    let mut material = wood([(1.0, 0.0, 0.1); 3]);
    for level in &mut material.levels {
        level.volume_per_turn = 250.0;
    }
    let small = item(vec![material], 500.0);
    let large = item(vec![material], 1000.0);
    let mut rolls = Rolls::new(1);
    let small_burn = simulate_burn(&small, 1, false, &mut rolls);
    let mut rolls = Rolls::new(1);
    let large_burn = simulate_burn(&large, 1, false, &mut rolls);
    assert!(
        small_burn.fuel > large_burn.fuel,
        "same material, half the volume, twice the rate"
    );
}

#[test]
fn two_materials_are_averaged_by_their_portions() {
    let mut first = wood([(2.0, 0.0, 0.0); 3]);
    first.portions = 1;
    let mut second = wood([(4.0, 0.0, 0.0); 3]);
    second.portions = 1;
    let mut mixed = item(vec![first, second], 1000.0);
    mixed.material_total = 2;
    let mut rolls = Rolls::new(1);
    let burn = simulate_burn(&mixed, 1, false, &mut rolls);
    assert!((burn.fuel - 3.0).abs() < 1e-9, "the mean of 2 and 4");
}

#[test]
fn an_item_with_no_materials_burns_at_a_flat_rate() {
    let nothing = item(Vec::new(), 100.0);
    let mut rolls = Rolls::new(1);
    let burn = simulate_burn(&nothing, 1, false, &mut rolls);
    assert!((burn.burn - 1.0).abs() < 1e-9, "the reference's fallback");
    assert!(burn.fuel.abs() < 1e-9);
}

#[test]
fn a_liquid_that_burns_poorly_smothers_the_fire() {
    let mut wet = item(vec![wood([(1.0, 0.0, 0.1); 3])], 1000.0);
    wet.liquid = true;
    let mut rolls = Rolls::new(1);
    let burn = simulate_burn(&wet, 1, false, &mut rolls);
    assert!(burn.fuel < 0.0, "fuel goes negative: it puts the fire out");
}

#[test]
fn a_solid_item_draws_no_dice_at_all() {
    let solid = item(vec![wood([(1.0, 0.0, 0.1); 3])], 1000.0);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    let _ = simulate_burn(&solid, 1, false, &mut rolls);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "burning a solid is arithmetic"
    );
}

#[test]
fn a_stack_multiplies_by_how_many_stacks_it_holds() {
    let mut stack = item(vec![wood([(1.0, 1.0, 0.01); 3])], 250.0);
    stack.by_charges = true;
    stack.charges = 6;
    stack.stack_size = 3;
    let mut rolls = Rolls::new(1);
    let burn = simulate_burn(&stack, 1, false, &mut rolls);
    assert!(
        (burn.fuel - 2.0).abs() < 1e-9,
        "six charges in stacks of three is two stacks"
    );
}
