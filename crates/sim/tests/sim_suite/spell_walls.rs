//! Two wall rules the spell flags decide: where a blast lands, and whether a jump may kill.
//!
//! Reference: `calculate_spell_effect_area` (`magic_spell_effect.cpp:447-465`) stops a **blast** short at the first impassable square unless the spell carries
//! `NO_PROJECTILE`, and `spell_effect::short_range_teleport` (`:159-182`) passes `safe = !sp.has_flag( UNSAFE_TELEPORT )`.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

/// A floor with a wall at `(2,0)`, so a spell aimed east of it is aimed through it.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    table.insert(2, false);
    table.insert_move_cost(2, 0);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), 1_u32), ("t_wall".to_owned(), 2_u32)],
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -6..=6 {
        for x in -6..=6 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    level.set_terrain(2, 0, 2, 0);
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_blast(flag: Option<&str>) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    let mut flags = std::collections::BTreeSet::new();
    if let Some(flag) = flag {
        flags.insert(flag.to_owned());
    }
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "none".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: scaling(9.0),
        area: scaling(1.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags,
    }
}

/// The squares a blast covers, asked of the effect's own rule rather than rebuilt here.
fn covered(world: &mut World, caster: Entity, spell: &cdda_content::spells::Spell) -> BTreeSet<(i32, i32)> {
    cdda_sim::spell_effects::squares_in_area(world, caster, spell, 0, (4, 0, 0))
        .into_iter()
        .map(|(x, y, _)| (x, y))
        .collect()
}

#[test]
fn a_blast_stops_short_at_a_wall_unless_the_spell_is_no_projectile() {
    let (mut world, caster) = a_world();
    let behind = covered(&mut world, caster, &a_blast(None));
    assert!(
        !behind.contains(&(4, 0)) && !behind.contains(&(5, 0)),
        "**a wall in the way keeps the blast on the near side**: {behind:?}"
    );
    assert!(
        behind.contains(&(1, 0)),
        "and it lands on the last square before the wall"
    );
    let through = covered(&mut world, caster, &a_blast(Some("NO_PROJECTILE")));
    assert!(
        through.contains(&(4, 0)) && through.contains(&(3, 0)),
        "**`NO_PROJECTILE` is not stopped at all**, so the blast lands where it was aimed: {through:?}"
    );
}

/// **An unsafe jump is death where a safe one is refused.**
///
/// Reference: `teleport::teleport_to_point` (`teleport.cpp:198-260`) through `spell_effect::short_range_teleport` (`:159-182`): a safe jump that keeps drawing solid ground
/// gives up after twenty tries and stays put, and the same draws without safety kill the caster on the first one.
#[test]
fn a_jump_that_can_only_land_in_a_wall_kills_when_the_spell_is_unsafe() {
    // A plateau of one standable square: every candidate two squares away is off it, so all twenty tries fail.
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
    level.set_terrain(0, 0, 1, 0);
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);

    assert_eq!(
        cdda_sim::teleport::creature(&mut world, caster, 2, 2, true),
        cdda_sim::teleport::Teleport::Refused,
        "**a safe jump with nowhere to land refuses to move**"
    );
    assert_eq!(
        cdda_sim::teleport::creature(&mut world, caster, 2, 2, false),
        cdda_sim::teleport::Teleport::Died,
        "**and the same draws without safety are death**, on the first try"
    );
    let mut spell = a_blast(None);
    spell.effect_name = "short_range_teleport".to_owned();
    spell.range = cdda_content::spells::Scaling {
        start: Some(2.0),
        increment: Some(0.0),
        end: Some(2.0),
    };
    spell.area = cdda_content::spells::Scaling {
        start: Some(0.0),
        increment: Some(0.0),
        end: Some(0.0),
    };
    let mut unsafe_spell = spell.clone();
    unsafe_spell.flags = ["UNSAFE_TELEPORT".to_owned()].into_iter().collect();
    // A second caster, so the death below is this spell's doing and not the probe's.
    let second = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, second, &unsafe_spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "the jump happened and the caster did not survive to call it a move"
    );
    assert!(
        cdda_sim::creature::health_of(&world, second).is_some_and(|hp| hp <= 0),
        "**an unsafe landing leaves nothing standing**, which is the reference's 9,999 damage"
    );
    // The same spell without the flag leaves the caster alive and where it stood.
    let third = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, third, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "and a safe spell of the same shape does nothing at all"
    );
    assert!(
        cdda_sim::creature::health_of(&world, third).is_some_and(|hp| hp > 0),
        "**because refusing is not dying**"
    );
}
