//! That armour is asked **by damage type**, which is the whole reason a monster states three figures.
//!
//! Reference: `Creature::get_armor_type( dt, bp )` - `monster::get_armor_type` answers from `mtype::armor_bash`, `armor_cut` and `armor_stab` (`mtype.h:216-220`), so a zombie that
//! shrugs off a club can still be cut by a blade. The port asked one figure for every type until this round, which made a cut spell and a bash spell identical.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor, one monster kind, and the armour the data states for it.
fn a_world(bash: i32, cut: i32) -> (World, Entity) {
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
    cdda_sim::monster_registry::set_armours(
        &mut world,
        &[(
            cdda_sim::MonsterKindId(1),
            cdda_sim::creature::Armour { bash, cut, stab: 0 },
        )],
    );
    cdda_sim::monster_registry::set_bodies(
        &mut world,
        &[(
            cdda_sim::MonsterKindId(1),
            cdda_sim::creature::Body {
                parts: vec![cdda_sim::hit_location::BodyPart {
                    name: "torso".to_owned(),
                    hit_size: 36.0,
                    hit_difficulty: 1.0,
                    upper: false,
                    lower: false,
                    ..Default::default()
                }],
            },
        )],
    );
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn an_attack(damage_type: &str) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "attack".to_owned(),
        effect_str: String::new(),
        damage_type: damage_type.to_owned(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(9.0),
        range: scaling(9.0),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

/// **The same monster stops four of a club and none of a blade.**
#[test]
fn a_monsters_armour_answers_for_the_type_the_blow_deals() {
    let (mut world, _caster) = a_world(4, 0);
    let zombie = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(&mut world, (0, 0, 0), &an_attack("bash"), 0, zombie, 0.5),
        cdda_sim::spellcasting::Cast::Hit {
            part: "torso".to_owned(),
            dealt: 5
        },
        "**nine of blunt against four of armour is five**, which is the data's `armor_bash`"
    );
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(&mut world, (0, 0, 0), &an_attack("cut"), 0, zombie, 0.5),
        cdda_sim::spellcasting::Cast::Hit {
            part: "torso".to_owned(),
            dealt: 9
        },
        "**and the same body lets all nine of a cut through**, because it states no `armor_cut`"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, zombie),
        Some(40 - 5 - 9),
        "the store records both blows"
    );
}

/// **A type the creature states no figure for is nothing**, rather than borrowing the bash figure.
#[test]
fn a_damage_type_with_no_figure_resists_nothing() {
    let (mut world, _caster) = a_world(4, 0);
    let zombie = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    assert_eq!(
        cdda_sim::creature::armour_on(&world, zombie, "torso", Some("bash")),
        4,
        "bash is the data's four"
    );
    assert_eq!(
        cdda_sim::creature::armour_on(&world, zombie, "torso", Some("stab")),
        0,
        "stab is the data's zero"
    );
    assert_eq!(
        cdda_sim::creature::armour_on(&world, zombie, "torso", Some("heat")),
        0,
        "**and a type with no figure at all is nothing**, which is the reference's default branch"
    );
}
