//! Burning the items on a square: how much of what, and in which order.
//!
//! Reference: the item block at the top of the fire processor (`map_field.cpp:976-1048`).
//!
//! ```text
//! fire_data frd( cur.get_field_intensity(), !can_spread );
//! int max_consume = cur.get_field_intensity() * 2;
//! for( auto fuel = items_here.begin(); fuel != items_here.end() && consumed < max_consume; ) {
//!     bool destroyed = fuel->burn( frd );
//!     ...
//! }
//! smoke = roll_remainder( frd.smoke_produced );
//! time_added = 1_turns * roll_remainder( frd.fuel_produced );
//! ```
//!
//! Three things are easy to lose here. The fire only removes `intensity * 2` items a turn, and an item it fails to destroy is looked at again next turn, so a
//! big pile burns slowly. `time_added` is an **assignment**, not an addition - the item block runs before the ground block, and the ground adds to what the
//! items gave. And the two `roll_remainder` calls are the only dice the block spends beyond the per-item ones that round `burnt`.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::fire::{BurningItem, MaterialBurn, burn_the_items};
use cdda_core_types::rng::{Recording, Rolls};

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

fn plank() -> BurningItem {
    BurningItem {
        volume_ml: 4130.0,
        materials: vec![(WOOD.to_vec(), 1)],
        burnt: 0.0,
    }
}

#[test]
fn a_plank_gives_a_whole_turn_of_fuel_only_when_its_roll_lands() {
    // 200/4130 of a turn is 0.0484, so `roll_remainder` gives one turn about one time in twenty. The recording decides which: here it lands.
    let mut items = vec![plank()];
    let mut rolls = Rolls::replaying(
        Recording::parse("real 0 1 0\nreal 0 1 0\nreal 0 1 0").expect("a recording"),
    );
    let burn = burn_the_items(&mut items, 1, false, &mut rolls);
    assert_eq!(burn.time_added, 1, "the plank's fraction rounded up");
    // Wood smokes the same fraction as it fuels, so a recording of three zero reals rounds both totals up.
    assert_eq!(burn.smoke, 1);
    assert_eq!(items.len(), 1, "and the plank is still there");
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn the_same_fire_gets_nothing_when_the_roll_misses() {
    let mut items = vec![plank()];
    let mut rolls = Rolls::replaying(
        Recording::parse("real 0 1 1\nreal 0 1 1\nreal 0 1 1").expect("a recording"),
    );
    let burn = burn_the_items(&mut items, 1, false, &mut rolls);
    assert_eq!(burn.time_added, 0);
    assert_eq!(burn.smoke, 0);
}

#[test]
fn a_fire_takes_only_what_its_intensity_allows_it_to_destroy() {
    // `max_consume = intensity * 2`, and the count only rises when an item is **destroyed**. Six near-gone rags in a level-one fire: the loop stops after
    // two destructions however many are lying there, which is why a big pile burns slowly. Each rag's burning is exact (flat at one a turn, threshold
    // three), so this test has no dice of its own to get wrong.
    static RAG: [MaterialBurn; 3] = [MaterialBurn {
        immune: false,
        volume_per_turn_ml: 0.0,
        fuel: 0.0,
        smoke: 0.0,
        burn: 1.0,
    }; 3];
    let rag = BurningItem {
        volume_ml: 250.0,
        materials: vec![(RAG.to_vec(), 1)],
        burnt: 2.0,
    };
    let mut items = vec![rag; 6];
    let mut rolls = Rolls::new(7);
    let burn = burn_the_items(&mut items, 1, false, &mut rolls);
    assert_eq!(
        items.len(),
        4,
        "two were destroyed and the loop stopped: {burn:?}"
    );
}

#[test]
fn an_immune_item_is_left_alone_and_gives_nothing() {
    static ASBESTOS: [MaterialBurn; 3] = [MaterialBurn {
        immune: true,
        volume_per_turn_ml: 0.0,
        fuel: 0.0,
        smoke: 0.0,
        burn: 0.0,
    }; 3];
    let mut items = vec![BurningItem {
        volume_ml: 500.0,
        materials: vec![(ASBESTOS.to_vec(), 1)],
        burnt: 0.0,
    }];
    let mut rolls = Rolls::new(7);
    let burn = burn_the_items(&mut items, 3, false, &mut rolls);
    assert_eq!(burn.time_added, 0);
    assert_eq!(items.len(), 1);
}

#[test]
fn a_contained_fire_eats_an_item_at_level_three_and_rounds_up_every_time() {
    // Contained: level three's numbers with no volume division, so a plank gives three whole turns of fuel and one smoke - wood smokes the same at every
    // level - and both are exact, so neither total's die can change them.
    let mut items = vec![plank()];
    let mut rolls = Rolls::new(7);
    let burn = burn_the_items(&mut items, 1, true, &mut rolls);
    assert_eq!(burn.time_added, 3);
    assert_eq!(burn.smoke, 1);
}
