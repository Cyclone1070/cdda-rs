//! Pouring a field out from one square.
//!
//! Reference: `map::propagate_field` (`map_field.cpp:2226-2299`). The rule that matters is that a blast thickens the squares
//! it can reach **evenly** and never above the field's maximum: an emission of nine of something at a maximum of three fills
//! three squares to the cap, not one square to nine.

#![allow(clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldId;
use cdda_core_types::rng::Rolls;
use cdda_sim::field_spread;

const GAS: FieldId = FieldId(20);

fn strength(world: &World, x: i32, y: i32) -> i32 {
    cdda_sim::field_strength_at(world, (x, y, 0), GAS)
}

#[test]
fn a_field_poured_out_stops_at_its_maximum_and_spends_what_it_has() {
    let mut world = World::new();
    let mut rolls = Rolls::new(3);
    let placed = field_spread::propagate(
        &mut world,
        (0, 0, 0),
        GAS,
        9,
        3,
        &mut rolls,
        |_x, _y, _z| true,
    );
    assert_eq!(placed, 9, "every unit of the amount was placed");

    let mut total = 0;
    let mut on_the_centre = 0;
    for x in -3..=3 {
        for y in -3..=3 {
            let here = strength(&world, x, y);
            assert!(here <= 3, "no square goes above the maximum, and ({x},{y}) is at {here}");
            total += here;
            if (x, y) == (0, 0) {
                on_the_centre = here;
            }
        }
    }
    assert_eq!(total, 9, "the field holds exactly what was poured");
    assert_eq!(on_the_centre, 3, "and the centre took the maximum first");
}

#[test]
fn a_wall_the_field_cannot_enter_stops_it() {
    let mut world = World::new();
    let mut rolls = Rolls::new(3);
    // Everything past x = 0 is impassable, so nothing can reach the far side.
    field_spread::propagate(
        &mut world,
        (0, 0, 0),
        GAS,
        40,
        3,
        &mut rolls,
        |x, _y, _z| x <= 0,
    );
    assert!(strength(&world, 0, 0) > 0, "the near side holds the gas");
    let far: i32 = (1..=6).map(|x| strength(&world, x, 0)).sum();
    assert_eq!(far, 0, "and the far side is untouched");
}

#[test]
fn pouring_nothing_places_nothing() {
    let mut world = World::new();
    let mut rolls = Rolls::new(3);
    assert_eq!(
        field_spread::propagate(&mut world, (0, 0, 0), GAS, 0, 3, &mut rolls, |_, _, _| true),
        0
    );
    assert_eq!(strength(&world, 0, 0), 0);
}

/// A gas stops at a wall, and passes a permeable one.
///
/// Reference: `map::propagate_field` (`map_field.cpp:2264-2270`) refuses a square that is impassable unless the terrain carries
/// `TFLAG_PERMEABLE`. The distinction is the whole reason a window stops nothing and a wall stops everything.
#[test]
fn a_field_passes_a_permeable_square_and_not_a_solid_one() {
    let mut world = World::new();
    // A wall down the middle at x = 1: permeable on the row the gas is spreading along, solid everywhere else.
    for y in -3..=3 {
        cdda_sim::field_passage::set_passage(
            &mut world,
            (1, y, 0),
            cdda_sim::field_passage::FieldPassage {
                impassable: true,
                permeable: y == 0,
            },
        );
    }
    let mut rolls = Rolls::new(11);
    // Enough gas to travel well past the wall: a square costs at least one unit, so 400 units reach the far side wherever the
    // way is open, and the assertion does not depend on where in a front the draw happens to stop.
    let blocked = cdda_sim::field_passage::blocked_within(&world, (0, 0, 0), 400);
    assert!(!blocked.is_empty(), "the solid part of the wall is collected");
    field_spread::propagate(
        &mut world,
        (0, 0, 0),
        GAS,
        400,
        3,
        &mut rolls,
        |x, y, z| !blocked.contains(&(x, y, z)),
    );

    assert!(
        strength(&world, 1, 0) > 0,
        "the permeable square holds gas - a screen door stops nothing"
    );
    assert_eq!(
        strength(&world, 1, 1),
        0,
        "the solid part of the wall holds it back"
    );
    assert!(
        strength(&world, 2, 0) > 0,
        "the permeable square on the gas's own row lets it through"
    );
    // A wall is not a seal: the spread is a flood over eight neighbours, so gas that cannot enter a square walks around the end
    // of one. What the reference's rule does is refuse **entry**, and that is what can be asserted - the solid squares stay
    // empty while the permeable one fills.
    for y in [1, 2, 3] {
        assert_eq!(
            strength(&world, 1, y),
            0,
            "the solid part of the wall holds it back at ({}, {y})",
            1
        );
    }
}
