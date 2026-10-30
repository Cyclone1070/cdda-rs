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
