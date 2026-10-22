//! The ring's order is the reference's, transcribed rather than inferred.
//!
//! Reference: `point.h:450`.
//!
//! ```text
//! eight_horizontal_neighbors = { north_west, north, north_east, west, east, south_west, south, south_east }
//! ```
//!
//! It is not a clockwise walk: west comes before east, and the southern row runs south-west, south, south-east. The order decides which square each die is
//! spent on, so a replay notices it - but only when two neighbours could catch and the dice disagree between them. A fixture is a thin guard for a fact this
//! small, and a fact this small is exactly what a transcript pins cheaply.

#![allow(clippy::panic)]

#[test]
fn the_eight_neighbours_are_in_the_reference_order() {
    assert_eq!(
        cdda_sim::EIGHT_NEIGHBOURS,
        [
            (-1, -1), // north-west
            (0, -1),  // north
            (1, -1),  // north-east
            (-1, 0),  // west
            (1, 0),   // east
            (-1, 1),  // south-west
            (0, 1),   // south
            (1, 1),   // south-east
        ]
    );
}
