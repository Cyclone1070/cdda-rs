//! Where a summon lands and whose side it is on.
//!
//! Reference: `spawn_summoned_monster` and `add_summoned_mon` (`magic_spell_effect.cpp:1316-1362`) over `is_summon_friendly` (`:1306-1314`). Two facts decide a summon: the
//! square is **drawn from the spell's own area** and erased whether or not it was used, and the monster arrives on the caster's side unless the spell says otherwise.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor of nine squares around the aim point, one monster kind, and a generator.
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
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    cdda_sim::calendar::set_turn(&mut world, 0);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(seed));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_summon(flags: &[&str], how_many: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "summon".to_owned(),
        effect_str: "mon_zombie".to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(how_many),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(30.0),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned(), "ground".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

fn summoned(world: &mut World, caster: Entity) -> Vec<Entity> {
    world
        .query::<Entity>()
        .iter(world)
        .filter(|entity| *entity != caster)
        .filter(|entity| world.get::<cdda_sim::creature::CreatureKind>(*entity).is_some())
        .collect()
}

#[test]
fn a_summon_is_on_the_casters_side_unless_the_spell_is_hostile() {
    let (mut world, caster) = a_world(7);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &a_summon(&[], 1.0, 0.0), 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the summon landed"
    );
    let allies = summoned(&mut world, caster);
    assert_eq!(allies.len(), 1, "one monster");
    assert!(
        cdda_sim::friendly::is_friendly(&world, allies[0]),
        "**a summon arrives friendly**, which is the reference's `friendly = INT_MAX`"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, allies[0]),
        i32::MAX,
        "and its friendliness never runs out"
    );

    let (mut world, caster) = a_world(7);
    assert_eq!(
        cdda_sim::spell_effects::apply(
            &mut world,
            caster,
            &a_summon(&["HOSTILE_SUMMON"], 1.0, 0.0),
            0,
            (2, 0, 0)
        ),
        cdda_sim::spell_effects::Effective::Done,
        "the hostile summon landed too"
    );
    let hostiles = summoned(&mut world, caster);
    assert_eq!(hostiles.len(), 1, "one monster");
    assert!(
        !cdda_sim::friendly::is_friendly(&world, hostiles[0]),
        "**`HOSTILE_SUMMON` arrives on nobody's side** - thirty spells in this content are"
    );
}

/// **The square is drawn from the spell's own area**, so a summon never lands outside the blast it was aimed with.
#[test]
fn every_summon_lands_inside_the_spells_own_area() {
    let (mut world, caster) = a_world(11);
    let spell = a_summon(&[], 9.0, 1.0);
    // **The area is the one the spell had when it was cast**, which is the reference's own order: `calculate_spell_effect_area` filters first and everything after works on the
    // result. Asked afterwards, the squares the summons now stand on answer differently - they hold allies, and an ally is not what a hostile-and-ground spell was aimed at.
    let area: Vec<(i32, i32)> = cdda_sim::spell_effects::squares_in_area(&mut world, caster, &spell, 0, (2, 0, 0))
        .into_iter()
        .map(|(x, y, _)| (x, y))
        .collect();
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "nine summons landed"
    );
    let made = summoned(&mut world, caster);
    assert_eq!(
        made.len(),
        9,
        "**an area of one holds all nine**, which is what drawing without replacement from the blast gives"
    );
    for monster in &made {
        let at = world.get::<cdda_sim::Position>(*monster).map(|held| (held.x, held.y));
        assert!(
            at.is_some_and(|at| area.contains(&at)),
            "**every summon stands inside the area**: {:?} in {area:?}",
            at
        );
    }
}
