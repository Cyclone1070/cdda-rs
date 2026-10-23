//! Which squares are under open sky, and the three-square stamp that decides it.
//!
//! Reference: `map::build_outside_cache( zlev )` (`src/map.cpp:9260-9314`). The rule is small and its consequences are large, because the weather, the sun and the
//! indoors light all ask this one question:
//!
//! * below ground everything is inside, in one fill, with no per-square work at all;
//! * otherwise the cache starts **all outside**, and every square whose terrain **or** furniture has `TFLAG_INDOORS` stamps a **3x3 block of inside** centred on itself.
//!
//! The stamp is the interesting part. It is not "this square is indoors" but "this square and everything within one square of it is indoors", which is what makes a doorway
//! count as indoors without listing it and what stops a single wall tile from being rained on. The reference builds it in a cache **padded by one square** so that the
//! neighbours of an indoor square on the map's edge can be marked without a bounds check - the padding means the edge starts as `true` and the stamp does the rest.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use cdda_core_types::outside_cache::{OutsideFacts, outside_of};

const SIDE: usize = 9;

/// A level of open ground with the given squares marked indoors.
fn level(indoors: &[(usize, usize)]) -> OutsideFacts {
    let mut facts = OutsideFacts {
        side: SIDE,
        indoors: vec![false; SIDE * SIDE],
    };
    for (x, y) in indoors {
        facts.indoors[x * SIDE + y] = true;
    }
    facts
}

#[test]
fn open_ground_is_outside_and_below_ground_is_not() {
    let outside = outside_of(0, &level(&[]));
    assert!(
        outside.iter().all(|sky| *sky),
        "an empty surface level is all sky"
    );

    let below = outside_of(-1, &level(&[]));
    assert!(
        below.iter().all(|sky| !*sky),
        "and the level below is all indoors, whatever the tiles say"
    );
}

#[test]
fn an_indoor_tile_makes_itself_and_its_eight_neighbours_indoors() {
    // The 3x3 stamp (`map.cpp:9294-9300`). A room is indoors, and so is every square touching it - which is how a doorway, a threshold or a wall's own square counts as
    // indoors without any of them being listed.
    let outside = outside_of(0, &level(&[(4, 4)]));
    for dx in -1..=1 {
        for dy in -1..=1 {
            let x = usize::try_from(4 + dx).expect("in range");
            let y = usize::try_from(4 + dy).expect("in range");
            assert!(
                !outside[x * SIDE + y],
                "({x}, {y}) is within one square of the indoor tile"
            );
        }
    }
    assert!(outside[0], "and the far corner is still under the sky");
}

#[test]
fn two_squares_away_is_already_outside_again() {
    // The stamp is exactly three wide. A wall is indoors, the square beside it is indoors, and the square two away is not - which is why a building's light comes in at the
    // windows and not through the wall.
    let outside = outside_of(0, &level(&[(4, 4)]));
    // East of the indoor tile: one square away, then two.
    assert!(!outside[5 * SIDE + 4], "one square away, indoors");
    assert!(
        outside[6 * SIDE + 4],
        "two squares away, under the sky again"
    );
}

#[test]
fn furniture_alone_can_make_a_square_indoors() {
    // `get_ter().has_flag( INDOORS ) || get_furn().has_flag( INDOORS )` (`map.cpp:9291-9293`): an awning, a tent or a roof tile is furniture, and either one is enough.
    // The facts are the disjunction, so this pins that the caller's `||` is what the rule needs.
    let mut facts = OutsideFacts {
        side: SIDE,
        indoors: vec![false; SIDE * SIDE],
    };
    facts.indoors[2 * SIDE + 2] = true;
    assert!(!outside_of(0, &facts)[2 * SIDE + 2]);
}

#[test]
fn an_indoor_tile_in_the_corner_does_not_need_to_read_outside_the_map() {
    // The padded cache (`map.cpp:9271-9275`): the stamp always writes a full 3x3, and at the map's edge two of those squares lie outside it. They are dropped, and the
    // reference does not need a bounds check to do it. A port that wrote through an unpadded index would either panic or wrap to the far edge of the map.
    let outside = outside_of(0, &level(&[(0, 0)]));
    assert!(!outside[0], "the corner itself is indoors");
    assert!(!outside[SIDE], "and its in-map neighbour, one row down");
    assert!(
        outside[(SIDE - 1) * SIDE + (SIDE - 1)],
        "and the padding did not wrap round to the far corner"
    );
}
