//! What a **summoned** body leaves behind when it dies.
//!
//! Reference: `add_summoned_mon` sets `spawned_mon.no_extra_death_drops = !sp.has_flag( SPAWN_WITH_DEATH_DROPS )` (`magic_spell_effect.cpp:1343`), and `monster::die` tests that
//! flag before `drop_items_on_death` (`monster.cpp:3126-3129`). A summoned body therefore leaves its corpse and nothing else, unless the spell says `SPAWN_WITH_DEATH_DROPS`.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A world where `mon_looter` names a `death_drops` group that always produces one rag.
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
            id: "mon_looter",
            hp: 40,
        }],
    );
    cdda_sim::monster_registry::set_death_drops(&mut world, cdda_sim::MonsterKindId(1), "loot_group");
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 250,
            weight_g: 100,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let mut definitions = std::collections::BTreeMap::new();
    definitions.insert(
        "loot_group".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "loot_group".to_owned(),
            distribution: false,
            entries: vec![cdda_content::item_group_draw::Entry::Item {
                id: "rag".to_owned(),
                probability: 100,
                count: (1, 1),
                event: None,
            }],
        },
    );
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    cdda_sim::calendar::set_turn(&mut world, 0);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_summon(flags: &[&str]) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "summon".to_owned(),
        effect_str: "mon_looter".to_owned(),
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
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

/// Kill the one summoned body and answer what is lying where it fell.
fn loot_after_death(world: &mut World, at: (i32, i32)) -> Vec<cdda_sim::ItemTypeId> {
    let rag = cdda_sim::item_registry::id_of(world, "rag").expect("the item is registered");
    // **A body has to die before it can drop anything**, and this fixture has no blow to kill it: the sweep only reaps creatures whose health has reached zero.
    let doomed: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::Creature)>()
        .iter(world)
        .map(|(entity, _)| entity)
        .collect();
    for creature in doomed {
        if cdda_sim::creature::health_of(world, creature).is_some_and(|hp| hp > 0) {
            cdda_sim::creature::set_health_left(world, creature, 0);
        }
    }
    cdda_sim::creature::reap_dead_creatures_as_corpses(world, cdda_sim::ItemTypeId(99), |_| 62_500);
    cdda_sim::items_at(world, at)
        .iter()
        .filter_map(|item| world.get::<cdda_sim::GroundItem>(*item).map(|held| held.kind))
        .filter(|kind| *kind == rag)
        .collect()
}

#[test]
fn a_summoned_body_leaves_no_extra_loot_unless_the_spell_says_so() {
    let (mut world, caster) = a_world();
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_summon(&[]), 0, (3, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the summon landed"
    );
    // The marker is on **the body standing on the summon's square**, which is the entity the reaper will look at - and that is the fact the drop rule reads.
    let on_square: Vec<Entity> = cdda_sim::creature::creature_at(&world, (3, 0, 0));
    assert_eq!(on_square.len(), 1, "one body was summoned");
    assert!(
        world.get::<cdda_sim::creature::NoExtraDeathDrops>(on_square[0]).is_some(),
        "**the summoned body carries the marker**"
    );
    let rags = loot_after_death(&mut world, (3, 0));

    assert!(
        rags.is_empty(),
        "**a summoned body drops none of its `death_drops` loot** - the type's group would have produced a rag: {rags:?}"
    );

    let (mut world, caster) = a_world();
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_summon(&["SPAWN_WITH_DEATH_DROPS"]),
            0,
            (3, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::Done,
        "the flagged summon landed"
    );
    let rags = loot_after_death(&mut world, (3, 0));
    assert_eq!(
        rags.len(),
        1,
        "**`SPAWN_WITH_DEATH_DROPS` keeps the loot**, which is what the nine flagged spells in the data ask for"
    );
}

/// **An ordinary monster, with no spell involved, still drops what its type says.**
#[test]
fn a_plain_monster_still_drops_its_loot() {
    let (mut world, _caster) = a_world();
    cdda_sim::spawn_monster(&mut world, (4, 0), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let rags = loot_after_death(&mut world, (4, 0));
    assert_eq!(
        rags.len(),
        1,
        "a body that no rule marked leaves the rag its group names"
    );
}
