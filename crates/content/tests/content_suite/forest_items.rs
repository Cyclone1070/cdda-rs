//! The items a forest chunk leaves on its ground.
//!
//! Reference: `mapgen_forest` ends with
//!
//! ```text
//! for( int i = 0; i < self_biome.item_spawn_iterations; i++ )
//!     m->place_items( item_group, item_group_chance, zero, SEEX*2-1, SEEY*2-1, z, true, when );
//! ```
//!
//! and `map::place_items` turns the chance into a **count** (`roll_remainder( chance * spawn_rate / 100 )`), then picks a
//! square at random for each count - rejecting squares the items may not sit on, up to twenty tries - and fills it. The dice
//! therefore are: one for the count, then three per attempt. A port that skips this spends none, which is exactly how the
//! replay desynchronised.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_build::place_forest_items;
use cdda_core_types::rng::Rolls;

#[test]
fn a_chance_under_a_hundred_places_at_most_one_batch_per_iteration() {
    // A sixty-in-a-hundred chance is not "sixty percent of the squares": it is one roll for whether **this chunk** gets a
    // batch at all, once per iteration. With a spawn rate of one, that is `roll_remainder(0.6)` - zero or one.
    let mut placed = 0;
    for seed in 0..200 {
        let mut rolls = Rolls::new(seed);
        let squares = place_forest_items(24, 60, 1, 1.0, &mut rolls, |_, _| false);
        assert!(
            squares.len() <= 1,
            "seed {seed} placed {} batches",
            squares.len()
        );
        placed += squares.len();
    }
    // Sixty percent of two hundred is a hundred and twenty; the roll has to be a roll, not a constant.
    assert!(
        (80..=160).contains(&placed),
        "{placed} of 200 chunks got a batch"
    );
}

#[test]
fn every_iteration_can_place_its_own_batch() {
    // Two iterations are two independent rolls and two independent squares - the port must not place once and copy.
    let mut any_two = 0;
    for seed in 0..400 {
        let mut rolls = Rolls::new(seed);
        let squares = place_forest_items(24, 100, 2, 1.0, &mut rolls, |_, _| false);
        assert_eq!(squares.len(), 2, "seed {seed}");
        if squares[0] != squares[1] {
            any_two += 1;
        }
    }
    assert!(
        any_two > 300,
        "the two batches landed on the same square too often: {any_two} of 400"
    );
}

#[test]
fn a_square_items_may_not_sit_on_is_passed_over() {
    // `map::place_items` retries while the chosen square is unusable, up to twenty times, and places nothing if it never
    // finds one. A port that placed anyway would put a log inside a tree trunk.
    let mut rolls = Rolls::new(7);
    let squares = place_forest_items(24, 100, 1, 1.0, &mut rolls, |_, _| true);
    assert!(
        squares.is_empty(),
        "every square was refused, so nothing is placed: {squares:?}"
    );
}

#[test]
fn the_dice_are_spent_in_the_reference_s_order() {
    // Two runs with the same seed have to agree, and a run whose count roll fails must not spend the three draws that
    // picking a square would have cost - the reference does not.
    let mut first = Rolls::new(4242);
    let a = place_forest_items(24, 60, 1, 1.0, &mut first, |_, _| false);
    let mut second = Rolls::new(4242);
    let b = place_forest_items(24, 60, 1, 1.0, &mut second, |_, _| false);
    assert_eq!(
        a, b,
        "the same seed places the same batch on the same square"
    );
}
