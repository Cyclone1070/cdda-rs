//! Whether a dialogue condition holds, which is what every effect-on-condition asks first.
//!
//! Reference: `conditional_t` (`condition.cpp:2660-2760`). This is the first half of the `effect_on_condition` machine that
//! 63 of the game's spells name: a spell that is an EOC is a program, and a program starts by asking a question.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::dialogue::{Condition, read_condition};
use cdda_sim::applied_effects;
use cdda_sim::conditions::holds;
use cdda_sim::effects::ActiveEffect;
use std::collections::BTreeMap;

fn condition(json: &str) -> Condition {
    let value = cdda_json::parse(json.as_bytes()).expect("valid json");
    read_condition(&value).expect("the port reads this condition")
}

fn wearing(world: &mut World, creature: Entity, id: &str, intensity: i32) {
    applied_effects::add(
        world,
        creature,
        ActiveEffect {
            id: id.to_owned(),
            intensity,
            duration: 100,
            permanent: false,
        },
    );
}

#[test]
fn an_effect_condition_asks_what_the_creature_is_wearing() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let bleeding = condition(r#"{"u_has_effect": "effect_bleed"}"#);

    assert!(!holds(&world, survivor, &bleeding), "nothing is worn yet");
    wearing(&mut world, survivor, "effect_bleed", 1);
    assert!(
        holds(&world, survivor, &bleeding),
        "and then the question is answered yes"
    );
}

#[test]
fn a_stated_intensity_is_a_floor_not_an_exact_match() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    wearing(&mut world, survivor, "effect_bleed", 2);

    let any = condition(r#"{"u_has_effect": "effect_bleed"}"#);
    let two = condition(r#"{"u_has_effect": "effect_bleed", "intensity": 2}"#);
    let three = condition(r#"{"u_has_effect": "effect_bleed", "intensity": 3}"#);

    assert!(holds(&world, survivor, &any));
    assert!(holds(&world, survivor, &two), "two asks for at least two");
    assert!(
        !holds(&world, survivor, &three),
        "and three asks for more than is worn: the reference compares `intensity <= worn`, not equality"
    );
}

#[test]
fn and_or_and_not_are_the_reference_s_three_combinators() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    wearing(&mut world, survivor, "effect_bleed", 1);

    let both =
        condition(r#"{"and": [{"u_has_effect": "effect_bleed"}, {"u_has_effect": "effect_stun"}]}"#);
    let either =
        condition(r#"{"or": [{"u_has_effect": "effect_bleed"}, {"u_has_effect": "effect_stun"}]}"#);
    let neither = condition(r#"{"not": {"u_has_effect": "effect_stun"}}"#);

    assert!(!holds(&world, survivor, &both), "and needs every part");
    assert!(holds(&world, survivor, &either), "or needs one");
    assert!(holds(&world, survivor, &neither), "not inverts");
}

#[test]
fn a_condition_the_port_cannot_read_is_refused_rather_than_answered() {
    // A body part makes the question one the port's effects cannot answer: they belong to the creature, not to a part.
    let with_part = cdda_json::parse(br#"{"u_has_effect": "effect_bleed", "bodypart": "arm_l"}"#.as_slice())
        .expect("valid json");
    assert!(
        read_condition(&with_part).is_none(),
        "the reader refuses what it cannot evaluate"
    );
    // And a verb nobody has ported is refused too.
    let a_trait = cdda_json::parse(br#"{"u_has_trait": "TRAIT_FASTHEALER"}"#.as_slice()).expect("valid json");
    assert!(read_condition(&a_trait).is_none());
    // A combination is only as readable as its parts.
    let mixed = cdda_json::parse(
        br#"{"and": [{"u_has_effect": "effect_bleed"}, {"u_has_trait": "TRAIT_X"}]}"#.as_slice(),
    )
    .expect("valid json");
    assert!(
        read_condition(&mixed).is_none(),
        "one unreadable part makes the whole unreadable, or the answer would be quietly weaker"
    );
}

/// Every `condition` member anywhere in a file, at any depth: the reference reads them wherever they sit.
fn count_conditions(
    value: &cdda_json::Value,
    seen: &mut usize,
    readable: &mut usize,
    refused: &mut BTreeMap<String, usize>,
) {
    match value {
        cdda_json::Value::Object(object) => {
            if let Some(condition) = object.get("condition")
                && let cdda_json::Value::Object(inner) = condition
            {
                *seen += 1;
                if read_condition(condition).is_some() {
                    *readable += 1;
                } else {
                    for (key, _) in inner.members() {
                        if key != "intensity" && key != "bodypart" {
                            *refused.entry(key.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
            for (_, member) in object.members() {
                count_conditions(member, seen, readable, refused);
            }
        }
        cdda_json::Value::Array(entries) => {
            for entry in entries {
                count_conditions(entry, seen, readable, refused);
            }
        }
        _ => {}
    }
}

/// How much of the game's dialogue the port can answer, counted against the data.
///
/// Every `condition` object in the JSON is a question the game asks. The sweep counts them all and how many the port can
/// read, so "the condition machine is started" is a number rather than a claim - and so the next verb to port is chosen by
/// how often it appears rather than by which one looked easiest.
#[test]
fn the_port_answers_the_questions_it_can_and_says_how_many_that_is() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let files = cdda_content::json_files_in_load_order(&data).expect("the data directory is there");

    let mut seen = 0usize;
    let mut readable = 0usize;
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    for path in &files {
        let Some(value) = cdda_content::read_json(path) else {
            continue;
        };
        count_conditions(&value, &mut seen, &mut readable, &mut refused);
    }

    let mut top: Vec<(&String, &usize)> = refused.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    let top: Vec<(&String, &usize)> = top.into_iter().take(5).collect();
    println!("conditions: {seen} found, {readable} readable; most common refusals: {top:?}");

    // The numbers are the measurement, and the sequence is the lesson: the round that added the expression *tree* moved
    // nothing (435), the round that added the string parser moved it to 449, and the one-line fix that accepted the data's
    // own shape - `"math": ["one string"]`, a one-element array, not only the `[left, op, right]` triple - moved it to 768.
    // What is left in `math` (263) is mostly calls into the reference's function table, `u_effect_intensity(...)` and
    // friends, each of which refuses the whole expression until the port knows the name.
    //
    // Up to 1,500 with the conversation's two actors: `n_val` reads the listener. The gain from that was six conditions,
    // because the strings that name `n_val` also call `weather(...)` and friends, and one unknown name refuses the whole
    // expression - which is the price of refusing honestly, and the reason the next leaves to port are functions rather
    // than more shapes.
    //
    // 3,708 questions in the data and 1,300 the port can answer today, after `compare_string` - a repeat-search over a
    // list, not a comparison of two values - took it from 768. `not` fell from 392 refusals to 258 for the same reason: a
    // combinator is only as readable as its parts, so every leaf ported is worth more than itself. `and` and `not` lead
    // the refusals because a combinator is only as readable as its parts, and the parts refused most are `math` (518),
    // `compare_string` (238) and `u_has_items` (155) - which is the list of what to port next, by frequency.
    assert_eq!(seen, 3708, "the data asks 3,708 `condition` objects");
    assert_eq!(
        readable, 1501,
        "of which the port answers 1,501: `u_has_effect`, `compare_string`, `u_has_items`, the combinators, and the `math` expressions whose every name it knows"
    );
}

/// `compare_string` holds when a string comes round twice, not when two chosen strings are equal.
///
/// Reference: `conditional_fun::f_compare_string` (`condition.cpp:1900-1920`) - it walks the list, remembering what it has
/// seen, and returns true the moment a repeat appears. The name reads like a comparison and the rule is a search, which is
/// exactly the kind of difference a port gets wrong by trusting the name.
#[test]
fn compare_string_looks_for_a_repeat_anywhere_in_the_list() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "chosen_side",
        cdda_sim::dialogue_values::Value::Text("marloss".to_owned()),
    );

    let repeats = condition(r#"{"compare_string": ["marloss", "mycus", "marloss"]}"#);
    assert!(holds(&world, survivor, &repeats), "the first and last match");

    let all_different = condition(r#"{"compare_string": ["marloss", "mycus", "zombie"]}"#);
    assert!(!holds(&world, survivor, &all_different));

    // A variable is read when the question is asked, and a literal that equals it is a repeat.
    let with_variable = condition(r#"{"compare_string": [{"u_val": "chosen_side"}, "marloss"]}"#);
    assert!(holds(&world, survivor, &with_variable));

    // One string on its own can never repeat, and a variable the store cannot answer makes the condition false.
    let alone = condition(r#"{"compare_string": ["marloss"]}"#);
    assert!(!holds(&world, survivor, &alone));
    let unset = condition(r#"{"compare_string": [{"u_val": "never_set"}, "marloss"]}"#);
    assert!(!holds(&world, survivor, &unset));
}

/// `u_has_items` counts what the creature actually carries, in stacks and in charges.
///
/// Reference: `conditional_fun::f_has_items` (`condition.cpp:836-865`). An object with neither `count` nor `charges` answers
/// false without asking the inventory anything, and the port refuses to read it at all rather than answering a question the
/// reference answers for a reason the port does not share.
#[test]
fn has_items_counts_the_stacks_and_the_charges_a_creature_carries() {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "rag",
                volume_ml: 250,
                weight_g: 100,
                pockets: &[],
            },
            cdda_sim::item_registry::Kind {
                id: "rope",
                volume_ml: 500,
                weight_g: 200,
                pockets: &[],
            },
        ],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for _ in 0..2 {
        let item = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(item).insert(cdda_sim::creature::Carried(survivor));
    }

    let two_rags = condition(r#"{"u_has_items": {"item": "rag", "count": 2}}"#);
    assert!(holds(&world, survivor, &two_rags), "two rag stacks are carried");

    let three_rags = condition(r#"{"u_has_items": {"item": "rag", "count": 3}}"#);
    assert!(!holds(&world, survivor, &three_rags), "and a third is not");

    let two_charges = condition(r#"{"u_has_items": {"item": "rag", "charges": 2}}"#);
    assert!(holds(&world, survivor, &two_charges), "each stack holds one charge here, so two stacks are two charges");

    let some_rope = condition(r#"{"u_has_items": {"item": "rope", "count": 1}}"#);
    assert!(!holds(&world, survivor, &some_rope), "no rope is carried");

    // A requirement stating neither a count nor charges is not read: the reference answers false, the port refuses.
    let empty = cdda_json::parse(br#"{"u_has_items": {"item": "rag"}}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_condition(&empty).is_none());

    // A variable can name the item, which is what `str_or_var` means everywhere else in the data.
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "wanted",
        cdda_sim::dialogue_values::Value::Text("rag".to_owned()),
    );
    let by_variable = condition(r#"{"u_has_items": {"item": {"u_val": "wanted"}, "count": 2}}"#);
    assert!(holds(&world, survivor, &by_variable));
}

/// A conversation has two people, and `n_val` reads the other one.
///
/// Reference: a `const_dialogue` holds `actor( false )` (the player) and `actor( true )` (the NPC), and `read_var_info`
/// (`condition.cpp:408-437`) classes `u_val` as one and `n_val`/`npc_val` as the other. The port had one actor, which made
/// every question about the NPC unanswerable - and unanswerable is false, not "the player's value".
#[test]
fn the_npc_s_variables_are_not_the_player_s() {
    use cdda_sim::conditions::holds_with;

    let mut world = World::new();
    let player = cdda_sim::creature::spawn_creature(&mut world, "the player", (0, 0, 0));
    let trader = cdda_sim::creature::spawn_creature(&mut world, "the trader", (1, 0, 0));
    cdda_sim::dialogue_values::set(&mut world, player, "price", cdda_sim::dialogue_values::Value::Number(10.0));
    cdda_sim::dialogue_values::set(&mut world, trader, "price", cdda_sim::dialogue_values::Value::Number(25.0));

    let asks_the_npc = condition(r#"{"math": ["n_val('price')", "==", 25]}"#);
    let asks_the_player = condition(r#"{"math": ["u_val('price')", "==", 10]}"#);

    assert!(
        holds_with(&world, player, Some(trader), &asks_the_npc),
        "n_val is the other actor's"
    );
    assert!(
        holds_with(&world, player, Some(trader), &asks_the_player),
        "u_val is the speaker's"
    );
    assert!(
        !holds(&world, player, &asks_the_npc),
        "with nobody to talk to, a question about the NPC is unanswerable - which is false, not the player's value"
    );
}

/// The math functions are the reference's, constants and all.
///
/// Reference: `math_parser_func.h:141-163` for the table and `units.h:397-424` for the four temperature conversions. The
/// conversions are the ones most worth pinning, because their constants are spelled in the reference and a port that
/// "cleaned them up" to 273.15 would be wrong by a hundredth of a degree everywhere.
#[test]
fn the_math_functions_answer_what_the_reference_s_table_answers() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // 300 kelvin is 26.85 celsius and 80.33 fahrenheit, and each converts back.
    let kelvin = condition(r#"{"math": ["celsius(300)", ">", 26.84]}"#);
    assert!(holds(&world, survivor, &kelvin));
    let fahrenheit = condition(r#"{"math": ["fahrenheit(300)", ">", 80.32]}"#);
    assert!(holds(&world, survivor, &fahrenheit));
    let back = condition(r#"{"math": ["from_celsius(celsius(300))", "==", 300]}"#);
    assert!(holds(&world, survivor, &back), "the two conversions invert each other");

    // Rounding, and the three-argument clamp.
    let rounded = condition(r#"{"math": ["round(2.5)", "==", 3]}"#);
    assert!(holds(&world, survivor, &rounded));
    let clamped = condition(r#"{"math": ["clamp(12, 0, 10)", "==", 10]}"#);
    assert!(holds(&world, survivor, &clamped));

    // The variadic pair.
    let largest = condition(r#"{"math": ["max(1, 9, 4)", "==", 9]}"#);
    assert!(holds(&world, survivor, &largest));
    let smallest = condition(r#"{"math": ["min(1, 9, 4)", "==", 1]}"#);
    assert!(holds(&world, survivor, &smallest));

    // A wrong number of arguments is refused, and so is a function the port has no rule for - including the dice, which
    // need rolls the reader does not hold.
    for text in [
        r#"{"math": ["clamp(1, 2)", "==", 1]}"#,
        r#"{"math": ["rng(1, 6)", "==", 1]}"#,
        r#"{"math": ["weather(temperature)", "==", 1]}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_condition(&value).is_none(),
            "{text} must be refused rather than answered"
        );
    }
}
