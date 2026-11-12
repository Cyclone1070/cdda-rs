//! A spell that makes you throw up, and the retching rule itself.
//!
//! Reference: `spell_effect::vomit` (`magic_spell_effect.cpp:1440-1455`) over `Character::vomit` (`character.cpp:7520-7560`). The gate pins the two things that are easy to get
//! wrong: the stomach really is emptied and leaves **bile** behind, and the nausea is the effect's **floor** duration - because the reference measures the stomach *after*
//! emptying it, so the larger term it looks like it computes is always zero.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A floor, the effects the rule names, and a character with a full stomach.
fn a_world(contents_ml: i64) -> (World, Entity) {
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
    for y in -4..=4 {
        for x in -4..=4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // The effect types the rule reads, with a nausea floor of seven turns and a hundred-turn ceiling - so a gate can tell the floor from the product.
    let plain = |id: &str, max_duration: i64, int_dur_factor: i64| cdda_content::effects::EffectType {
        id: id.to_owned(),
        max_intensity: 1,
        max_duration,
        dur_add_perc: 100,
        int_add_val: 0,
        int_decay_step: 1,
        int_decay_tick: 0,
        int_dur_factor,
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
        &[
            plain("nausea", 900, 7),
            plain("foodpoison", 100_000, 0),
            plain("drunk", 100_000, 0),
        ],
    );
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_bile", 0, 3)]);
    let who = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    if let Some(mut body) = world.get_mut::<cdda_sim::body::Body>(who) {
        body.stomach.contents_ml = contents_ml;
    }
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, who)
}

fn a_spell(area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "vomit".to_owned(),
        effect_str: String::new(),
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
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_vomit_spell_empties_the_stomach_and_leaves_bile_behind() {
    let (mut world, who) = a_world(500);
    let caster = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let spell = a_spell(1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the character retched"
    );
    let body = cdda_sim::body::body_of(&world, who).expect("a body");
    assert_eq!(
        body.stomach.contains_ml(),
        0,
        "**the stomach is empty afterwards**, whatever it held"
    );
    let mut bile = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if cdda_sim::field_registry::id_of(&mut world, "fd_bile").is_some_and(|bile_id| {
                cdda_sim::field_strength_at(&world, (dx, dy, 0), bile_id) == 1
            }) {
                bile += 1;
            }
        }
    }
    assert_eq!(
        bile, 1,
        "**and one square beside the character holds bile of strength one**, which is where the reference puts it"
    );
    // The nausea is the floor, not the product: nine hundred turns times five hundred millilitres over twenty-one would be enormous, and the reference reads the stomach
    // after emptying it.
    let nausea = cdda_sim::applied_effects::worn(&world, who)
        .into_iter()
        .find(|effect| effect.id == "nausea")
        .expect("nausea");
    assert_eq!(
        nausea.duration, 7,
        "**seven turns, the effect's floor** - not `max_duration * ml / 21`, because the stomach is measured empty"
    );
}

/// **A dry heave leaves nothing behind and does not wake you**, and a character already nauseous is not made more so.
#[test]
fn an_empty_stomach_leaves_no_bile_and_a_nauseous_character_is_not_re_nauseated() {
    let (mut world, _who) = a_world(0);
    let caster = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let spell = a_spell(1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "a dry heave is still a retch"
    );
    let mut bile = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(bile_id) = cdda_sim::field_registry::id_of(&mut world, "fd_bile")
                && cdda_sim::field_strength_at(&world, (dx, dy, 0), bile_id) > 0
            {
                bile += 1;
            }
        }
    }
    assert_eq!(
        bile, 0,
        "**nothing in the stomach leaves nothing on the ground**"
    );

    // A character who is already nauseous keeps the nausea it had; the reference only adds one when there is none.
    let (mut world, who) = a_world(0);
    let caster = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let nausea = cdda_sim::effect_registry::kind_of(&mut world, "nausea").expect("nausea");
    cdda_sim::applied_effects::add(
        &mut world,
        who,
        cdda_sim::effects::ActiveEffect::new(&nausea, 400, 1),
    );
    cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0));
    let after = cdda_sim::applied_effects::worn(&world, who)
        .into_iter()
        .find(|effect| effect.id == "nausea")
        .expect("nausea");
    assert_eq!(
        after.duration, 400,
        "**the nausea it already had is the nausea it keeps**"
    );
}
