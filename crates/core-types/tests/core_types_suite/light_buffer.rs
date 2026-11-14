//! Buffered light sources: the reference records them first and casts them second, and casts a square's rays only into the directions no brighter neighbour already covers.
//!
//! Source: `map::add_light_source` (`src/lightmap.cpp:666-670`), the gathering loop's second phase (`:654-659`), and `map::apply_light_source`'s neighbour test
//! (`:1257-1290`).

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_core_types::level::Level;
use cdda_core_types::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    level
}

fn light_of(
    level: &mut Level,
    recorded: &[(i32, i32, f32)],
    applied: &[(i32, i32, f32)],
) -> BTreeMap<(i32, i32), FourQuadrants> {
    let clear = |_x: i32, _y: i32| cdda_core_types::transparency::SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: false,
        fields: Vec::new(),
    };
    for (x, y, luminance) in recorded {
        level.add_light_source((*x, *y), *luminance);
    }
    let mut light = BTreeMap::new();
    level.apply_buffered_sources(1.0, clear, &mut light);
    for (x, y, luminance) in applied {
        level.apply_light_source_with((*x, *y), *luminance, 1.0, clear, &mut light);
    }
    light
}

#[test]
fn a_buffered_source_waits_to_be_applied() {
    // `add_light_source` only records (`lightmap.cpp:666-670`); the cast happens once the gathering loop has walked the whole map (`:654-659`). A port that casts on the way
    // cannot ask "is there a brighter neighbour", because the neighbour may not have been seen yet.
    let mut level = level();
    let clear = |_x: i32, _y: i32| cdda_core_types::transparency::SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: false,
        fields: Vec::new(),
    };
    level.add_light_source((5, 5), 20.0);
    let mut light = BTreeMap::new();
    level.apply_buffered_sources(1.0, clear, &mut light);
    assert!(
        light.contains_key(&(5, 5)),
        "the source lights once applied"
    );
    assert!(light.contains_key(&(8, 5)), "and casts into the room");
}

#[test]
fn a_neighbour_at_least_as_bright_covers_the_direction_so_the_rays_are_not_cast() {
    // The rule (`lightmap.cpp:1257-1290`): a square does not cast rays to the east when its eastern neighbour is already lit by a buffered source of the same luminance or
    // more, because that neighbour's own cast reaches the squares beyond. The observable consequence: the light past the pair comes from the far source alone.
    let mut pair = level();
    let together = light_of(&mut pair, &[(5, 5, 20.0), (6, 5, 20.0)], &[]);

    let mut alone = level();
    let far_only = light_of(&mut alone, &[(6, 5, 20.0)], &[]);

    assert_eq!(
        together
            .get(&(9, 5))
            .copied()
            .unwrap_or_else(|| FourQuadrants::uniform(0.0))
            .max(),
        far_only
            .get(&(9, 5))
            .copied()
            .unwrap_or_else(|| FourQuadrants::uniform(0.0))
            .max(),
        "the eastern source alone reaches the square, and the western one adds nothing east of it"
    );
    assert!(
        together.contains_key(&(4, 5)),
        "and the western source still casts west"
    );
}

#[test]
fn an_unbuffered_source_casts_every_direction_and_is_not_recorded() {
    // "If there's a 100 luminance magnesium flare south added via `apply_light_source` instead of `add_light_source`, it's unbuffered so we'll still cast rays into sy"
    // (`lightmap.cpp:1244-1246`). Items and creatures are applied directly, so they neither consult the buffer nor fill it.
    let mut level = level();
    let clear = |_x: i32, _y: i32| cdda_core_types::transparency::SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: false,
        fields: Vec::new(),
    };
    level.add_light_source((6, 5), 20.0);
    let mut light = BTreeMap::new();
    level.apply_buffered_sources(1.0, clear, &mut light);
    let after_buffered = light
        .get(&(9, 5))
        .copied()
        .unwrap_or_else(|| FourQuadrants::uniform(0.0))
        .max();

    level.apply_light_source_with((8, 5), 20.0, 1.0, clear, &mut light);
    let after_unbuffered = light
        .get(&(10, 5))
        .copied()
        .unwrap_or_else(|| FourQuadrants::uniform(0.0))
        .max();

    assert!(after_buffered > 0.0);
    assert!(
        after_unbuffered > 0.0,
        "the unbuffered source lights its own neighbourhood"
    );
}

#[test]
fn the_buffer_is_emptied_once_it_has_been_applied() {
    // The reference fills the buffer with zeroes at the start of every lightmap build (`lightmap.cpp:463-464`), so a fire that went out leaves nothing behind. A buffer that
    // kept its entries would relight the room from a source the world no longer has.
    let mut level = level();
    let clear = |_x: i32, _y: i32| cdda_core_types::transparency::SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: false,
        fields: Vec::new(),
    };
    level.add_light_source((5, 5), 20.0);
    assert_eq!(level.buffered_sources(), 1);
    let mut light = BTreeMap::new();
    level.apply_buffered_sources(1.0, clear, &mut light);
    assert_eq!(level.buffered_sources(), 0, "drained by applying");

    let mut second = BTreeMap::new();
    level.apply_buffered_sources(1.0, clear, &mut second);
    assert!(second.is_empty(), "and a second pass has nothing to cast");
}
