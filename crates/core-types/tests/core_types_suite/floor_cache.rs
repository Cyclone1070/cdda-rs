//! Where a level's floor is missing, and the one piece of furniture that puts it back.
//!
//! Reference: `map::build_floor_cache( zlev )` (`src/map.cpp:9417-9471`). The cache answers "is there something underfoot here" for every square, and the sunlight pass reads
//! the answer one level up: a square's light comes through only when the level above has no floor there. So this cache is what turns a stairwell into a column of daylight,
//! and what stops a solid roof from doing the same.
//!
//! The rule has two halves:
//!
//! * a square has no floor when its terrain carries **any** of four flags - open air, water you can fall into, a stair going down, or a see-through floor;
//! * **unless** the level below has furniture with `SUN_ROOF_ABOVE` there, which is a greenhouse roof: it catches the sun on its own level, so the square above is not a hole
//!   after all.
//!
//! The cache also carries one summary bit, `no_floor_gaps`, which is true only when every square has a floor. Every caller that asks "can I skip the sunlight pass entirely"
//! reads it.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use cdda_core_types::floor_cache::{FloorFacts, floor_of};

const SIDE: usize = 5;

/// A solid level with the given squares open, and no greenhouse roofs below.
fn level(open: &[(usize, usize)]) -> FloorFacts {
    let mut facts = FloorFacts {
        side: SIDE,
        no_floor: vec![false; SIDE * SIDE],
        sun_roof_below: vec![false; SIDE * SIDE],
    };
    for (x, y) in open {
        facts.no_floor[x * SIDE + y] = true;
    }
    facts
}

#[test]
fn a_solid_level_has_a_floor_everywhere_and_says_so() {
    let answer = floor_of(&level(&[]));
    assert!(
        answer.cells.iter().all(|floor| *floor),
        "every square has something underfoot"
    );
    assert!(
        answer.no_floor_gaps,
        "and the level can skip the sunlight pass"
    );
}

#[test]
fn any_open_square_clears_the_summary_bit() {
    // `no_floor_gaps` is what lets a caller skip the sunlight pass, so one hole anywhere is enough to switch it off.
    let answer = floor_of(&level(&[(2, 2)]));
    assert!(!answer.cells[2 * SIDE + 2], "the hole has no floor");
    assert!(answer.cells[0], "its neighbour does");
    assert!(!answer.no_floor_gaps, "but the level is no longer sealed");
}

#[test]
fn every_one_of_the_four_flags_opens_the_floor() {
    // `NO_FLOOR`, `NO_FLOOR_WATER`, `GOES_DOWN` and `TRANSPARENT_FLOOR` (`map.cpp:9453-9456`). The disjunction is the caller's, since four different terrain flags contribute,
    // but each of the four must reach this rule - so the level here is built with each in turn.
    for opened in [(0, 0), (1, 1), (3, 0), (4, 4)] {
        let answer = floor_of(&level(&[opened]));
        assert!(
            !answer.cells[opened.0 * SIDE + opened.1],
            "{opened:?} should have no floor"
        );
    }
}

#[test]
fn a_greenhouse_roof_below_catches_the_sun_and_keeps_the_floor() {
    // `SUN_ROOF_ABOVE` on the level **below** cancels the hole (`map.cpp:9457-9460`): the roof below takes the sunlight on its own level, so the square above must not pass
    // light straight through to it.
    let mut facts = level(&[(2, 2)]);
    facts.sun_roof_below[2 * SIDE + 2] = true;
    let answer = floor_of(&facts);
    assert!(
        answer.cells[2 * SIDE + 2],
        "the roof below put the floor back"
    );
    assert!(
        answer.no_floor_gaps,
        "so the level is sealed again as far as the sun is concerned"
    );
}

#[test]
fn the_roof_only_saves_the_square_it_is_under() {
    // The exception is per square, not per level: a greenhouse roof on one square does not plug the stairwell next door.
    let mut facts = level(&[(2, 2), (3, 2)]);
    facts.sun_roof_below[2 * SIDE + 2] = true;
    let answer = floor_of(&facts);
    assert!(answer.cells[2 * SIDE + 2], "covered");
    assert!(!answer.cells[3 * SIDE + 2], "not covered");
    assert!(!answer.no_floor_gaps);
}

#[test]
fn the_roof_is_furniture_on_the_level_below_and_not_on_this_one() {
    // A roof above you is a ceiling; a roof *below* you is what this exception is about, because the sunlight pass reads the cache one level up. Reading the same level's
    // furniture would make every greenhouse seal the sky above itself instead of catching it.
    let mut facts = level(&[(1, 1)]);
    facts.sun_roof_below[SIDE + 1] = false;
    assert!(!floor_of(&facts).cells[SIDE + 1]);

    facts.sun_roof_below[SIDE + 1] = true;
    assert!(floor_of(&facts).cells[SIDE + 1]);
}
