//! A spell that changes the ground: `ter_transform`.
//!
//! Reference: `spell_effect::transform_blast` (`magic_spell_effect.cpp:1423-1434`). The spell's `effect_str` names a `ter_furn_transform`, each square of its blast draws
//! `one_in( damage )`, and a square whose terrain the table names is replaced.
//!
//! The odds are the part that has to be pinned rather than assumed: `one_in` is **always true for a chance of one or less** (`rng.cpp:196-199`), and all three of the content's
//! terrain spells state a damage of **zero**. So in this content the spell changes every square it covers, and a "fix" that treated zero as "never" would change the game.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor of the two terrains the transform names, with the table installed.
fn a_world() -> World {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_dirt".to_owned(), 1_u32), ("t_rock_floor".to_owned(), 2_u32)],
    );
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    table.insert(2, true);
    table.insert_move_cost(2, 2);
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
    let mut results = std::collections::BTreeMap::new();
    results.insert("t_dirt".to_owned(), "t_rock_floor".to_owned());
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [("cavern_emptiness".to_owned(), results)],
    );
    // A generator that answers the bottom of every range, so a draw of `one_in( n )` for n above one always succeeds - the gate's other cases lean on a chance below two.
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    world
}

fn a_spell(damage: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "ter_transform".to_owned(),
        effect_str: "cavern_emptiness".to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(damage),
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
fn a_zero_chance_transforms_every_square_because_one_in_says_so() {
    let mut world = a_world();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // `void_spider_empty` is the data's own: a damage of zero and an area of five.
    let spell = a_spell(0.0, 5.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the ground changed"
    );
    let mut changed = 0;
    for dy in -5..=5 {
        for dx in -5..=5 {
            if cdda_sim::square::terrain_at(&world, (dx, dy, 0)) == Some(2) {
                changed += 1;
            }
        }
    }
    assert_eq!(
        changed,
        11 * 11,
        "**every square of an area-five blast changed**, because `one_in( 0 )` is true by the reference's own shortcut"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (6, 0, 0)),
        Some(1),
        "and the square just outside the blast did not"
    );
}

/// **A chance of one is the same as a chance of zero**, and a square the table does not name is left alone.
#[test]
fn a_chance_of_one_also_always_fires_and_an_unnamed_square_is_untouched() {
    let mut world = a_world();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let certain = a_spell(1.0, 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &certain, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "a chance of one fires"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(2),
        "**and it changes every square too**, because `one_in` short-circuits anything at or below one"
    );
}
