//! A spell that hits the world with the strength of its damage, and the square that keeps count.
//!
//! Reference: `spell_effect::bash` (`magic_spell_effect.cpp:1770-1781`) over `map::bash`. The spell is a thin thing; what makes it worth a gate is the **state between hits** -
//! a square keeps the damage a failed hit left behind, and the next hit is stronger for it - which is the rule [`cdda_sim::square::BashDamage`] was added to carry.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::mapdata::load_paths;
use std::path::{Path, PathBuf};

/// A chain fence standing on a floor, with rock floor below it - the square `corpus/bash/hits.json` bashes.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let reference =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let args: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let loaded = load_paths(&args).expect("the map data loads");
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_floor".to_owned(), 1_u32),
            ("t_wall".to_owned(), 2_u32),
            ("t_open_air".to_owned(), 3_u32),
            ("t_chainfence".to_owned(), 4_u32),
            ("t_chainfence_posts".to_owned(), 5_u32),
            ("t_rock_floor".to_owned(), 6_u32),
        ],
    );
    cdda_sim::furniture_types::set_types(&mut world, [("f_null".to_owned(), 1_u32)]);
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    table.insert(4, true);
    table.insert_move_cost(4, 0);
    table.insert(5, true);
    table.insert_move_cost(5, 0);
    table.insert(6, true);
    table.insert_move_cost(6, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -4..=4 {
        for x in -4..=4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    level.set_terrain(2, 0, 4, 0);
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::bash::set_data(&mut world, loaded);
    // A generator that answers the bottom of every range, so a hit that can break the wall does.
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    // **Adjacent to the fence, because a blast stops at the first wall it meets**: aiming from four squares away puts the epicentre on the near side of the fence and bashes
    // the floor instead, which is exactly what the reference's `calculate_spell_effect_area` does. Aimed at from the next square, the fence itself is the first square of
    // the trajectory and the blast lands on it.
    let caster = cdda_sim::spawn_player(&mut world, (1, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
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
        effect_name: "bash".to_owned(),
        effect_str: String::new(),
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
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_bash_spell_hits_every_square_of_its_blast_with_its_damage() {
    let (mut world, caster) = a_world();
    // The dump's own case: nine hundred and ninety-nine strength turns a chain fence into its posts, whatever the roll (`corpus/bash/hits.json`).
    let spell = a_spell(999.0, 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (2, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the blow hit something"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 0, 0))
            .and_then(|code| cdda_sim::terrain_types::name_of(&world, code)),
        Some("t_chainfence_posts".to_owned()),
        "**the fence is its posts now**, which is where the reference puts it"
    );
    assert_eq!(
        cdda_sim::square::bash_damage_at(&world, (2, 0, 0)),
        0,
        "and a square that broke keeps no damage"
    );
}

/// **A square keeps what a failed hit did to it**, and the next hit is stronger for it.
///
/// The numbers are the dump's: at eleven strength a chain fence takes one point of damage a hit and stays standing, four hits in a row
/// (`corpus/bash/hits.json`, `t_chainfence` at strength 11, steps 0 to 4).
#[test]
fn a_square_keeps_the_damage_a_failed_hit_left_and_the_next_hit_uses_it() {
    let (mut world, _caster) = a_world();
    for expected in 1..=4 {
        let outcome = cdda_sim::bash::bash_square(&mut world, (2, 0, 0), 11, true)
            .expect("the data is installed");
        assert!(outcome.did_bash, "**the hit landed on something solid**");
        assert_eq!(
            cdda_sim::square::bash_damage_at(&world, (2, 0, 0)),
            expected,
            "**the square keeps one more point of damage each hit**, so the state lives on the square and not in the caller"
        );
        assert_eq!(
            cdda_sim::square::terrain_at(&world, (2, 0, 0))
                .and_then(|code| cdda_sim::terrain_types::name_of(&world, code)),
            Some("t_chainfence".to_owned()),
            "and eleven strength never breaks a fence"
        );
    }
}
