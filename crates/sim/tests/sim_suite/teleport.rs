//! Where a teleport lands, and what happens when it lands somewhere solid.
//!
//! Reference: `teleport::teleport_creature` and `teleport::teleport_to_point` (`teleport.cpp:152-260`). The draws are the store's own, so a gate can hold them still and pin the
//! arithmetic: an angle of zero is `cos( 0 )` of one and `sin( 0 )` of zero, which puts the creature exactly `distance` squares east whatever the platform's libm does.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A world of one floor code with move cost two, so every square inside it is standable and nothing outside has terrain at all.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    // A square's standability is its move cost, the same question a route asks: without one the search refuses the square and so does a teleport.
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let who = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    (world, who)
}

#[test]
fn a_teleport_draws_an_angle_and_a_distance_and_moves_the_creature_exactly_there() {
    let (mut world, who) = a_world();
    // The reference draws `rng( 0, 360 )` and then `rng( min, max )`; with a generator that answers the bottom of every range, the angle is zero and the distance is the
    // minimum, which is the one pair whose arithmetic cannot differ between platforms.
    let outcome = cdda_sim::teleport::creature(&mut world, who, 3, 5, true);
    assert_eq!(
        outcome,
        cdda_sim::teleport::Teleport::Moved {
            from: (0, 0, 0),
            to: (3, 0, 0)
        },
        "**an angle of zero sends the creature east by the distance drawn**, and the distance drawn is the minimum"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(who).map(|at| (at.x, at.y, at.z)),
        Some((3, 0, 0)),
        "and the store says so"
    );
}

/// **A safe teleport into a wall refuses; an unsafe one is death.**
///
/// Reference: `teleport::teleport_to_point` (`teleport.cpp:198-260`): the same square is no teleport, a solid landing returns false with a message when the jump is safe, and
/// without safety the creature takes 9,999 damage to the torso - the reference's way of saying there is nothing left to find.
#[test]
fn landing_in_a_wall_refuses_if_safe_and_kills_if_not() {
    let (mut world, who) = a_world();
    // A square with no terrain at all is impassable, which is how the port answers the reference's `impassable`.
    let outside = (100, 100, 0);
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, outside, true),
        cdda_sim::teleport::Teleport::Refused,
        "**a safe teleport into solid rock does nothing at all**"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(who).map(|at| (at.x, at.y, at.z)),
        Some((0, 0, 0)),
        "and the creature has not moved"
    );
    world.entity_mut(who).insert(cdda_sim::creature::Health { hp: 40, max_hp: 40 });
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, outside, false),
        cdda_sim::teleport::Teleport::Died,
        "an unsafe one kills"
    );
    assert!(
        cdda_sim::creature::health_of(&world, who).is_some_and(|hp| hp <= 0),
        "**and the creature is dead where it stood**, not moved and not merely hurt"
    );
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (0, 0, 0), true),
        cdda_sim::teleport::Teleport::Unchanged,
        "and a teleport to the square it is already on is no teleport"
    );
}

#[test]
fn a_short_range_teleport_spell_moves_its_caster() {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    let spell = cdda_content::spells::Spell {
        id: "blink".to_owned(),
        name: "Blink".to_owned(),
        effect_name: "short_range_teleport".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling {
            start: Some(4.0),
            increment: Some(0.0),
            end: Some(4.0),
        },
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        flags: std::collections::BTreeSet::new(),
    };
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell moved its caster"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(caster).map(|at| (at.x, at.y, at.z)),
        Some((4, 0, 0)),
        "**a range of four and no area is a jump of exactly four squares**"
    );
}
