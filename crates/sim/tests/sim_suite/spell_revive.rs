//! Standing a corpse back up: `revive` and `revive_dormant`.
//!
//! Reference: `spell_effect::revive` and `spell_effect::revive_dormant` (`magic_spell_effect.cpp:1551-1596`) over `game::revive_corpse` (`game.cpp:5713-5760`). The two
//! spells differ in **exactly one clause** - whether the body is dormant - and in whether `NO_NECRO` refuses it, which is the whole reason two spells exist.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// Three corpse kinds: a plain zombie, a dormant one, and a `NO_NECRO` sleeper.
fn a_world() -> (World, cdda_sim::MonsterKindId, cdda_sim::MonsterKindId, cdda_sim::MonsterKindId) {
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
    for y in -5..=5 {
        for x in -5..=5 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_zombie", hp: 40 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_sleeper", hp: 30 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_wretch", hp: 20 },
        ],
    );
    let zombie = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("zombie");
    let sleeper = cdda_sim::monster_registry::id_of(&mut world, "mon_sleeper").expect("sleeper");
    let wretch = cdda_sim::monster_registry::id_of(&mut world, "mon_wretch").expect("wretch");
    cdda_sim::monster_registry::set_flags_and_species(
        &mut world,
        &[
            (zombie, set(&["REVIVES"]), set(&["ZOMBIE"])),
            (
                sleeper,
                set(&["REVIVES", "DORMANT"]),
                set(&["ZOMBIE"]),
            ),
            (
                wretch,
                set(&["REVIVES", "DORMANT", "NO_NECRO"]),
                set(&["ZOMBIE"]),
            ),
        ],
    );
    (world, zombie, sleeper, wretch)
}

/// A set of names, which is the shape the registry keeps a kind's flags and species in.
fn set(names: &[&str]) -> std::collections::BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A corpse of a kind on a square, as the death path leaves one.
fn a_corpse(world: &mut World, square: (i32, i32), kind: cdda_sim::MonsterKindId, revives: bool) -> Entity {
    // A corpse is an ordinary item of the `corpse` type with the dead kind attached, which is exactly what `item::make_corpse` leaves behind.
    let corpse = cdda_sim::drop_item(world, square, cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(corpse).insert(cdda_sim::CorpseOf {
        kind,
        died_on: 0,
        revives,
        monster_volume_ml: 62_500,
    });
    corpse
}

fn a_spell(effect: &str, species: &str, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: effect.to_owned(),
        effect_str: species.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_revive_spell_stands_up_a_zombie_and_takes_the_corpse_with_it() {
    let (mut world, zombie, _sleeper, _wretch) = a_world();
    let corpse = a_corpse(&mut world, (2, 0), zombie, true);
    let caster = cdda_sim::spawn_monster(&mut world, (0, 0), zombie, 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_spell("revive", "ZOMBIE", 2.0), 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the body stood up"
    );
    assert!(
        world.get_entity(corpse).is_err(),
        "**the corpse is gone**, because the thing it was is standing there instead"
    );
    let standing: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::CreatureKind)>()
        .iter(&world)
        .filter(|(entity, kind)| kind.0 == zombie && *entity != caster)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(standing.len(), 1, "**one zombie stands where the corpse was**");
    assert_eq!(
        cdda_sim::creature::health_of(&world, standing[0]),
        Some(40),
        "at its type's hit points, which is the port's stated answer to `init_from_item`"
    );
}

/// **The dormant clause is the whole difference between the two spells**, and `NO_NECRO` is the other half of it.
#[test]
fn a_dormant_body_is_only_woken_by_the_dormant_spell_and_no_necro_only_ignores_it_there() {
    let (mut world, _zombie, sleeper, wretch) = a_world();
    a_corpse(&mut world, (2, 0), sleeper, true);
    a_corpse(&mut world, (3, 0), wretch, true);
    let caster = cdda_sim::spawn_monster(&mut world, (0, 0), sleeper, 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // The plain revive will not touch a dormant body, nor a `NO_NECRO` one.
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_spell("revive", "ZOMBIE", 3.0), 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**a dormant body does not stand up for the plain revive**"
    );
    // The dormant spell wakes the sleeper, and revives the `NO_NECRO` wretch as well - it is the relaxed one.
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_spell("revive_dormant", "ZOMBIE", 3.0),
            0,
            (2, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::Done,
        "the dormant spell woke what it found"
    );
    let raised: Vec<cdda_sim::MonsterKindId> = world
        .query::<(&cdda_sim::creature::CreatureKind, &cdda_sim::creature::Creature)>()
        .iter(&world)
        .map(|(kind, _)| kind.0)
        .collect();
    assert!(
        raised.contains(&sleeper),
        "**the sleeper is up**, which the plain spell could not do"
    );
    assert!(
        raised.contains(&wretch),
        "**and so is the `NO_NECRO` body**, which the dormant spell ignores the flag for"
    );
}
