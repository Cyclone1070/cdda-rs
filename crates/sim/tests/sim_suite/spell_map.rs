//! Two effect kinds that change nothing on the ground: `map` and `none`.
//!
//! Reference: `spell_effect::map` (`magic_spell_effect.cpp:1492-1501`) and `spell_effect::none` (`:1418-1421`). The first reveals the overmap **in overmap tiles around the
//! avatar**, which is not the same as squares; the second is the reference's own way of saying a spell's effect is a mistake, and it does nothing.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn a_spell(effect: &str, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: effect.to_owned(),
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
fn a_map_spell_reveals_overmap_tiles_around_the_avatar() {
    let mut world = World::new();
    // The avatar stands on a square that is the overmap tile (2, 3): twenty-four squares to a tile, which is the reference's own scale.
    let avatar = cdda_sim::spawn_player(&mut world, (2 * 24, 3 * 24), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // `AEA_MAP` is the data's own: an area of twenty.
    let spell = a_spell("map", 20.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, avatar, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the map was revealed"
    );
    let seen = cdda_sim::overmap::seen_tiles(&world);
    assert_eq!(
        seen.len(),
        41 * 41,
        "**a radius of twenty is a window forty-one tiles across**, which is the square the reference's `reveal` walks"
    );
    assert!(
        seen.contains(&(2, 3, 0)),
        "and the avatar's own tile is in it"
    );
}

/// **Only an avatar has a map**, and an effect the data calls `none` does nothing at all.
#[test]
fn a_monster_reveals_nothing_and_none_does_nothing() {
    let mut world = World::new();
    let monster = cdda_sim::spawn_monster(&mut world, (0, 0), cdda_sim::MonsterKindId(0), 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let spell = a_spell("map", 5.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, monster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "a monster's map spell reveals nothing, because a monster has no map"
    );
    assert!(
        cdda_sim::overmap::seen_tiles(&world).is_empty(),
        "**and not a single tile was revealed**"
    );

    let nothing = a_spell("none", 3.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, monster, &nothing, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "`none` is the reference's way of reporting an invalid effect, and it changes nothing"
    );
    assert!(
        cdda_sim::overmap::seen_tiles(&world).is_empty(),
        "still nothing"
    );
}
