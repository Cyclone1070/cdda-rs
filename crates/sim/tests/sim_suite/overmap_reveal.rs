//! What `reveal_map` does to the overmap: the tiles it marks seen, and how far the mark reaches.
//!
//! Reference: `overmapbuffer::reveal( center, radius )` (`overmapbuffer.cpp:880-905`) walks `i` and `j` from `-radius` to `radius`, skips tiles already seen, and marks the rest
//! `om_vision_level::full`. With `trigdist` off - the default - the **square** is what is marked, so a radius of two marks twenty-five tiles and a radius of zero marks one.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

#[test]
fn revealing_marks_a_square_of_tiles_and_never_unmarks_one() {
    let mut world = World::new();
    cdda_sim::overmap::reveal(&mut world, (5, 7), 2);
    let seen = cdda_sim::overmap::seen_tiles(&world);
    assert_eq!(
        seen.len(),
        25,
        "**a radius of two marks twenty-five tiles**: `(2r+1)^2`, because `trigdist` is off and the walk is a square (`overmapbuffer.cpp:887-905`)"
    );
    assert!(
        seen.contains(&(3, 5, 0)) && seen.contains(&(7, 9, 0)),
        "from the corner of the square to the far corner"
    );
    assert!(!seen.contains(&(8, 7, 0)), "and nothing outside it");

    cdda_sim::overmap::reveal(&mut world, (5, 7), 0);
    assert_eq!(
        cdda_sim::overmap::seen_tiles(&world).len(),
        25,
        "**revealing again marks nothing new**, which is the reference's `continue` on a tile already seen"
    );
    cdda_sim::overmap::reveal(&mut world, (40, 40), 0);
    assert_eq!(
        cdda_sim::overmap::seen_tiles(&world).len(),
        26,
        "and a radius of zero reaches exactly the tile it names"
    );
}

/// **The effect reveals around the square the variable names, by the radius it gives.**
///
/// Reference: `f_reveal_map` (`npctalk.cpp:7752-7760`) reads a `var_info` and a `dbl_or_var` radius, projects the square to an overmap tile, and calls
/// `overmap_buffer.reveal`. `EOC_CHECK_MAP_CACHE` is the data's own user: `"radius": { "math": [ "rng(11, 36)" ] }`.
#[test]
fn the_reveal_effect_marks_the_tiles_around_its_variable() {
    use cdda_content::dialogue::{AssignmentTarget, Effect};

    let mut world = World::new();
    let actor = world
        .spawn(cdda_sim::Position { x: 0, y: 0, z: 0 })
        .id();
    // An overmap tile is twenty-four squares across, so a square at 30, 30 is overmap tile (1, 1).
    cdda_sim::dialogue_values::set(
        &mut world,
        actor,
        "place",
        cdda_sim::dialogue_values::Value::Position { x: 30, y: 30, z: 0 },
    );
    let effect = Effect::RevealMap {
        target: AssignmentTarget::Variable {
            scope: "context_val".to_owned(),
            name: "place".to_owned(),
        },
        radius: None,
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, actor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect is answered"
    );
    assert_eq!(
        cdda_sim::overmap::seen_tiles(&world),
        vec![(1, 1, 0)],
        "**with no radius it reveals exactly the tile the square belongs to**"
    );
}
