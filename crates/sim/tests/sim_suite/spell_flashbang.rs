//! A spell that bangs and flashes: `flashbang`.
//!
//! Reference: `explosion_handler::flashbang` (`explosion.cpp:657-707`) through `spell_effect::flashbang` (`magic_spell_effect.cpp:1469-1473`). Two rules wear the one name:
//! the **bang** deafens whatever has ears within eight squares, and the **flash** blinds only what is looking at it - closer to the centre it also stuns.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A room of floor with the effects the flashbang names, so the gate tests the rule and not the registry.
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
    for y in -14..=14 {
        for x in -14..=14 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // The three effects the reference names, installed the plainest way an effect type can be: no caps, no decay, no flags. What the gate tests is the flashbang's own
    // arithmetic, so the types only have to exist.
    let plain = |id: &str| cdda_content::effects::EffectType {
        id: id.to_owned(),
        max_intensity: 1,
        max_duration: i64::MAX,
        dur_add_perc: 100,
        int_add_val: 0,
        int_decay_step: 1,
        int_decay_tick: 0,
        int_dur_factor: 0,
        int_decay_remove: false,
        main_parts_only: false,
        show_in_info: false,
        show_intensity: false,
        pain_sizing: false,
        hurt_sizing: false,
        harmful_cough: false,
        impairs_movement: false,
        pkill_addict_reduces: false,
        kill_chance: Vec::new(),
        red_kill_chance: Vec::new(),
        apply_ratings: Vec::new(),
        flags: std::collections::BTreeSet::new(),
        resist_traits: Vec::new(),
        resist_effects: Vec::new(),
        removes_effects: Vec::new(),
        blocks_effects: Vec::new(),
        mods: cdda_content::effects::ModTable::new(),
    };
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[plain("deaf"), plain("blind"), plain("stunned")],
    );
    world
}

/// A monster of a kind whose flags the gate states.
fn a_monster(world: &mut World, at: (i32, i32), flags: &[&str]) -> Entity {
    let entity = cdda_sim::spawn_monster(world, at, cdda_sim::MonsterKindId(0), 40);
    for flag in flags {
        cdda_sim::creature_flags::add_flag(world, entity, flag);
    }
    entity
}

#[test]
fn a_flashbang_deafens_what_hears_and_blinds_what_looks() {
    let mut world = a_world();
    let near = a_monster(&mut world, (2, 0), &["HEARS", "SEES"]);
    let deaf_one = a_monster(&mut world, (2, 1), &["SEES"]);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let outcome = cdda_sim::flashbang::flashbang(&mut world, (0, 0, 0), false);
    assert!(
        outcome.deafened.contains(&near),
        "**the monster with ears was deafened**, by the bang alone"
    );
    assert!(
        outcome.blinded.contains(&near),
        "and, looking at it, blinded too"
    );
    assert!(
        outcome.stunned.contains(&near),
        "and stunned, because it was within four squares"
    );
    let deaf = cdda_sim::applied_effects::worn(&world, near)
        .into_iter()
        .find(|effect| effect.id == "deaf")
        .expect("the deafness is on the creature");
    assert_eq!(
        deaf.duration, 32,
        "**eight square range less four a square: two squares away is thirty-two turns**"
    );
    assert!(
        !cdda_sim::applied_effects::has(&world, deaf_one, "deaf"),
        "**and a creature without the HEARS flag is not deafened**, even though it was blinded"
    );
    assert!(cdda_sim::applied_effects::has(&world, deaf_one, "blind"));
}

/// **A robot has no ears to hurt**, and distance decides who is even reached.
///
/// Reference: `:689-691` skips every monster of the `ROBOT` species, and `:698` only touches what is within eight squares.
#[test]
fn a_robot_is_not_deafened_and_nothing_out_of_range_is_touched() {
    let mut world = a_world();
    let robot = a_monster(&mut world, (1, 0), &["HEARS", "SEES", "ROBOT"]);
    let far = a_monster(&mut world, (9, 0), &["HEARS", "SEES"]);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let outcome = cdda_sim::flashbang::flashbang(&mut world, (0, 0, 0), false);
    assert!(
        !outcome.deafened.contains(&robot) && !outcome.blinded.contains(&robot),
        "**a robot is skipped whole**, because the reference continues past the whole robot species"
    );
    assert!(
        !outcome.deafened.contains(&far) && !outcome.blinded.contains(&far),
        "**and nine squares away is outside the eight-square bang**"
    );
    assert!(
        cdda_sim::sounds::heard_this_turn(&mut world)
            .iter()
            .any(|sound| sound.volume == 12),
        "the bang itself made a sound of twelve, which is the reference's own volume"
    );
}
