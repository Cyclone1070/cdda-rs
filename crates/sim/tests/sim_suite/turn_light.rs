//! A turn leaves the light map answering for the world it just made.
//!
//! Source: `game::draw`'s `map::build_map_cache` (`src/map.cpp:9550-9570`) running **after** the turn's own work, so the frame a player sees at the end of a turn is the
//! light of that turn's fires and not of the turn before it.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::field_table::FieldTable;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Field, FireMap, Map, Position, Prototypes, run_environment_turn, set_field_table};
use cdda_sim::{light};

const FLOOR: u32 = 1;
const FIRE: u32 = 2;

/// Every square takes what is offered: the spread rule's `can_take`, which this test is not about.
fn every_square_takes(_from: (i32, i32, i32), _to: (i32, i32, i32)) -> bool {
    true
}

/// A floor that will not burn, so the only field in play is the one the test puts there.
struct HardFloor;

impl FireMap for HardFloor {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn ground(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        cdda_core_types::fire::Ground::Inert
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour {
        // Nothing to catch: this test is about light, not about the fire spreading.
        cdda_core_types::fire::Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
}

fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    world.insert_resource(Map(level));
    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    world.insert_resource(Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });
    let mut fields = FieldTable::new();
    fields.insert(FIRE, &[0.7, 0.4, 0.1]);
    fields.insert_light(FIRE, &[20.0, 60.0, 160.0]);
    set_field_table(&mut world, fields);
    world
}

fn spawn_fire(world: &mut World, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 30, false, true);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position { x: 5, y: 5, z: 0 },
            Field {
                id: FieldId(FIRE),
                entry,
                priority: 0,
            },
        ))
        .id()
}

#[test]
fn a_fire_burning_through_a_turn_is_lit_at_the_end_of_it() {
    // The light map is rebuilt after the turn (the reference's `build_map_cache` in the draw path), so the fire's own square answers with the fire's light in the same turn
    // that the fire burned. Before this, `run_environment_turn` left whatever the previous cast had written, and a fire that started this turn was invisible until
    // something else asked for light.
    let mut world = world();
    spawn_fire(&mut world, 3);
    let mut rolls = Rolls::new(1);
    run_environment_turn(&mut world, &HardFloor, 0, (5, 5), 0, every_square_takes, &mut rolls);

    let lit = light(&world).max_at(5, 5);
    assert!(lit > 50.0, "the fire lights itself: {lit}");
}

#[test]
fn a_square_whose_fire_is_gone_is_dark_at_the_end_of_the_next_turn() {
    // The rebuild clears before it casts. A merge by maximum would keep the brighter answer from the turn before, which is the mistake the smoke test caught once already and
    // is worth pinning where a whole turn is involved rather than a single cast.
    let mut world = world();
    let fire = spawn_fire(&mut world, 3);
    let mut rolls = Rolls::new(1);
    run_environment_turn(&mut world, &HardFloor, 0, (5, 5), 0, every_square_takes, &mut rolls);
    assert!(light(&world).max_at(5, 5) > 50.0, "lit while it burns");

    world.despawn(fire);
    run_environment_turn(&mut world, &HardFloor, 0, (5, 5), 0, every_square_takes, &mut rolls);
    assert_eq!(
        light(&world).max_at(5, 5),
        0.0,
        "and dark once it is gone"
    );
}
