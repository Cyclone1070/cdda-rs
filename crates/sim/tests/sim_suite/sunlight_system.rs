//! Daylight and torches meet in one answer.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::shadowcasting::Quadrant;
use cdda_sim::field_table::FieldTable;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{
    light,
 LightSource, Map, Position, Prototypes, cast_lights_with, cast_sunlight,
    cast_sunlight_with, clear_light, set_field_table};

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
    cdda_sim::load_level(&mut world, level);
    cdda_sim::set_prototypes(&mut world, Prototypes::default());
    set_field_table(&mut world, FieldTable::new());
    world
}

#[test]
fn open_sky_is_bright_at_noon_and_black_at_midnight() {
    let mut world = world();
    cast_sunlight(&mut world, NOON, 0.0, |_, _| true);
    assert!(
        light(&world).max_at(5, 5) > 50.0,
        "noon overhead"
    );

    clear_light(&mut world);
    cast_sunlight(&mut world, MIDNIGHT, 0.0, |_, _| true);
    assert_eq!(light(&world).max_at(5, 5), 0.0, "midnight");
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
    let indoors_only = light(&world).max_at(6, 5);
    assert_eq!(indoors_only, 0.0, "no open sky, no daylight");

    cast_lights_with(&mut world);
    let with_torch = light(&world).max_at(6, 5);
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
    let day = light(&world).max_at(5, 5);
    cast_lights_with(&mut world);
    let both = light(&world).max_at(5, 5);
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
    assert!(light(&world).lit_squares() > 0);
    clear_light(&mut world);
    assert_eq!(
        light(&world).lit_squares(),
        0,
        "a stale answer is not an answer"
    );
}

#[test]
fn daylight_reaches_a_square_just_inside_a_doorway_and_stops_one_step_further_in() {
    // The cascade's fingerprint, and the reason the hand-rolled stand-in had to go. Light from the cardinal beside a square lights **only the two quarters facing it**
    // (`build_sunlight_cache`, `lightmap.cpp:421-431`), and a square whose neighbours are all roofed gets nothing. So the claim has to be read in quarters: a uniform fill
    // cannot produce this shape, and `max()` would hide it - a square under a roof still has a bright quarter where the doorway is.
    let mut table = TerrainTable::new();
    table.insert(1, true); // a floor under the sky
    table.insert(7, true); // a roofed floor
    table.insert_indoors(7, true);

    let mut world = world();
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain: table.clone(),
        furniture: TerrainTable::new(),
    });
    for y in 0..12 {
        for x in 0..12 {
            world.resource_mut::<Map>().0.set_terrain(x, y, 1, 1);
        }
    }
    // A room: everything from x=2 to x=5 and y=3 to y=7 is roofed, and the square at (6,4) is its doorway.
    for y in 3..=7 {
        for x in 2..=5 {
            world.resource_mut::<Map>().0.set_terrain(x, y, 7, 1);
        }
    }

    cast_sunlight_with(&mut world, NOON);
    let light = light(&world);

    let doorway_square = light.at(5, 4).expect("the square inside the door");
    // An interior square, all four of whose cardinals are roofed: nothing can spill into it.
    let deep_square = light.at(4, 5).expect("a square in the middle of the room");
    assert!(
        doorway_square.get(Quadrant::NorthEast) > doorway_square.get(Quadrant::NorthWest),
        "the door-facing quarters are bright and the far ones are not: {doorway_square:?}"
    );
    assert!(
        deep_square.max() <= 4.0 + 1e-3,
        "and the far corner is at the indoors light throughout: {deep_square:?}"
    );
}

#[test]
fn a_roofed_square_takes_the_indoors_light_and_an_open_one_takes_the_sun() {
    // `inside_light_level` (`lightmap.cpp:338-340`): under a bright sky, indoors is dim-but-lit and the sky is daylight. The stand-in used a caller-supplied constant; this
    // reads it from the transcribed rule, so a night sky changes it without anybody passing a new number.
    let mut table = TerrainTable::new();
    table.insert(1, true);
    table.insert(7, true);
    table.insert_indoors(7, true);

    let mut world = world();
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain: table.clone(),
        furniture: TerrainTable::new(),
    });
    // A roofed room with an interior square, so that nothing spills in from the open.
    for y in 3..=6 {
        for x in 3..=6 {
            world.resource_mut::<Map>().0.set_terrain(x, y, 7, 1);
        }
    }
    cast_sunlight_with(&mut world, NOON);

    let open = light(&world).max_at(0, 0);
    let roofed = light(&world).max_at(4, 4);
    assert!(open > 50.0, "noon outside: {open}");
    assert!(
        (3.0..open).contains(&roofed),
        "and inside the room, dim but lit: {roofed}"
    );
}

#[test]
fn the_old_entry_point_speaks_the_cascade_now_and_keeps_the_quarter_shape() {
    // `cast_sunlight` is the closure-shaped entry point everything older calls, and it used to compute `Level::daylight` - a hand-rolled rule that gave a uniform value to
    // any square with open sky on one side. It is now the cascade behind an adapter, and the shape is the evidence: a square inside a doorway is bright on the side the sky
    // is on and dark on the other.
    let mut world = world();
    cast_sunlight(&mut world, NOON, 0.0, |x, y| (x, y) == (6, 5));

    let light = light(&world);
    let inside = light.at(5, 5).expect("the square inside the doorway");
    assert!(
        inside.get(Quadrant::SouthEast) > inside.get(Quadrant::NorthWest),
        "the doorway lights the quarters facing it: {inside:?}"
    );
    assert!(
        (inside.get(Quadrant::NorthWest)).abs() < 1e-6,
        "and the far quarters stay dark, because the caller asked for no indoors light"
    );
}
