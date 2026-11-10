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
        13,
        "thirteen effects are ported, out of the {} the game names",
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
        &[("fd_null", 0, 3), ("fd_fatigue", 30, 3), ("fd_fire", 14, 3)],
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

/// An `attack` spell damages every creature its area covers, and the blast decides who, not the range.
///
/// Reference: `spell_effect::attack` (`magic_spell_effect.cpp:530-545`) hands each creature in the area to `damage_targets`.
/// The port splits that into `damage_one`, so the area is what reaches a target - a spell with a blast of one hits the
/// creature next to the centre, and a spell with a blast of zero hits nobody, however close the caster stands.
#[test]
fn an_attack_spell_hits_everything_its_blast_reaches() {
    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let near = cdda_sim::creature::spawn_creature(&mut world, "near", (1, 0, 0));
    let far = cdda_sim::creature::spawn_creature(&mut world, "far", (9, 0, 0));
    for creature in [caster, near, far] {
        world.entity_mut(creature).insert(cdda_sim::creature::Body {
            parts: vec![cdda_sim::hit_location::BodyPart {
                name: "torso".to_owned(),
                hit_size: 1.0,
                hit_difficulty: 1.0,
                upper: false,
                lower: false,
                ..Default::default()
            }],
        });
        world
            .entity_mut(creature)
            .insert(cdda_sim::creature::Health { hp: 100, max_hp: 100 });
    }

    let mut spell = a_spell("attack", 10, 1);
    spell.effect_str = String::new();
    let outcome = spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0));
    assert_eq!(outcome, Effective::Done, "the neighbour was hit");

    let hurt = |world: &World, who: Entity| {
        world
            .get::<cdda_sim::creature::Health>(who)
            .map_or(100, |health| health.hp)
    };
    assert!(hurt(&world, near) < 100, "the neighbour lost health");
    assert_eq!(hurt(&world, far), 100, "and the creature out of the blast did not");
    assert_eq!(hurt(&world, caster), 100, "nor did the caster");

    // A blast of nothing reaches nobody, which is `NothingAffected` rather than a cast that silently did nothing.
    let mut empty = a_spell("attack", 10, 0);
    empty.effect_str = String::new();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &empty, 0, (0, 0, 0)),
        Effective::NothingAffected
    );
}

/// An `emit` spell puts its field on every square its blast reaches, asking the chance once per square.
///
/// Reference: `spell_effect::emit` (`magic_spell_effect.cpp:1723-1729`) with `map::emit_field` (`emit.cpp`): the area decides
/// the squares, the emission decides the field, the intensity and the chance - and the chance is drawn per square, so a thin
/// emission covers part of the blast rather than all or none of it.
#[test]
fn an_emit_spell_fills_its_blast_with_the_field_the_emission_names() {
    let mut world = World::new();
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0, 3), ("fd_toxic_gas", 20, 3)]);
    cdda_sim::emit_registry::set_emits(
        &mut world,
        &[(
            "emit_toxic_blast".to_owned(),
            vec![cdda_content::emit::Emission {
                field: "fd_toxic_gas".to_owned(),
                intensity: 1,
                chance: 100,
                qty: 1,
            }],
        )],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));

    let mut spell = a_spell("emit", 0, 1);
    spell.effect_str = "emit_toxic_blast".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );

    // A blast of one covers nine squares, and a certain emission fills all nine.
    let gas = cdda_core_types::field::FieldId(20);
    let mut filled = 0;
    for x in -1..=1 {
        for y in -1..=1 {
            if cdda_sim::has_field_at(&world, (x, y, 0), gas) {
                filled += 1;
            }
        }
    }
    assert_eq!(filled, 9, "every square of the blast holds the gas");

    // An emission id the port does not know is nothing affected, not a silent success.
    let mut unknown = a_spell("emit", 0, 1);
    unknown.effect_str = "emit_nothing_at_all".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &unknown, 0, (0, 0, 0)),
        Effective::NothingAffected
    );
}

/// An emission of a quantity is poured out, which is what the game's `emit` spells actually ask for.
///
/// Reference: `map::emit_field` (`map_field.cpp:2212-2224`): a chance, then `propagate_field` with the emission's quantity as
/// the amount. The six `emit` spells name emissions like `emit_toxic_blast`, whose `qty` is what makes the blast a cloud
/// rather than one thick square - so a port that placed a single unit would get every one of them wrong while looking right.
#[test]
fn an_emission_of_a_quantity_spreads_over_the_squares_it_can_reach() {
    let mut world = World::new();
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0, 3), ("fd_toxic_gas", 20, 3)]);
    cdda_sim::emit_registry::set_emits(
        &mut world,
        &[(
            "emit_toxic_blast".to_owned(),
            vec![cdda_content::emit::Emission {
                field: "fd_toxic_gas".to_owned(),
                intensity: 3,
                chance: 100,
                qty: 12,
            }],
        )],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));

    let mut spell = a_spell("emit", 0, 0);
    spell.effect_str = "emit_toxic_blast".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );

    let gas = cdda_core_types::field::FieldId(20);
    let mut squares = 0;
    let mut total = 0;
    for x in -4..=4 {
        for y in -4..=4 {
            let here = cdda_sim::field_strength_at(&world, (x, y, 0), gas);
            if here > 0 {
                squares += 1;
                total += here;
                assert!(here <= 3, "and no square exceeds the emission's intensity");
            }
        }
    }
    assert!(squares > 1, "the gas spread over {squares} squares, not one");
    assert_eq!(total, 12, "holding exactly the quantity the emission stated");
}

/// **A spell can hand over to a program, and the program knows where it happened.**
///
/// Reference: `spell_effect::effect_on_condition` (`magic_spell_effect.cpp:1910-1943`): every square of the area that holds a valid target runs the program the spell
/// names, with `spell_location` written into the dialogue's context and the **victim as the speaker**, the caster as the listener. 63 of the game's spells are this
/// effect, which makes it the largest single unported name in the data.
#[test]
fn an_effect_on_condition_spell_runs_its_program_with_the_square_it_stands_on() {
    let mut world = World::new();
    let caster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    let victim = world.spawn(cdda_sim::Position { x: 1, y: 0, z: 0 }).id();
    for who in [caster, victim] {
        world.entity_mut(who).insert(cdda_sim::creature::Creature {
            name: "a survivor".to_owned(),
            size: cdda_sim::CreatureSize::Medium,
        });
    }
    cdda_sim::PositionIndex::rebuild(&mut world);
    // A program with no effects at all: what this gate reads is the **variable the reference writes before it runs**, so the program only has to be
    // there and be runnable.
    let program = cdda_json::parse(
        br#"[{"id": "EOC_gate_mark", "type": "effect_on_condition", "effect": [{"u_message": "done"}]}]"#,
    )
    .expect("the program is json");
    let cdda_json::Value::Array(entries) = &program else {
        panic!("a list");
    };
    let programs: Vec<cdda_content::dialogue::Program> = entries
        .iter()
        .filter_map(|entry| cdda_content::dialogue::read_program(entry))
        .collect();
    assert_eq!(programs.len(), 1, "the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &programs);

    let mut spell = a_spell("effect_on_condition", 1, 1);
    spell.effect_str = "EOC_gate_mark".to_owned();
    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (1, 0, 0)),
        Effective::Done,
        "the effect ran for the square the victim stands on"
    );
    let stored = cdda_sim::dialogue_values::value_of(&world, victim, "spell_location");
    assert_eq!(
        stored,
        Some(cdda_sim::dialogue_values::Value::Position { x: 1, y: 0, z: 0 }),
        "**and the program's own talker was told the square as `spell_location`**, the variable the reference writes before it runs"
    );
}
