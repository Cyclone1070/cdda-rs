//! A spell that sets a trap, and the store the trap lives in.
//!
//! Reference: `spell_effect::add_trap` (`magic_spell_effect.cpp:1598-1605`) over `map::trap_set` and `map::tr_at`. A trap is a thing standing on a square, so in a one-store port
//! it is a component on an entity at that square - the same shape as furniture - and "is there a trap here" is a question about the store.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor with nothing on it.
fn a_world() -> World {
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
    world
}

fn a_spell(trap: &str, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "add_trap".to_owned(),
        effect_str: trap.to_owned(),
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
    }
}

#[test]
fn a_trap_spell_sets_its_trap_on_the_square_it_landed_on() {
    let mut world = a_world();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // `pseudo_dormant_trap_setup` is the data's own spell: `tr_dormant_corpse`.
    let spell = a_spell("tr_dormant_corpse", 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (2, 1, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the trap was set"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (2, 1, 0)),
        Some("tr_dormant_corpse".to_owned()),
        "**on the square the spell landed on** and nowhere else"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "the caster's own square is untouched"
    );
}

/// **A trap already set is not overwritten**, which is the reference's guard and `map::trap_set`'s own.
#[test]
fn a_square_that_already_has_a_trap_keeps_it() {
    let mut world = a_world();
    assert!(
        cdda_sim::square::set_trap(&mut world, (1, 1, 0), "tr_bubblewrap"),
        "the first trap goes down"
    );
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let spell = a_spell("tr_dormant_corpse", 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (1, 1, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**the spell does nothing where a trap already stands**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 1, 0)),
        Some("tr_bubblewrap".to_owned()),
        "and the trap that was there is still there"
    );
}
