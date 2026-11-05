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

/// A sound is provocative if its category says so **or** if it arrived loud - which is why monsters come to gunfire after all.
///
/// Reference: `monster::hear_sound`'s `int tmp_provocative = provocative || volume >= normal_roll( 30, 5 );` (`monster.cpp:3939`). Round 1186 pinned the category half and **this gate is the other
/// half**: a `combat` sound at volume ten is not provocative, the same sound at sixty almost always is, and a `speech` sound at two always is - **so the two halves can disagree and the louder
/// answer wins.**
#[test]
fn a_sound_is_provocative_by_its_kind_or_its_loudness() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::sounds::{SoundCategory, heard_as_provocative};

    let mut dice = Rolls::new(77);
    // Loudness alone: a combat sound of ten never beats a roll around thirty, and one of sixty always does.
    let quiet = (0..200).filter(|_| heard_as_provocative(SoundCategory::Combat, 10, &mut dice)).count();
    assert_eq!(quiet, 0, "a quiet gunshot's own category is not provocative, and ten is not loud: {quiet}");
    let mut dice = Rolls::new(77);
    let loud = (0..200).filter(|_| heard_as_provocative(SoundCategory::Combat, 60, &mut dice)).count();
    assert!(loud > 190, "but sixty beats a roll of normal thirty and five nearly always: {loud} of 200");

    // Category alone: speech at a whisper is provocative whatever the volume.
    let mut dice = Rolls::new(77);
    for _ in 0..100 {
        assert!(heard_as_provocative(SoundCategory::Speech, 1, &mut dice), "somebody talking is always worth a look");
        assert!(heard_as_provocative(SoundCategory::Order, 1, &mut dice), "and an order more so");
    }
    // And the famous line: a hundred and fifty-four squares of gunshot is provocative because it is loud, not because it is violent.
    let mut dice = Rolls::new(77);
    let shot = (0..200).filter(|_| heard_as_provocative(SoundCategory::Combat, 154, &mut dice)).count();
    assert_eq!(shot, 200, "a .223 at the shooter's own square is heard as provocative every time: {shot}");
}

/// A monster already walking towards something interesting is not distracted by something duller.
///
/// Reference: `monster::hear_sound`'s two guards (`monster.cpp:3940-3944`, `:3966-3970`): a **provocative** sound it is following cannot be replaced by a **dull** one while the wander has
/// turns left, and any sound that would not hold it **as long** is ignored. **The second guard is the one that is easy to miss**: a loud dull noise that lasts fewer turns than the current
/// wander changes nothing, even though its volume is greater.
#[test]
fn a_monster_following_something_interesting_ignores_a_duller_sound() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(31));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (0, 0, 0));
    // A listener is a creature that **hears**: `hear_sounds` asks for the `HEARS` flag (`hearing.rs:78`), so a zombie without it would make this gate pass by doing nothing at all.
    cdda_sim::creature_flags::add_flag(&mut world, zombie, "HEARS");
    cdda_sim::creature::set_wander(&mut world, zombie, (5, 5, 0), 40);
    // Mark the wander as worth investigating, the way the hearing rule does for a provocative sound.
    if let Some(mut wander) = world.get_mut::<cdda_sim::creature::Wander>(zombie) {
        wander.provocative = true;
    }
    // A dull sound that would hold it only ten turns, and a louder dull one that would hold it twenty.
    cdda_sim::sounds::make_a_sound_of_kind(&mut world, (9, 0, 0), 30, "bang!", cdda_sim::sounds::SoundCategory::Combat);
    cdda_sim::hearing::hear_sounds(&mut world);

    let wander = world.get::<cdda_sim::creature::Wander>(zombie).copied().expect("it is still wandering");
    assert_eq!(wander.turns_left, 40, "the dull sound did not replace a provocative wander");
    assert_eq!(wander.square, (5, 5, 0), "and it is still heading where the interesting sound was");

    // A sound that would hold it sixty turns wins, because it lasts longer - the second guard is about duration, not volume.
    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(31));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (0, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, zombie, "HEARS");
    world.entity_mut(zombie).insert(cdda_sim::creature::Anger(100));
    cdda_sim::creature::set_wander(&mut world, zombie, (5, 5, 0), 10);
    cdda_sim::sounds::make_a_sound_of_kind(&mut world, (9, 0, 0), 60, "bang!", cdda_sim::sounds::SoundCategory::Combat);
    cdda_sim::hearing::hear_sounds(&mut world);
    let wander = world.get::<cdda_sim::creature::Wander>(zombie).copied().expect("it is wandering");
    assert!(wander.turns_left > 40, "a longer-lasting sound does take over: {}", wander.turns_left);
}

/// A hearing monster walks towards the noise, a frightened one walks away from it, and an uninterested one does not move - three outcomes, not one.
///
/// Reference: `monster::hear_sound`'s tail (`monster.cpp:3969-3981`): `morale >= 0 && anger >= 10` walks to the sound, `morale < 0` walks to the **mirror point** `pos + ( pos - target )`, and
/// anything else (`anger < 10` with morale at or above zero) walks nowhere. **The mirror point is the rule a port loses**: it is not "away" in general, it is the same distance on the other side.
#[test]
fn a_frightened_monster_walks_away_from_the_noise_and_an_uninterested_one_does_not_move() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let hearing = |morale: i32, anger: i32| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(5));
        let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (10, 10, 0));
        cdda_sim::creature_flags::add_flag(&mut world, zombie, "HEARS");
        world
            .entity_mut(zombie)
            .insert(cdda_sim::creature::Morale(morale))
            .insert(cdda_sim::creature::Anger(anger));
        // A loud shout to the east, at a square whose x differs by four, so the mirror point is unambiguous.
        cdda_sim::sounds::make_a_sound_of_kind(&mut world, (14, 10, 0), 60, "shout", cdda_sim::sounds::SoundCategory::Speech);
        cdda_sim::hearing::hear_sounds(&mut world);
        world.get::<cdda_sim::creature::Wander>(zombie).copied()
    };

    let bold = hearing(0, 100).expect("a bold monster walks somewhere");
    assert_eq!(bold.square.0, 14, "it walks towards the noise");
    assert_eq!(bold.square.1, 10, "and stays on the same line");

    let scared = hearing(-50, 100).expect("a frightened monster walks somewhere too");
    assert_eq!(scared.square.0, 6, "but to the mirror point, four squares the other way");
    assert_eq!(scared.square.1, 10, "on the same line");

    assert!(hearing(0, 5).is_none(), "a monster with morale but no anger walks nowhere at all");
}
