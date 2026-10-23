//! What each terrain costs the light that crosses it, taken from the loaded prototypes.
//!
//! Reference: `map::build_transparency_cache`'s `calc_transp` (`src/lightmap.cpp:140-150`), which asks one question of the terrain - is it `transparent` - and answers
//! `LIGHT_TRANSPARENCY_SOLID` or the open-air value. Everything else the cache does (fields, weather, furniture) is layered on top of that answer elsewhere.
//!
//! The sim used to hard-code the rule: terrain code zero was solid and every other code was open air, because the per-terrain `transparent` bool was not loaded. It is now
//! loaded (`cdda_content::mapdata::MapData::transparent`, `cdda_content::terrain::Terrain::transparent`), so the rule can be data: a window passes light, a wall does not,
//! and the difference is one field in a JSON file rather than a code in a match arm.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Field, Light, LightSource, Map, Position, cast_lights, cast_lights_with_furniture};

/// A level whose square `(5, 5)` is the given terrain code, with a lamp west of it.
fn level_with(code: u32) -> World {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    level.set_terrain(5, 5, code, 1);
    world.insert_resource(Map(level));
    world.spawn((
        Position { x: 2, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    world
}

fn light_past_the_square(world: &mut World, table: &TerrainTable) -> f32 {
    cast_lights(
        world,
        |code| table.transparency(code),
        |_id, _intensity| 0.0,
    );
    world.resource::<Light>().max_at(8, 5)
}

#[test]
fn a_wall_stops_the_light_and_a_window_does_not() {
    // The whole point of loading the bool: `t_wall` and `t_window` differ by one flag in the data, and the light map should show it without the sim knowing either id.
    let mut table = TerrainTable::new();
    table.insert(1, true); // the floor
    table.insert(2, false); // a wall
    table.insert(3, true); // a window

    let mut through_floor = level_with(1);
    let floor = light_past_the_square(&mut through_floor, &table);

    let mut through_wall = level_with(2);
    let wall = light_past_the_square(&mut through_wall, &table);

    let mut through_window = level_with(3);
    let window = light_past_the_square(&mut through_window, &table);

    assert!(floor > 0.0, "the reference case: nothing in the way");
    assert!(
        wall < floor * 0.5,
        "the wall shades the far side: {wall} against {floor}"
    );
    assert!(
        (window - floor).abs() < 1e-4,
        "and the window costs the light nothing, as a transparent terrain does: {window} against {floor}"
    );
}

#[test]
fn a_transparent_terrain_costs_exactly_the_open_air_value() {
    // `calc_transp` does not interpolate: a transparent terrain is `LIGHT_TRANSPARENCY_OPEN_AIR` and anything else is solid (`lightmap.cpp:144-150`). There is no middle,
    // which is why a window dims nothing and a wall stops everything.
    let mut table = TerrainTable::new();
    table.insert(1, true);
    table.insert(2, false);
    assert!((table.transparency(1) - TRANSPARENCY_OPEN_AIR).abs() < 1e-6);
    assert!(
        table.transparency(2).abs() < 1e-6,
        "an opaque terrain costs everything"
    );
}

#[test]
fn a_code_the_table_has_never_seen_is_treated_as_open_air() {
    // Codes come from whatever built the level, and a table that has not been told about one must not black out the map: the older rule treated every code but zero as
    // air, and this keeps that behaviour for the unknown case while letting a known code be opaque.
    let mut table = TerrainTable::new();
    table.insert(2, false);
    assert!((table.transparency(99) - TRANSPARENCY_OPEN_AIR).abs() < 1e-6);
}

#[test]
fn the_table_is_built_from_loaded_prototypes_by_asking_which_are_transparent() {
    // The seam between the loader and the light: the caller decides which code stands for which prototype, and the table takes the bool from the data.
    let loaded = [
        ("t_floor", true),
        ("t_wall", false),
        ("t_window", true),
        ("f_null", true),
    ];
    let codes: std::collections::BTreeMap<&str, u32> = loaded
        .iter()
        .enumerate()
        .map(|(index, (id, _))| {
            #[allow(clippy::cast_possible_truncation)]
            (*id, index as u32 + 1)
        })
        .collect();
    let table = TerrainTable::from_prototypes(
        loaded
            .iter()
            .map(|(id, transparent)| (codes.get(id).copied().unwrap_or(0), *transparent)),
    );
    let code_of = |id: &str| codes.get(id).copied().expect("the id is in the table");
    assert!(table.transparency(code_of("t_floor")) > 0.0);
    assert!(table.transparency(code_of("t_wall")).abs() < 1e-6);
    assert!(table.transparency(code_of("t_window")) > 0.0);
}

#[test]
fn smoke_still_dims_a_ray_through_a_window() {
    // The two layers compose: the terrain answer is the base and the field multiplies it, so a smoked window is dimmer than a clear floor even though both are transparent
    // terrain.
    let mut table = TerrainTable::new();
    table.insert(1, true);

    let mut clear = level_with(1);
    cast_lights(&mut clear, |code| table.transparency(code), |_id, _i| 0.0);
    let open = clear.resource::<Light>().max_at(8, 5);

    let mut smoked = level_with(1);
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(3);
    smoked.spawn((
        Position { x: 5, y: 5, z: 0 },
        Field {
            id: FieldId(1),
            entry,
            priority: 0,
        },
    ));
    cast_lights(&mut smoked, |code| table.transparency(code), |_id, _i| 10.0);
    let dimmed = smoked.resource::<Light>().max_at(8, 5);

    assert!(
        dimmed < open,
        "smoke on the square dims what is behind it: {dimmed} against {open}"
    );
}

#[test]
fn an_opaque_table_on_a_glass_floor_is_opaque() {
    // `calc_transp` asks for `get_ter().transparent && get_furn().transparent` (`lightmap.cpp:146-149`) - a **conjunction**, so one opaque layer is enough and the floor
    // underneath being see-through does not save the ray. This is the case the sim could not express while it passed `furniture_transparent: true` unconditionally.
    let mut table = TerrainTable::new();
    table.insert(1, true); // a glass floor
    table.insert(9, false); // a table

    let open_floor = {
        let mut world = level_with(1);
        cast_lights(&mut world, |code| table.transparency(code), |_id, _i| 0.0);
        world.resource::<Light>().max_at(8, 5)
    };

    let behind_the_table = {
        let mut world = level_with(1);
        world.resource_mut::<Map>().0.set_furniture(5, 5, 9, 1);
        cast_lights_with_furniture(
            &mut world,
            |code| table.transparency(code),
            |code| table.transparency(code),
            |_id, _i| 0.0,
        );
        world.resource::<Light>().max_at(8, 5)
    };

    assert!(open_floor > 0.0, "the glass floor passes the light");
    assert!(
        behind_the_table < open_floor * 0.5,
        "and the table on it does not: {behind_the_table} against {open_floor}"
    );
}

#[test]
fn furniture_that_is_see_through_does_not_stop_the_ray() {
    // The other half of the conjunction: a wire frame on a glass floor is still transparent, so the rule must not become "any furniture blocks".
    let mut table = TerrainTable::new();
    table.insert(1, true);
    table.insert(8, true); // a wire frame

    let open_floor = {
        let mut world = level_with(1);
        cast_lights(&mut world, |code| table.transparency(code), |_id, _i| 0.0);
        world.resource::<Light>().max_at(8, 5)
    };

    let through = {
        let mut world = level_with(1);
        world.resource_mut::<Map>().0.set_furniture(5, 5, 8, 1);
        cast_lights_with_furniture(
            &mut world,
            |code| table.transparency(code),
            |code| table.transparency(code),
            |_id, _i| 0.0,
        );
        world.resource::<Light>().max_at(8, 5)
    };

    assert!(
        (through - open_floor).abs() < 1e-4,
        "a see-through frame costs nothing: {through} against {open_floor}"
    );
}
