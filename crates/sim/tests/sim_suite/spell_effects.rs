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
        duration: scaling(10),
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
        4,
        "four effects are ported, out of the {} the game names",
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

/// A spell that removes a field takes it off every square it reaches, and leaves the rest.
///
/// Reference: `spell_effect::remove_field` (`magic_spell_effect.cpp:1010-1028`), whose one user is `AO_CLOSE_TEAR` removing
/// `fd_fatigue`. This needed the field names, which the port had as numbers and could not look up: a spell names a field
/// and nothing in the store could say which number that was.
#[test]
fn a_spell_takes_one_field_off_the_squares_it_reaches() {
    use cdda_core_types::field::FieldId;
    use cdda_sim::field_registry;

    let mut world = World::new();
    // The first names in the reference's registry order, which is what the numbers mean.
    field_registry::set_field_names(
        &mut world,
        &[("fd_null", 0), ("fd_fatigue", 30), ("fd_fire", 14)],
    );
    assert_eq!(field_registry::count(&mut world), 3);
    assert_eq!(
        field_registry::id_of(&mut world, "fd_fatigue"),
        Some(FieldId(30))
    );
    assert_eq!(
        field_registry::name_of(&mut world, FieldId(14)).as_deref(),
        Some("fd_fire")
    );

    let tear: FieldId = field_registry::id_of(&mut world, "fd_fatigue").expect("named");
    let fire: FieldId = field_registry::id_of(&mut world, "fd_fire").expect("named");
    cdda_sim::add_field_strength(&mut world, (0, 0, 0), tear, 1);
    cdda_sim::add_field_strength(&mut world, (1, 0, 0), tear, 1);
    cdda_sim::add_field_strength(&mut world, (0, 0, 0), fire, 2);
    cdda_sim::add_field_strength(&mut world, (9, 9, 0), tear, 1);

    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let mut spell = a_spell("remove_field", 0, 1);
    spell.effect_str = "fd_fatigue".to_owned();

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    assert!(
        !cdda_sim::has_field_at(&world, (0, 0, 0), tear),
        "the tear at the centre is gone"
    );
    assert!(
        !cdda_sim::has_field_at(&world, (1, 0, 0), tear),
        "and so is the one beside it, inside a blast of one"
    );
    assert!(
        cdda_sim::has_field_at(&world, (0, 0, 0), fire),
        "the fire on the same square stays"
    );
    assert!(
        cdda_sim::has_field_at(&world, (9, 9, 0), tear),
        "and a tear nine squares away is out of reach"
    );

    // A field name the store does not know removes nothing and says so.
    spell.effect_str = "fd_not_a_field".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::NothingAffected
    );
}

/// The field numbers the port already used are the oracle's own ordering.
///
/// Fourteen field constants existed before the registry did - `FIRE` at 14, `HOT_AIR_4` at 61 and the rest, each picked
/// out of the reference while porting fire and gas. Now that `corpus/field_types.json` carries the reference's list, the
/// two can be compared: every constant must be the position of its name **plus one**, and the count says how many were
/// checked. If the offset were wrong by one, all fourteen would disagree at once.
#[test]
fn the_field_numbers_the_port_uses_are_the_oracles_order() {
    use cdda_core_types::field::FieldId;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/field_types.json");
    let text = std::fs::read_to_string(path).expect("the field dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let cdda_json::Value::Object(root) = &value else {
        panic!("a field dump is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("prototypes") else {
        panic!("a field dump has prototypes");
    };
    assert_eq!(rows.len(), 145, "the reference has 145 field types");

    // Each prototype carries its own number, which is the answer a position cannot give: the list holds an entry whose
    // place and whose number disagree, so counting positions would be wrong for it and everything after.
    let mut by_name: Vec<(&str, u32)> = Vec::with_capacity(rows.len());
    for row in rows {
        let cdda_json::Value::Object(object) = row else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = object.get("id") else {
            continue;
        };
        let Some(cdda_json::Value::Int(num)) = object.get("num") else {
            panic!("{id} states its number");
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        by_name.push((id.as_str(), *num as u32));
    }
    assert_eq!(by_name.len(), 145, "every one of the 145 fields states a number");

    let constants: [(&str, FieldId); 14] = [
        ("fd_fire", cdda_sim::FIRE),
        ("fd_smoke", cdda_sim::SMOKE),
        ("fd_extinguisher", cdda_sim::EXTINGUISHER),
        ("fd_flame_burst", cdda_sim::FLAME_BURST),
        ("fd_fire_vent", cdda_sim::FIRE_VENT),
        ("fd_acid_vent", cdda_sim::ACID_VENT),
        ("fd_shock_vent", cdda_sim::SHOCK_VENT),
        ("fd_electricity", cdda_sim::ELECTRICITY),
        ("fd_incendiary", cdda_sim::INCENDIARY),
        ("fd_push_items", cdda_sim::PUSH_ITEMS),
        ("fd_hot_air1", cdda_sim::HOT_AIR_1),
        ("fd_hot_air2", cdda_sim::HOT_AIR_2),
        ("fd_hot_air3", cdda_sim::HOT_AIR_3),
        ("fd_hot_air4", cdda_sim::HOT_AIR_4),
    ];
    for (name, constant) in constants {
        let from_the_data = by_name
            .iter()
            .find(|(listed, _)| *listed == name)
            .unwrap_or_else(|| panic!("{name} is in the reference's list"))
            .1;
        assert_eq!(
            constant.0, from_the_data,
            "{name} is numbered {from_the_data} by the reference and {} by the port",
            constant.0
        );
    }
}
