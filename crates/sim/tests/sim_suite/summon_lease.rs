//! How long a summoned creature lasts, and what `PERMANENT` actually means.
//!
//! Reference: `spell_effect::summon` (`magic_spell_effect.cpp`) calls `mon->set_summon_time( sp.duration_turns( caster ) )` unless the spell carries `PERMANENT`, and
//! `Creature::decrement_summon_timer` (`creature.cpp:2263-2272`) kills the creature when the deadline it stored has passed. The deadline is **absolute**, so it does not move
//! when the creature idles, and a summon with no deadline never expires.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor and a monster kind, with the clock at zero.
fn a_world() -> (World, Entity) {
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
    for y in -6..=6 {
        for x in -6..=6 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    cdda_sim::calendar::set_turn(&mut world, 0);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_summon(seconds: f64, permanent: bool) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    let mut flags = std::collections::BTreeSet::new();
    if permanent {
        flags.insert("PERMANENT".to_owned());
    }
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "summon".to_owned(),
        effect_str: "mon_zombie".to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(1.0),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(seconds),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags,
    }
}

#[test]
fn a_summoned_monster_lasts_exactly_as_long_as_the_spell_says() {
    let (mut world, caster) = a_world();
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_summon(30.0, false), 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the summon landed"
    );
    let summoned: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::summoned::Summoned)>()
        .iter(&world)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(summoned.len(), 1, "**one monster, with a lease**");
    assert_eq!(
        world.get::<cdda_sim::summoned::Summoned>(summoned[0]),
        Some(&cdda_sim::summoned::Summoned { ends_on: 30 }),
        "**and the deadline is the turn it was summoned plus thirty seconds**"
    );
    // Twenty-nine turns later it is still there; the thirtieth is the one that ends it.
    cdda_sim::calendar::set_turn(&mut world, 29);
    assert_eq!(cdda_sim::summoned::expire(&mut world), 0, "not yet");
    assert!(
        cdda_sim::creature::health_of(&world, summoned[0]).is_some_and(|hp| hp > 0),
        "and it still stands"
    );
    cdda_sim::calendar::set_turn(&mut world, 30);
    assert_eq!(
        cdda_sim::summoned::expire(&mut world),
        1,
        "**the deadline is inclusive: on the thirtieth turn the summon ends**"
    );
    assert!(
        cdda_sim::creature::health_of(&world, summoned[0]).is_some_and(|hp| hp <= 0),
        "which the store shows as a body with nothing left"
    );
}

/// **`PERMANENT` is the absence of a deadline**, not a very long one.
#[test]
fn a_permanent_summon_has_no_deadline_at_all() {
    let (mut world, caster) = a_world();
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_summon(30.0, true), 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the summon landed"
    );
    let summoned: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::CreatureKind)>()
        .iter(&world)
        .filter(|(entity, _)| *entity != caster)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(summoned.len(), 1, "one monster");
    assert_eq!(
        world.get::<cdda_sim::summoned::Summoned>(summoned[0]),
        None,
        "**a permanent summon carries no deadline at all**, which is how the reference says 'for good'"
    );
    cdda_sim::calendar::set_turn(&mut world, 10_000_000);
    assert_eq!(cdda_sim::summoned::expire(&mut world), 0, "so nothing expires");
    assert!(
        cdda_sim::creature::health_of(&world, summoned[0]).is_some_and(|hp| hp > 0),
        "however long the world runs"
    );
}
