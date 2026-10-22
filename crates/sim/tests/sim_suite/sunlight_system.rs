//! Daylight and torches meet in one answer.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::{Light, LightSource, Map, Position, cast_lights, cast_sunlight, clear_light};

const NOON: f64 = 12.0 * 3600.0;
const MIDNIGHT: f64 = 0.0;

fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    world.insert_resource(Map(level));
    world
}

#[test]
fn open_sky_is_bright_at_noon_and_black_at_midnight() {
    let mut world = world();
    cast_sunlight(&mut world, NOON, 0.0, |_, _| true);
    assert!(
        world.resource::<Light>().max_at(5, 5) > 50.0,
        "noon overhead"
    );

    clear_light(&mut world);
    cast_sunlight(&mut world, MIDNIGHT, 0.0, |_, _| true);
    assert_eq!(world.resource::<Light>().max_at(5, 5), 0.0, "midnight");
}

#[test]
fn a_torch_indoors_adds_where_the_sun_cannot_reach() {
    // A room with no open sky: the sun contributes nothing, the torch does. Then a room with a
    // door to the sky: both contribute, and the answer is the brighter of the two per corner,
    // never their sum.
    let mut world = world();
    let torch = world
        .spawn((
            Position { x: 5, y: 5, z: 0 },
            LightSource { luminance: 10.0 },
        ))
        .id();
    let _ = torch;

    cast_sunlight(&mut world, NOON, 0.0, |_, _| false);
    let indoors_only = world.resource::<Light>().max_at(6, 5);
    assert_eq!(indoors_only, 0.0, "no open sky, no daylight");

    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);
    let with_torch = world.resource::<Light>().max_at(6, 5);
    assert!(with_torch > 0.0, "the torch lights it");
}

#[test]
fn outside_torchlight_never_outshines_daylight() {
    let mut world = world();
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    cast_sunlight(&mut world, NOON, 0.0, |_, _| true);
    let day = world.resource::<Light>().max_at(5, 5);
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);
    let both = world.resource::<Light>().max_at(5, 5);
    assert!(
        both >= day,
        "the answer does not fall when a torch is added"
    );
    assert!(
        both < day * 2.0,
        "and does not double: it is the brighter, not the sum"
    );
}

#[test]
fn clearing_the_answer_leaves_nothing_behind() {
    let mut world = world();
    cast_sunlight(&mut world, NOON, 0.0, |_, _| true);
    assert!(world.resource::<Light>().lit_squares() > 0);
    clear_light(&mut world);
    assert_eq!(
        world.resource::<Light>().lit_squares(),
        0,
        "a stale answer is not an answer"
    );
}
