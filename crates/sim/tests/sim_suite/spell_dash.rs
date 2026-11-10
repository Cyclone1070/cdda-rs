//! A spell that moves its caster: `dash`.
//!
//! Reference: `spell_effect::dash` (`magic_spell_effect.cpp:1783-1833`). The leap is the point, and three of its properties are gates below: the caster stops **short** of a
//! creature, only an avatar walks at all, and the moves the leap spent are handed back - a dash costs the casting, not a walk.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor of move cost two, so every square is walkable, with the level loaded.
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
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    world
}

fn a_spell(range: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "dash".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "cone".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(0.0),
        range: scaling(range),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_dash_carries_its_caster_along_the_line_and_gives_the_moves_back() {
    let mut world = a_world();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    // A step costs the ground's own price twice over - leaving one square and entering the next - so six squares want more than a turn's hundred moves. **The leap is
    // still limited by what the caster can pay**, because the reference walks with `walk_move` before it gives the moves back.
    cdda_sim::creature::set_moves(&mut world, caster, 700);
    // `spell_short_leap` is the data's own: range six to a square six east.
    let spell = a_spell(6.0, 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (6, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the caster moved"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(caster).map(|at| (at.x, at.y, at.z)),
        Some((6, 0, 0)),
        "**six squares east, one per square of the line**"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Moves>(caster).map(|moves| moves.left),
        Some(700),
        "**and every move the leap spent is handed back**, which is the reference's `set_moves( cur_moves )`"
    );
}

/// A monster does not walk, and a creature in the way stops an avatar short.
///
/// Reference: `magic_spell_effect.cpp:1803-1818`: the walking is inside `if( caster_you != nullptr )`, and the first square holding a creature (or refusing the move) breaks
/// the walk, leaving the caster on the square before it.
#[test]
fn only_an_avatar_walks_and_a_creature_in_the_way_stops_it_short() {
    let mut world = a_world();
    // A monster caster: its dash is a cone from where it stands and it never moves.
    let monster = cdda_sim::spawn_monster(&mut world, (0, 0), cdda_sim::MonsterKindId(0), 30);
    let spell = a_spell(6.0, 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, monster, &spell, 0, (6, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**a monster's dash does nothing at all** - the reference's walk is the avatar's alone"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(monster).map(|at| (at.x, at.y, at.z)),
        Some((0, 0, 0)),
        "and it is still standing where it was"
    );

    let avatar = cdda_sim::spawn_player(&mut world, (0, 5), 80, 0);
    cdda_sim::creature::set_moves(&mut world, avatar, 500);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // Something standing three squares east of the avatar.
    cdda_sim::spawn_monster(&mut world, (3, 5), cdda_sim::MonsterKindId(0), 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, avatar, &spell, 0, (6, 5, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the avatar moved"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(avatar).map(|at| (at.x, at.y, at.z)),
        Some((2, 5, 0)),
        "**two squares east, stopping before the creature** - it does not share a square and does not pass through"
    );
}
