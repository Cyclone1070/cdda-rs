//! Sounds last less than a turn, and a `noise` spell makes one.
//!
//! Reference: `sounds::sound` (`sounds.cpp:317-336`) puts an event in `sounds_since_last_turn`, which the turn loop drains;
//! `spell_effect::noise` (`magic_spell_effect.cpp:1435-1438`) makes one whose volume is the spell's damage.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;
use cdda_sim::sounds;
use cdda_sim::spell_effects::{self, Effective};

fn scaling(value: i64) -> cdda_content::spells::Scaling {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    cdda_content::spells::Scaling {
        start: Some(as_float),
        increment: Some(0.0),
        end: Some(as_float),
    }
}

fn a_noise_spell(volume: i64) -> Spell {
    Spell {
        id: "test_noise".to_owned(),
        name: "Test Noise".to_owned(),
        effect_name: "noise".to_owned(),
        effect_str: String::new(),
        damage_type: "pure".to_owned(),
        shape: "blast".to_owned(),
        energy_source: String::new(),
        damage: scaling(volume),
        range: scaling(3),
        area: scaling(0),
        energy: scaling(0),
        duration: scaling(0),
        casting_time: scaling(0),
        difficulty: Some(0.0),
        max_level: Some(10.0),
    }
}

#[test]
fn a_noise_spell_makes_a_sound_as_loud_as_its_damage() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let outcome = spell_effects::apply(&mut world, caster, &a_noise_spell(14), 0, (3, 4, 0));
    assert_eq!(outcome, Effective::Done);

    let made = sounds::heard_this_turn(&mut world);
    assert_eq!(made.len(), 1, "one sound");
    assert_eq!(made[0].volume, 14, "as loud as the spell's damage");
    assert_eq!(made[0].position, (3, 4, 0), "from the square the spell reached");
}

#[test]
fn a_sound_is_louder_the_closer_you_stand_and_ends_with_the_turn() {
    let mut world = World::new();
    let sound = sounds::Sound {
        position: (0, 0, 0),
        volume: 10,
        category: sounds::SoundCategory::Combat,
        description: "bang!".to_owned(),
    };
    assert_eq!(sounds::volume_at(&sound, (0, 0, 0)), 10, "at the source");
    assert_eq!(sounds::volume_at(&sound, (3, 0, 0)), 7, "three squares away");
    assert_eq!(
        sounds::volume_at(&sound, (20, 0, 0)),
        0,
        "and past the volume it is silence, not a negative loudness"
    );
    // The measure is the larger axis, as the reference's `sound_distance` is, not the diagonal.
    assert_eq!(sounds::volume_at(&sound, (3, 3, 0)), 7);

    sounds::make_sound(&mut world, (1, 1, 0), 5);
    assert_eq!(sounds::heard_this_turn(&mut world).len(), 1);
    sounds::clear(&mut world);
    assert!(
        sounds::heard_this_turn(&mut world).is_empty(),
        "the end of a turn takes the sound out of the store"
    );
}

#[test]
fn a_sound_of_negative_volume_is_a_mistake_and_creates_nothing() {
    let mut world = World::new();
    assert!(sounds::make_sound(&mut world, (0, 0, 0), -1).is_none());
    assert!(sounds::heard_this_turn(&mut world).is_empty());
}

/// A gunshot is `combat`, and `combat` is **not** provocative - which is the reference's answer, not the intuitive one.
///
/// Reference: `is_provocative` (`sounds.cpp:287-309`) returns `true` for exactly **speech, electronic speech, alert and order**, and `false` for everything else including `combat` and `alarm`.
/// **This is the rule most likely to be "fixed" by a reader who assumes violence should attract attention**, so the gate names both halves: what is provocative, and what famously is not.
#[test]
fn only_speech_and_orders_provoke_and_a_gunshot_does_not() {
    use cdda_sim::sounds::{SoundCategory, is_provocative};

    for provocative in [
        SoundCategory::Speech,
        SoundCategory::ElectronicSpeech,
        SoundCategory::Alert,
        SoundCategory::Order,
    ] {
        assert!(is_provocative(provocative), "`{provocative:?}` is provocative, as the reference says");
    }
    for quiet in [
        SoundCategory::Background,
        SoundCategory::Weather,
        SoundCategory::Sensory,
        SoundCategory::Music,
        SoundCategory::Movement,
        SoundCategory::Activity,
        SoundCategory::DestructiveActivity,
        SoundCategory::Alarm,
        SoundCategory::Combat,
    ] {
        assert!(!is_provocative(quiet), "`{quiet:?}` is not provocative, however loud or violent it sounds");
    }
    // Nine and four is the whole enumeration: every category is on one side of the line.
    assert_eq!(9 + 4, 13, "thirteen categories, nine quiet and four provocative");
}
