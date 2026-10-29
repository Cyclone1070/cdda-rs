//! Falling as part of the turn, in the reference's own position in it.
//!
//! Reference: `game::do_turn` (`src/do_turn.cpp:642-655`) runs the scent, then `map::build_floor_caches`, then `m.process_falling()`, and only then `m.process_fields()`.
//! The order is not cosmetic: a thing that falls is aged by the same turn's field sweep rather than by the next one, and a square whose support went away is asked once
//! because `process_falling` moves the queue out and clears it.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::fire::{Ground, Neighbour};
use cdda_core_types::rng::Rolls;
use cdda_sim::falling::FieldRules;
use cdda_sim::{
    queue_support_loss, run_environment_turn, set_field_rules, support_lost, Field, FireMap, GroundItem, ItemTypeId, Position,
};

/// A map with one hole in it: the squares in `holes` have no floor, everything else does.
struct Holed {
    holes: Vec<(i32, i32, i32)>,
}

impl FireMap for Holed {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn ground(&self, _square: (i32, i32, i32)) -> Ground {
        Ground::Inert
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn no_floor(&self, square: (i32, i32, i32)) -> bool {
        self.holes.contains(&square)
    }
}

fn item_at(world: &mut World, x: i32, y: i32, z: i32) -> Entity {
    world
        .spawn((
            Position { x, y, z },
            GroundItem {
                kind: ItemTypeId(1),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ))
        .id()
}

fn square_of(world: &World, entity: Entity) -> (i32, i32, i32) {
    let position = world.get::<Position>(entity).expect("the item exists");
    (position.x, position.y, position.z)
}

/// A turn with nothing on fire and nothing spreading, which is what these fixtures need.
fn quiet_turn(world: &mut World, map: &impl FireMap) {
    let mut rolls = Rolls::new(1);
    run_environment_turn(
        world,
        map,
        0,
        (0, 0),
        0,
        |_, _| false,
        &mut rolls,
    );
}

#[test]
fn an_item_above_a_hole_falls_in_the_turn_and_the_queue_is_emptied() {
    let map = Holed { holes: Vec::new() };
    let mut world = World::new();
    cdda_sim::fire_facts::set_fire_facts(
        &mut world,
        (0, 0, 0),
        cdda_sim::TerrainFacts { no_floor: true, ..cdda_sim::TerrainFacts::default() },
    );
    let item = item_at(&mut world, 0, 0, 0);
    queue_support_loss(&mut world, (0, 0, 0));
    quiet_turn(&mut world, &map);
    // The floor under the hole is the level below, one step down: the map says -1 has a floor.
    assert_eq!(square_of(&world, item), (0, 0, -1), "the item fell one level");
    assert!(
        support_lost(&mut world).squares.is_empty(),
        "and the queue was emptied, so a square is asked once"
    );
    // A second turn moves nothing, which is the whole difference between a queue and a standing instruction.
    quiet_turn(&mut world, &map);
    assert_eq!(square_of(&world, item), (0, 0, -1), "nothing falls twice");
}

#[test]
fn an_item_on_a_square_with_a_floor_does_not_fall() {
    let map = Holed { holes: vec![] };
    let mut world = World::new();
    let item = item_at(&mut world, 0, 0, 0);
    queue_support_loss(&mut world, (0, 0, 0));
    quiet_turn(&mut world, &map);
    assert_eq!(square_of(&world, item), (0, 0, 0), "the square holds it up");
    assert!(
        support_lost(&mut world).squares.is_empty(),
        "and asking the question spends the square either way"
    );
}

#[test]
fn a_cosmetic_field_above_a_hole_falls_with_the_item() {
    let map = Holed { holes: Vec::new() };
    let mut world = World::new();
    cdda_sim::fire_facts::set_fire_facts(
        &mut world,
        (30, 30, 0),
        cdda_sim::TerrainFacts { no_floor: true, ..cdda_sim::TerrainFacts::default() },
    );
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(1);
    let smoke = FieldId(7);
    let field = world
        .spawn((
            Position {
                x: 30,
                y: 30,
                z: 0,
            },
            Field {
                id: smoke,
                entry,
                priority: 0,
            },
        ))
        .id();
    let item = item_at(&mut world, 30, 30, 0);
    set_field_rules(
        &mut world,
        FieldRules {
            falls: BTreeSet::from([smoke]),
            gas: BTreeSet::from([smoke]),
        },
    );
    queue_support_loss(&mut world, (30, 30, 0));
    quiet_turn(&mut world, &map);
    let field_at = world.get::<Position>(field).expect("the field exists");
    assert_eq!(
        (field_at.x, field_at.y, field_at.z),
        (30, 30, -1),
        "one level down"
    );
    assert_eq!(square_of(&world, item), (30, 30, -1), "and the item with it");
}
