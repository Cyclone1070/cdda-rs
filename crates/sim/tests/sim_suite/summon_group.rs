//! A summon that names a **group** rather than a monster.
//!
//! Reference: `add_summoned_mon` (`magic_spell_effect.cpp:1322-1332`) reads `effect_str` as a monster's id, or as a group's id when the spell carries `SPAWN_GROUP`, and draws
//! the monster from that group - **one draw per summon**, so a group with two entries can put two different monsters on the ground.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::monster_groups::{EntryKind, MonsterGroup, MonsterGroupEntry};

fn a_world(seed: u32) -> (World, Entity) {
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
        &[
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_zombie",
                hp: 40,
            },
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_dog",
                hp: 25,
            },
        ],
    );
    cdda_sim::monster_groups::set_groups(
        &mut world,
        vec![
            MonsterGroup {
                id: "GROUP_ONLY_ZOMBIES".to_owned(),
                entries: vec![MonsterGroupEntry {
                    kind: EntryKind::Monster("mon_zombie".to_owned()),
                    weight: 1,
                    cost_multiplier: 1,
                    pack: (1, 1),
                }],
            },
            // A group that names another group, which is how the data lets a broad family borrow a finer weight.
            MonsterGroup {
                id: "GROUP_NESTED".to_owned(),
                entries: vec![MonsterGroupEntry {
                    kind: EntryKind::Group("GROUP_ONLY_ZOMBIES".to_owned()),
                    weight: 1,
                    cost_multiplier: 1,
                    pack: (1, 1),
                }],
            },
        ],
    );
    cdda_sim::calendar::set_turn(&mut world, 0);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(seed));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_summon(name: &str, flags: &[&str]) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "summon".to_owned(),
        effect_str: name.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(1.0),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(30.0),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

fn summoned_kinds(world: &mut World, caster: Entity) -> Vec<String> {
    let kinds: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::CreatureKind)>()
        .iter(world)
        .filter(|(entity, _)| *entity != caster)
        .map(|(entity, _)| entity)
        .collect();
    kinds
        .into_iter()
        .filter_map(|entity| {
            let kind = world.get::<cdda_sim::creature::CreatureKind>(entity)?.0;
            cdda_sim::monster_registry::name_of(world, kind)
        })
        .collect()
}

/// **A group's id and a monster's id are different things**, and only the flag says which the spell means.
#[test]
fn a_group_summon_draws_a_monster_from_the_group() {
    let (mut world, caster) = a_world(3);
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_summon("GROUP_ONLY_ZOMBIES", &["SPAWN_GROUP"]),
            0,
            (2, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::Done,
        "the group summon landed"
    );
    assert_eq!(
        summoned_kinds(&mut world, caster),
        vec!["mon_zombie".to_owned()],
        "**and it brought the group's only monster**"
    );

    // Without the flag the same string is a monster's id, which no kind answers - so nothing is summoned rather than a guess.
    let (mut world, caster) = a_world(3);
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_summon("GROUP_ONLY_ZOMBIES", &[]),
            0,
            (2, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**a group's name without `SPAWN_GROUP` summons nothing**, because it names no monster"
    );
    assert!(
        summoned_kinds(&mut world, caster).is_empty(),
        "and the ground is as empty as it started"
    );
}

/// **A group that names a group is followed**, which is how the data nests them.
#[test]
fn a_nested_group_is_followed_to_its_monster() {
    let (mut world, caster) = a_world(5);
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_summon("GROUP_NESTED", &["SPAWN_GROUP"]),
            0,
            (2, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::Done,
        "the nested summon landed"
    );
    assert_eq!(
        summoned_kinds(&mut world, caster),
        vec!["mon_zombie".to_owned()],
        "**the draw walked from the group into the group it names**"
    );
}
