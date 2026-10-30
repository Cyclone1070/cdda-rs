//! What a spell does when it lands.
//!
//! Reference: `magic_spell_effect.cpp`, which has one function per `effect_name`. The port has one of them so far, and
//! the count is the gate: a table of effects has to say how long it is, or "we ported some effects" stops meaning
//! anything.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;
use cdda_sim::spell_effects::{self, Effective};

/// A scaling that does not change with level, which is what most of the test's numbers are.
fn scaling(value: i64) -> cdda_content::spells::Scaling {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    cdda_content::spells::Scaling {
        start: Some(as_float),
        increment: Some(0.0),
        end: Some(as_float),
    }
}

fn a_spell(effect_name: &str, damage: i64, area: i64) -> Spell {
    Spell {
        id: "test_spell".to_owned(),
        name: "Test Spell".to_owned(),
        effect_name: effect_name.to_owned(),
        effect_str: String::new(),
        damage_type: "pure".to_owned(),
        shape: "blast".to_owned(),
        energy_source: String::new(),
        damage: scaling(damage),
        range: scaling(3),
        area: scaling(area),
        energy: scaling(0),
        casting_time: scaling(0),
        difficulty: Some(0.0),
        max_level: Some(10.0),
    }
}

#[test]
fn a_spell_that_changes_pace_changes_it_for_everyone_in_reach() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let near = cdda_sim::creature::spawn_creature(&mut world, "near", (1, 1, 0));
    let far = cdda_sim::creature::spawn_creature(&mut world, "far", (9, 9, 0));
    let spell = a_spell("mod_moves", 300, 2);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, near),
        300,
        "a creature two squares away is inside a blast of two"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, far),
        0,
        "and one at (9,9) is not"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, caster),
        0,
        "the caster is not its own target, as in the reference"
    );
}

#[test]
fn a_slow_a_soul_spell_takes_moves_away() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let victim = cdda_sim::creature::spawn_creature(&mut world, "victim", (0, 0, 0));
    cdda_sim::creature::give_moves(&mut world, victim, 500);
    let spell = a_spell("mod_moves", -200, 0);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, victim),
        300,
        "two hundred moves taken from five hundred"
    );
}

#[test]
fn an_effect_the_port_does_not_know_spends_nothing_and_says_so() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let other = cdda_sim::creature::spawn_creature(&mut world, "other", (0, 0, 0));
    let spell = a_spell("explosion", 5, 0);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::NotPorted,
        "`explosion` is in the data and not in the port, and it says so rather than pretending"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, other),
        0,
        "and nothing in the world changed"
    );
}

/// How much of the game's spell effects the port can perform, counted against the data.
///
/// Reference: `magic_spell_effect.cpp` has one function per `effect_name`, and every spell names one. A table of ported
/// effects without a count is a claim; with a count it is a fact that can only change deliberately.
#[test]
fn the_port_can_do_one_effect_of_the_ones_the_game_names() {
    use std::collections::BTreeSet;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");
    assert!(spells.len() > 200, "the game has {} spells", spells.len());

    let named: BTreeSet<&str> = spells
        .iter()
        .map(|spell| spell.effect_name.as_str())
        .collect();
    assert!(
        named.len() > 30,
        "the game names {} different effects",
        named.len()
    );
    for (name, _) in spell_effects::EFFECTS {
        assert!(
            named.contains(name),
            "the port claims `{name}`, which no spell in the data names"
        );
    }
    assert_eq!(
        spell_effects::EFFECTS.len(),
        3,
        "three effects are ported, out of the {} the game names",
        named.len()
    );
}

/// A summoning spell puts monsters on the ground, and the store knows what a monster id means.
///
/// Reference: `spell_effect::summon` (`magic_spell_effect.cpp`) spawns `sp.damage( caster )` monsters of the type the
/// spell names. The registry is the piece that makes that possible: `MonsterKindId` was a number with nothing behind it,
/// so a spell could not say "a `mon_zombie`" and have anything happen.
#[test]
fn a_summoning_spell_brings_the_monster_it_names() {
    use cdda_sim::monster_registry;
    use cdda_sim::{MonsterKindId, monster_registry as registry};

    let mut world = World::new();
    let kinds = [
        registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        },
        registry::MonsterKind {
            id: "mon_dog",
            hp: 25,
        },
    ];
    monster_registry::set_monster_types(&mut world, &kinds);
    assert_eq!(monster_registry::count(&mut world), 2);
    assert_eq!(
        monster_registry::id_of(&mut world, "mon_dog"),
        Some(MonsterKindId(2))
    );
    assert_eq!(
        monster_registry::name_of(&mut world, MonsterKindId(1)).as_deref(),
        Some("mon_zombie")
    );
    assert_eq!(monster_registry::hit_points(&mut world, MonsterKindId(1)), 80);

    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let mut spell = a_spell("summon", 3, 1);
    spell.effect_str = "mon_zombie".to_owned();

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (5, 5, 0)),
        Effective::Done,
        "three zombies is a summoning that happened"
    );
    let near: Vec<bevy_ecs::prelude::Entity> = (4..=6)
        .flat_map(|x| (4..=6).map(move |y| (x, y)))
        .flat_map(|square| cdda_sim::creature::creature_at(&world, (square.0, square.1, 0)))
        .collect();
    assert_eq!(near.len(), 3, "three creatures appeared around (5,5)");
    for zombie in &near {
        assert_eq!(
            world
                .get::<cdda_sim::creature::CreatureKind>(*zombie)
                .map(|kind| kind.0),
            Some(MonsterKindId(1)),
            "and every one of them is the kind the spell named"
        );
        assert_eq!(
            cdda_sim::creature::health_of(&world, *zombie),
            Some(80),
            "with the hit points the registry says"
        );
    }
}

/// A spell naming a monster the store does not have summons nothing rather than a nameless thing.
#[test]
fn summoning_something_unknown_brings_nothing() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let mut spell = a_spell("summon", 2, 1);
    spell.effect_str = "mon_does_not_exist".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (3, 3, 0)),
        Effective::NothingAffected
    );
    assert!(cdda_sim::creature::creature_at(&world, (3, 3, 0)).is_empty());
}

/// A spell that removes an effect takes it off everyone it reaches, and nothing else.
///
/// Reference: `spell_effect::remove_effect` (`magic_spell_effect.cpp:1712-1721`). This is the first effect that changes
/// something the port already tracked rather than adding state, so the gate has to show both halves: the named effect
/// goes, and the creature's other effects do not.
#[test]
fn a_spell_takes_one_effect_off_and_leaves_the_rest() {
    use cdda_sim::applied_effects::{self, AppliedEffects};
    use cdda_sim::effects::ActiveEffect;

    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let victim = cdda_sim::creature::spawn_creature(&mut world, "victim", (1, 0, 0));
    for id in ["effect_vomit", "effect_bleed"] {
        applied_effects::add(
            &mut world,
            victim,
            ActiveEffect {
                id: id.to_owned(),
                intensity: 2,
                duration: 10,
                permanent: false,
            },
        );
    }
    let mut spell = a_spell("remove_effect", 0, 1);
    spell.effect_str = "effect_vomit".to_owned();

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    let worn = world.get::<AppliedEffects>(victim).expect("still wearing something");
    assert_eq!(worn.0.len(), 1, "one effect came off and one stayed");
    assert_eq!(worn.0[0].id, "effect_bleed", "the bleeding stayed");
    assert_eq!(worn.0[0].intensity, 2, "and kept its numbers");

    // A second cast finds nothing to take off, and says so rather than claiming success.
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::NothingAffected
    );
}
