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
    // A body part **is** read now: the port's effects belong to the creature, so a question about a part has one answer it can give, and refusing it made three
    // programs unreadable for a distinction the port cannot make either way. `a_has_effect_with_a_bodypart_answers_about_the_creature` pins what it answers.
    let with_part = cdda_json::parse(br#"{"u_has_effect": "effect_bleed", "bodypart": "arm_l"}"#.as_slice())
        .expect("valid json");
    assert!(
        read_condition(&with_part).is_some(),
        "a named part does not make the question unreadable"
    );

    // And a verb nobody has ported is refused too. This example has moved twice now - `u_has_trait`, then `u_has_flag` - because each one answered
    // as its rule landed, which is what the test is for and why it was worth keeping rather than deleting.
    let unported = cdda_json::parse(br#"{"u_is_on_furniture": "f_chair"}"#.as_slice()).expect("valid json");
    assert!(read_condition(&unported).is_none());
    // A combination is only as readable as its parts.
    let mixed = cdda_json::parse(
        br#"{"and": [{"u_has_effect": "effect_bleed"}, {"u_is_on_furniture": "f_chair"}]}"#.as_slice(),
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
    // 2,094 after `is_weather` - five, because most of the 41 `is_weather` guards in the data sit inside programs whose effects
    // still refuse, so they are counted where they are asked and not where they would have mattered. 2,089 after `u_has_mission` — 241 more than the 1,848 that `u_has_trait` reached, which was itself 347 more than the 1,501
    // before it. That is the whole point of a leaf: a combinator is only as readable as its parts, so one ported leaf is worth
    // several times itself.
    assert_eq!(
        readable, crate::numbers::NOW.conditions_readable,
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
                thrown_damage: 0.0,
            },
            cdda_sim::item_registry::Kind {
                id: "rope",
                volume_ml: 500,
                weight_g: 200,
                pockets: &[],
                thrown_damage: 0.0,
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

    // The dice parse like any other function and are refused at **run** time, because a draw changes the store and a guard
    // holds only `&World`. A guard asked here answers nothing rather than a zero.
    let dice = condition(r#"{"math": ["rng(1, 6)", "==", 1]}"#);
    assert!(
        !holds(&world, survivor, &dice),
        "a guard with no dice to offer cannot be answered"
    );

    // A wrong number of arguments is refused at **read**, because the port knows that function's arity. A name the port has no rule for is a different matter:
    // the reference parses any name and fails when it evaluates (`math_parser`), which is how the data's own functions are called after they load, so the port
    // reads it and refuses where it answers.
    let wrong_arity = cdda_json::parse(br#"{"math": ["clamp(1, 2)", "==", 1]}"#.as_slice()).expect("json");
    assert!(
        cdda_content::dialogue::read_condition(&wrong_arity).is_none(),
        "a known function with the wrong number of arguments is refused at read"
    );
    let unknown_name = cdda_json::parse(br#"{"math": ["weather(temperature)", "==", 1]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&unknown_name).expect("an unknown name reads");
    let mut world = bevy_ecs::prelude::World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(
        !cdda_sim::conditions::holds_with(&world, speaker, None, &read),
        "and it refuses where it is answered rather than answering a guess"
    );
}

/// A condition written as a bare name is an argument-less condition, and three of them can be answered.
///
/// Reference: `read_condition` (`condition.cpp:515-540`) hands a string to `conditional_t( std::string_view type )`
/// (`condition.cpp:2783-2810`), which looks the name up among the **simple** parsers. The data's ten top-level guards of this shape
/// name `has_beta`, `u_is_character`, `u_is_underwater`, `u_can_see`, `player_see_npc` and `u_is_avatar`; the port answers the three
/// whose answers it already has and refuses the rest, so the census stays honest.
#[test]
fn a_bare_name_is_an_argument_less_condition() {
    use cdda_content::dialogue::read_condition;

    let mut world = World::new();
    let player = cdda_sim::creature::spawn_creature(&mut world, "the player", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (0, 0, 0));

    let avatar = read_condition(&cdda_json::parse(br#""u_is_avatar""#.as_slice()).expect("json"))
        .expect("readable");
    let beta = read_condition(&cdda_json::parse(br#""has_beta""#.as_slice()).expect("json"))
        .expect("readable");

    assert!(
        !cdda_sim::conditions::holds(&world, player, &avatar),
        "nobody has claimed to be the avatar yet"
    );
    cdda_sim::missions::set_avatar(&mut world, player);
    assert!(cdda_sim::conditions::holds(&world, player, &avatar));
    assert!(
        !cdda_sim::conditions::holds(&world, talker, &avatar),
        "and an NPC is not the avatar"
    );

    assert!(
        !cdda_sim::conditions::holds_with(&world, player, None, &beta),
        "with nobody listening there is no beta"
    );
    assert!(cdda_sim::conditions::holds_with(
        &world,
        player,
        Some(talker),
        &beta
    ));

    // A name the port has no rule for is refused, not answered false.
    // `player_see_npc` **is** answered now (the avatar's own sight), so the refused example is a name the port still has no rule for.
    for unknown in ["u_is_on_furniture", "u_knows_recipe"] {
        let value = cdda_json::parse(format!(r#""{unknown}""#).as_bytes()).expect("json");
        assert!(
            read_condition(&value).is_none(),
            "`{unknown}` is refused rather than quietly answered"
        );
    }
}

/// The two chance guards roll, and a guard asked through the store can draw.
///
/// Reference: `f_one_in_chance` (`condition.cpp:1760-1766`) → `one_in` (`src/rng.cpp:196-199`) = `chance <= 1 || rng( 0, chance - 1 ) == 0`;
/// `f_x_in_y_chance` (`condition.cpp:1801-1810`) → `x_in_y` (`src/rng.cpp:206-209`) = `rng_float( 0, 1 ) <= x / y`.
#[test]
fn a_chance_guard_rolls_and_a_chance_of_one_always_holds() {
    use cdda_content::dialogue::read_condition;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(11));

    let sure = read_condition(&cdda_json::parse(br#"{"one_in_chance": 1}"#.as_slice()).expect("json"))
        .expect("readable");
    for _ in 0..50 {
        assert!(
            cdda_sim::conditions::holds_in_store(&mut world, survivor, None, &sure),
            "a chance of one always holds"
        );
    }

    // A chance of a hundred holds sometimes and not always, over a hundred draws.
    let rare = read_condition(&cdda_json::parse(br#"{"one_in_chance": 100}"#.as_slice()).expect("json"))
        .expect("readable");
    let held = (0..500)
        .filter(|_| cdda_sim::conditions::holds_in_store(&mut world, survivor, None, &rare))
        .count();
    assert!(
        (1..500).contains(&held),
        "one in a hundred held {held} times in five hundred"
    );

    // `x_in_y` with a numerator it cannot reach never holds; with `x >= y` it always does.
    let never = read_condition(
        &cdda_json::parse(br#"{"x_in_y_chance": {"x": 0, "y": 5}}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    let always = read_condition(
        &cdda_json::parse(br#"{"x_in_y_chance": {"x": 5, "y": 5}}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    for _ in 0..50 {
        assert!(!cdda_sim::conditions::holds_in_store(&mut world, survivor, None, &never));
        assert!(cdda_sim::conditions::holds_in_store(&mut world, survivor, None, &always));
    }

    // Asked **without** the store, the guards answer false rather than inventing a roll.
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &sure));

    // And inside a combinator the dice still reach the guard.
    let nested = read_condition(
        &cdda_json::parse(br#"{"and": [{"one_in_chance": 1}, "u_is_avatar"]}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    cdda_sim::missions::set_avatar(&mut world, survivor);
    assert!(cdda_sim::conditions::holds_in_store(
        &mut world,
        survivor,
        None,
        &nested
    ));
}

/// `u_has_item` asks whether any of an item is carried, by stack or by charge.
///
/// Reference: `f_has_item` (`condition.cpp:826-834`) answers `charges_of( id ) > 0 || has_amount( id, 1 )`, so a stack of a charge item
/// still counts however few charges are left - which is why the port asks both numbers rather than the stack count alone.
#[test]
fn carrying_one_item_answers_the_guard() {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 250,
            weight_g: 100,
            pockets: &[],
            thrown_damage: 0.0,
        }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let item = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(item).insert(cdda_sim::creature::Carried(survivor));

    assert!(holds(&world, survivor, &condition(r#"{"u_has_item": "rag"}"#)));
    assert!(!holds(&world, survivor, &condition(r#"{"u_has_item": "rope"}"#)));

    // The listener's own pockets are asked about when the guard names the listener.
    let other = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (0, 0, 0));
    let listener_guard = condition(r#"{"npc_has_item": "rag"}"#);
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, Some(other), &listener_guard),
        "the stranger carries nothing"
    );
    let theirs = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(theirs).insert(cdda_sim::creature::Carried(other));
    assert!(
        cdda_sim::conditions::holds_with(&world, survivor, Some(other), &listener_guard),
        "and now the listener's own pocket answers yes"
    );
}

/// The list forms of the trait and effect guards hold as soon as one name matches.
///
/// Reference: `f_has_any_trait` (`condition.cpp:563-578`) and `f_has_any_effect` (`condition.cpp:953`), both of which walk the list and
/// answer true on the first match rather than requiring all of them.
#[test]
fn holding_any_of_a_list_answers_a_list_guard() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::traits::set_traits(&mut world, survivor, &["QUICK"]);
    wearing(&mut world, survivor, "wet", 1);

    assert!(holds(&world, survivor, &condition(r#"{"u_has_any_trait": ["QUICK", "TOUGH"]}"#)));
    assert!(!holds(&world, survivor, &condition(r#"{"u_has_any_trait": ["TOUGH", "FAST"]}"#)));
    assert!(holds(&world, survivor, &condition(r#"{"u_has_any_effect": ["wet", "downed"]}"#)));
    assert!(!holds(&world, survivor, &condition(r#"{"u_has_any_effect": ["downed"]}"#)));

    // Asked about a listener that is not there, both answer false rather than reaching for the speaker.
    let npc_guard = condition(r#"{"npc_has_any_trait": ["QUICK"]}"#);
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &npc_guard));
    let stranger = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (0, 0, 0));
    assert!(!cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(stranger),
        &npc_guard
    ));
    cdda_sim::traits::add_trait(&mut world, stranger, "QUICK");
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(stranger),
        &npc_guard
    ));
}

/// `test_eoc` asks another program's own condition without running it.
///
/// Reference: `f_test_eoc` (`condition.cpp:1949-1961`) looks the program up and evaluates **its condition**; the program's effects never
/// run, so a `test_eoc` cannot change anything.
#[test]
fn asking_another_program_condition_runs_nothing() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let program = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"type": "effect_on_condition", "id": "EOC_TARGET",
                 "condition": {"math": ["armed", "==", 1]},
                 "effect": [{"u_message": "fired"}]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::eoc_registry::set_programs(&mut world, std::slice::from_ref(&program));

    let guard = condition(r#"{"test_eoc": "EOC_TARGET"}"#);
    assert!(!holds(&world, survivor, &guard), "the target's condition is false");
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "and asking ran nothing"
    );

    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "armed",
        cdda_sim::dialogue_values::Value::Number(1.0),
    );
    assert!(holds(&world, survivor, &guard), "and now it is true");
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "still without running the program"
    );

    // A name nothing defines answers false rather than panicking.
    assert!(!holds(&world, survivor, &condition(r#"{"test_eoc": "EOC_MISSING"}"#)));
}

/// The clock answers the season, and a guard can ask for one.
///
/// Reference: `season_of_year` (`calendar.cpp`) divides the day of the year by the season length; `f_is_season` (`condition.cpp:1250-1260`)
/// compares that against a name. The port had no clock at all - only a turn count inside systems - so this is the first question the
/// calendar can answer.
#[test]
fn the_calendar_turns_days_into_seasons() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // Turns count in seconds and a day is 86,400 of them, so every number here is exact in a double.
    #[allow(clippy::cast_precision_loss)]
    let day = cdda_sim::calendar::TURNS_PER_DAY;

    assert!(
        cdda_sim::calendar::season_of_year(&world).is_none(),
        "a world nothing has told the time has no season"
    );
    assert!(!holds(&world, survivor, &condition(r#"{"is_season": "spring"}"#)));

    cdda_sim::calendar::set_season_length(&mut world, 91);
    for (days_in, wanted) in [(0, "spring"), (90, "spring"), (91, "summer"), (182, "autumn"), (273, "winter")] {
        cdda_sim::calendar::set_turn(&mut world, days_in * day);
        assert_eq!(
            cdda_sim::calendar::season_of_year(&world).map(cdda_sim::calendar::Season::name),
            Some(wanted),
            "day {days_in} is in {wanted}"
        );
        assert!(
            holds(&world, survivor, &condition(&format!(r#"{{"is_season": "{wanted}"}}"#))),
            "and the guard agrees"
        );
    }

    // The four seasons repeat, so day 364 is spring again.
    cdda_sim::calendar::set_turn(&mut world, 364 * day);
    assert_eq!(
        cdda_sim::calendar::season_of_year(&world).map(cdda_sim::calendar::Season::name),
        Some("spring")
    );

    // A day is 86,400 turns: the reference multiplies by sixty at every step (`calendar.h:214-234`).
    assert_eq!(cdda_sim::calendar::TURNS_PER_DAY, 86_400);
}

/// `u_at_om_location` asks the overmap tile the speaker stands on.
///
/// Reference: `f_at_om_location` (`condition.cpp:1023-1046`) reads the actor's tile, strips direction and connections off its terrain id
/// (`oter_no_dir_or_connections`) and compares that with the string the data writes.
#[test]
fn standing_on_an_overmap_tile_answers_the_guard() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (30, 4, 0));

    // Nothing has generated a world, so no tile has a terrain and the guard answers false rather than inventing one.
    assert!(!holds(&world, survivor, &condition(r#"{"u_at_om_location": "field"}"#)));

    let tile = cdda_sim::overmap::tile_of(30, 4);
    assert_eq!(tile, (1, 0), "a tile is twenty-four squares across");
    cdda_sim::overmap::set_terrain(&mut world, tile, "field_north");
    assert!(
        holds(&world, survivor, &condition(r#"{"u_at_om_location": "field"}"#)),
        "a rotated tile answers its base terrain"
    );
    assert!(!holds(&world, survivor, &condition(r#"{"u_at_om_location": "house"}"#)));

    // A square in the next tile answers about that tile instead.
    world
        .entity_mut(survivor)
        .insert(cdda_sim::Position { x: 60, y: 4, z: 0 });
    assert!(!holds(&world, survivor, &condition(r#"{"u_at_om_location": "field"}"#)));
    cdda_sim::overmap::set_terrain(&mut world, cdda_sim::overmap::tile_of(60, 4), "house_east");
    assert!(holds(&world, survivor, &condition(r#"{"u_at_om_location": "house"}"#)));

    // The listener's own tile is asked about when the guard names the listener.
    let npc_guard = condition(r#"{"npc_at_om_location": "house"}"#);
    let stranger = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (0, 0, 0));
    assert!(!cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(stranger),
        &npc_guard
    ));
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        None,
        &condition(r#"{"u_at_om_location": "house"}"#)
    ));
}

/// Whether a condition the test writes holds for a creature.
fn asked_about_bionic(world: &World, creature: bevy_ecs::prelude::Entity, text: &str) -> bool {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value)
            .unwrap_or_else(|| panic!("{text} must read"));
        cdda_sim::conditions::holds(world, creature, &condition)
    }

/// `u_has_bionics` asks about the bionic a creature carries, and `ANY` asks about any of them.
///
/// Reference: `f_has_bionics` (`condition.cpp:940-951`): a named bionic holds when the creature has it installed, and `ANY` holds when the creature
/// has any bionic at all or a full power store - the power half is a divergence recorded in `docs/parity.md`.
#[test]
fn a_bionic_condition_asks_what_the_creature_carries() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(!asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "bio_power_storage"}"#), "nothing installed yet");
    assert!(!asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "ANY"}"#), "and no bionic at all");

    cdda_sim::bionics::install(&mut world, survivor, "bio_power_storage");
    assert!(
        asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "bio_power_storage"}"#),
        "the named bionic is installed"
    );
    assert!(
        !asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "bio_night_vision"}"#),
        "a bionic it does not have does not hold"
    );
    assert!(asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "ANY"}"#), "ANY holds with one installed");

    // The list keeps no duplicates, so installing twice changes nothing.
    cdda_sim::bionics::install(&mut world, survivor, "bio_power_storage");
    assert_eq!(
        cdda_sim::bionics::bionics_of(&world, survivor),
        vec!["bio_power_storage".to_owned()]
    );

    // A bionic named by a variable holds too.
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "wanted",
        cdda_sim::dialogue_values::Value::Text("bio_power_storage".to_owned()),
    );
    assert!(
        asked_about_bionic(&world, survivor, r#"{"u_has_bionics": "wanted"}"#),
        "a variable naming a bionic reads as that bionic"
    );
}

/// A creature asleep cannot see, and one awake can.
///
/// Reference: `talker_character_const::can_see` (`talker_character.cpp:917`) with `Creature::in_sleep_state` (`creature.cpp:2221`), which is any of
/// three effects. The data writes this as a bare string far more often than as an object, so both shapes are read.
#[test]
fn a_creature_asleep_cannot_see_and_one_awake_can() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let bare = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""u_can_see""#.as_slice()).expect("json"),
    )
    .expect("a bare name is readable");
    assert!(cdda_sim::conditions::holds(&world, survivor, &bare), "awake, so it sees");

    // Each of the three effects the reference counts as being asleep takes sight away, so each is exercised rather than a stand-in.
    for asleep in cdda_sim::seeing::ASLEEP {
        let add = cdda_content::dialogue::read_effect(
            &cdda_json::parse(format!(r#"{{"u_add_effect": "{asleep}", "duration": "PERMANENT"}}"#).as_bytes())
                .expect("json"),
        )
        .expect("readable");
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &add),
            cdda_sim::dialogue_effects::Applied::Done
        );
        assert!(
            !cdda_sim::conditions::holds(&world, survivor, &bare),
            "a creature with {asleep} cannot see"
        );
        let remove = cdda_content::dialogue::read_effect(
            &cdda_json::parse(format!(r#"{{"u_lose_effect": "{asleep}"}}"#).as_bytes()).expect("json"),
        )
        .expect("readable");
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &remove),
            cdda_sim::dialogue_effects::Applied::Done
        );
    }
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &bare),
        "awake again, so it sees again"
    );
}

/// The creature's own numbers, which the data asks for in 1,894 math calls and the port keeps on the creature already.
///
/// Reference: the `math_expressions` table (`dialogue_helpers.cpp`): `u_hp` answers remaining hit points, `u_effect_intensity` answers an effect's
/// strength and **zero** when it is not worn, and `has_var` tells "never set" apart from "set to zero".
#[test]
fn the_creatures_own_numbers_answer_in_math() {
    use cdda_content::dialogue::read_text_expression;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::set_health(&mut world, survivor, 42);

    let hp = read_text_expression("u_hp()").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &hp),
        Some(42.0),
        "the creature's hit points"
    );

    // Not worn at all is zero, not a refusal.
    let indifferent = read_text_expression("u_effect_intensity('drunk')").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &indifferent),
        Some(0.0)
    );

    let add = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_add_effect": "drunk", "duration": 100, "intensity": 3}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &add),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let worn = read_text_expression("u_effect_intensity('drunk')").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &worn),
        Some(3.0),
        "the intensity the effect was given"
    );

    // `has_var` separates unset from zero, which is the whole reason the data calls it.
    let unset = read_text_expression("has_var('scouted')").expect("readable");
    assert_eq!(cdda_sim::conditions::value(&world, survivor, None, &unset), Some(0.0));
    let assignment = cdda_content::dialogue::read_assignment("u_val('scouted') = 0").expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_assignment(&mut world, survivor, None, &assignment),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let set = read_text_expression("has_var('scouted')").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &set),
        Some(1.0),
        "set to zero is still set"
    );
}

/// The moon's phase is a number the data compares against, and it moves with the clock.
///
/// Reference: `moon_phase()` (`math_parser_diag.cpp:862`) answers `get_moon_phase( calendar::turn )` as the phase **index**; the phase flips at noon
/// and the lunar month is 29.530588853 days (`calendar.cpp:54-106`).
#[test]
fn the_moon_phase_is_the_index_the_data_compares_against() {
    use cdda_content::dialogue::read_text_expression;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let phase = read_text_expression("moon_phase()").expect("readable");

    // No clock set: the function refuses rather than answering a phase nobody set.
    assert_eq!(cdda_sim::conditions::value(&world, survivor, None, &phase), None);

    cdda_sim::calendar::set_turn(&mut world, 0);
    let at_zero = cdda_sim::conditions::value(&world, survivor, None, &phase).expect("answered");
    // A full lunar month later is the same phase again, which is what makes the number a phase rather than a day count.
    cdda_sim::calendar::set_turn(&mut world, 29 * 24 * 60 * 60);
    let a_month_later =
        cdda_sim::conditions::value(&world, survivor, None, &phase).expect("answered");
    assert!(
        (at_zero - a_month_later).abs() < f64::EPSILON,
        "the cycle closes after one lunar month: {at_zero} and {a_month_later}"
    );

    // A quarter of a month on is a different phase, so the function is not constant.
    cdda_sim::calendar::set_turn(&mut world, 7 * 24 * 60 * 60);
    let a_week_later = cdda_sim::conditions::value(&world, survivor, None, &phase).expect("answered");
    assert!(
        (at_zero - a_week_later).abs() > f64::EPSILON,
        "a week after the new moon the phase has moved: {at_zero} and {a_week_later}"
    );
    assert!(
        (0.0..=7.0).contains(&a_week_later),
        "the index is one of the eight phases, got {a_week_later}"
    );
}

/// A creature flag is a name, and a flag nobody gave the creature answers **no** rather than refusing.
///
/// Reference: `f_has_flag` (`condition.cpp:2529`) with `Creature::has_flag`: a plain survivor has no mutation flags, and 46 of the data's `u_has_flag`
/// guards ask exactly that - `GRAB_FILTER` on a survivor who cannot grab.
#[test]
fn a_creature_flag_answers_no_when_the_creature_does_not_carry_it() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (0, 1, 0));

    let grabs = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_has_flag": "GRAB"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &grabs),
        "a survivor carries no GRAB flag, and that is an answer"
    );

    // The flag family is on the creature, so giving it one changes the answer without touching anything else.
    cdda_sim::creature_flags::add_flag(&mut world, survivor, "GRAB");
    assert_eq!(cdda_sim::creature_flags::flags_of(&world, survivor), vec!["GRAB".to_owned()]);
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &grabs));

    // The listener's spelling asks about the other creature, which still carries none.
    let theirs = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"npc_has_flag": "GRAB"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &theirs),
        "the guard asks about the listener, not the speaker"
    );
    cdda_sim::creature_flags::add_flag(&mut world, talker, "GRAB");
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &theirs));

    // And taking it away puts the answer back, so the flag is what the guard reads.
    assert!(cdda_sim::creature_flags::remove_flag(&mut world, survivor, "GRAB"));
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &grabs));
}

/// A worn flag is a flag on a **worn item's type**, not on the creature.
///
/// Reference: `f_has_worn_with_flag` with `Character::worn_with_flag`: the reference walks the character's worn list and asks each item's type. The port
/// has no list on the wearer - an item is an entity and `WornBy` says who wears it - so the walk is over items.
#[test]
fn a_worn_flag_asks_the_items_type_and_not_the_creature() {
    use cdda_sim::ItemTypeId;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::item_registry::set_item_flags(
        &mut world,
        &[(ItemTypeId(1), &["BLIND"]), (ItemTypeId(2), &["HOOD"])],
    );

    let guard = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_has_worn_with_flag": "BLIND"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &guard),
        "wearing nothing is an answer, not a refusal"
    );

    // A blindfold with the flag: true. A hood without it: still false.
    let blindfold = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(1), 1);
    world.entity_mut(blindfold).insert(cdda_sim::creature::WornBy(survivor));
    assert!(cdda_sim::conditions::holds(&world, survivor, &guard));
    assert_eq!(
        cdda_sim::worn_flags::worn_flags_of(&world, survivor),
        vec!["BLIND".to_owned()]
    );

    let hood = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(2), 1);
    world.entity_mut(hood).insert(cdda_sim::creature::WornBy(survivor));
    let hooded = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_has_worn_with_flag": "HOOD"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(cdda_sim::conditions::holds(&world, survivor, &hooded));

    // The creature's own flags are a different question, and this one does not answer for them.
    assert!(
        !cdda_sim::creature_flags::has_flag(&world, survivor, "BLIND"),
        "wearing something with a flag does not give the creature the flag"
    );

    // An item lying on the ground is not worn, so it does not answer.
    let spare = cdda_sim::drop_item(&mut world, (3, 3), ItemTypeId(1), 1);
    world.entity_mut(spare).remove::<cdda_sim::creature::WornBy>();
    let elsewhere = cdda_sim::creature::spawn_creature(&mut world, "someone else", (5, 5, 0));
    assert!(
        !cdda_sim::conditions::holds(&world, elsewhere, &guard),
        "the blindfold is worn by the survivor, not by everyone"
    );
}

/// `time_since` counts from a named point on the clock, or from a turn the expression spells.
///
/// Reference: `time_since_eval` (`math_parser_diag.cpp:1173-1200`): `cataclysm`, `midnight` and `noon` are named counts, a number counts from that turn,
/// and a name that is not one of the three is not a time at all. `unit` is not read because the data never passes one.
#[test]
fn time_since_counts_from_a_named_point_or_a_turn() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // Ten turns past the second noon since the story began: two and a half days in, so the clock has run past two midnights and two noons.
    // Turns count in seconds and a day is 86,400 of them, so every number here is exact in a double.
    #[allow(clippy::cast_precision_loss)]
    let day = cdda_sim::calendar::TURNS_PER_DAY as f64;
    cdda_sim::calendar::set_start_of_cataclysm(&mut world, 0);
    #[allow(clippy::cast_possible_truncation)]
    let clock = (day + day / 2.0 + 10.0) as i64;
    cdda_sim::calendar::set_turn(&mut world, clock);
    assert_eq!(cdda_sim::calendar::of(&world).map(|calendar| calendar.turn), Some(clock));

    let count = |world: &mut World, text: &str| {
        cdda_sim::conditions::value(
            world,
            survivor,
            None,
            &cdda_content::dialogue::read_text_expression(text).expect("readable"),
        )
    };

    assert_eq!(
        count(&mut world, "time_since('cataclysm')"),
        Some(day + day / 2.0 + 10.0),
        "since the story began"
    );
    assert_eq!(
        count(&mut world, "time_since('midnight')"),
        Some(day / 2.0 + 10.0),
        "since the last midnight"
    );
    assert_eq!(
        count(&mut world, "time_since('noon')"),
        Some(10.0),
        "since the last noon, which is ten turns back"
    );

    // A turn the expression spells counts from there, and the answer moves with the clock.
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "then",
        cdda_sim::dialogue_values::Value::Number(day),
    );
    assert_eq!(count(&mut world, "time_since(u_val('then'))"), Some(day / 2.0 + 10.0));
    #[allow(clippy::cast_possible_truncation)]
    let two_days = (day * 2.0) as i64;
    cdda_sim::calendar::set_turn(&mut world, two_days);
    assert_eq!(
        count(&mut world, "time_since(u_val('then'))"),
        Some(day),
        "an hour later the same answer is a day older"
    );

    // Before noon the count is negative: the reference subtracts the noon it has not reached yet and keeps the sign, and the data relies on it.
    #[allow(clippy::cast_possible_truncation)]
    let just_after_midnight = (day + 100.0) as i64;
    cdda_sim::calendar::set_turn(&mut world, just_after_midnight);
    assert_eq!(count(&mut world, "time_since('noon')"), Some(100.0 - day / 2.0));

    // A name that is not a time is not answered.
    assert_eq!(count(&mut world, "time_since('teatime')"), None);
}

/// `u_near_om_location` asks whether a named overmap terrain is within a range, and answers about the tiles the overmap holds.
///
/// Reference: `f_near_om_location` (`condition.cpp:1074`): every overmap tile within `range` (default **1**) is compared against the name, and the guard
/// holds on the first match. The comparison is against the name **without** its direction or connection suffixes.
#[test]
fn a_nearby_overmap_terrain_guard_looks_within_its_range() {
    let mut world = World::new();
    // The survivor in overmap tile (0, 0): a tile two steps away is out of the default range and in at three.
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));
    cdda_sim::overmap::set_terrain(&mut world, (2, 0), "shelter");

    let nearby = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_near_om_location": "shelter"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &nearby),
        "two tiles away is outside the default range of one"
    );

    let wider = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_near_om_location": "shelter", "range": 3}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &wider),
        "three tiles is inside a range of three"
    );

    // A different name is a different question, and a name that is not there answers no rather than refusing.
    let elsewhere = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_near_om_location": "lab_finale", "range": 3}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(!cdda_sim::conditions::holds(&world, survivor, &elsewhere));

    // The name is compared without its direction: a tile the overmap spells `road_north` answers for `road`.
    cdda_sim::overmap::set_terrain(&mut world, (1, 1), "road_north");
    let road = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_near_om_location": "road"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(cdda_sim::conditions::holds(&world, survivor, &road));

    // The listener's spelling asks about the other creature's tile.
    let theirs = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"npc_near_om_location": "shelter", "range": 9}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (1, 1, 0));
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &theirs));
    let far_away = cdda_sim::creature::spawn_creature(&mut world, "someone far", (600, 600, 0));
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, Some(far_away), &theirs),
        "a listener a hundred overmap tiles away has nothing nearby"
    );

    // A camp name is a question about camps, not about terrain, so it is refused rather than answered as terrain.
    for camp in ["FACTION_CAMP_ANY", "FACTION_CAMP_START"] {
        let text = format!(r#"{{"u_near_om_location": "{camp}"}}"#);
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(cdda_content::dialogue::read_condition(&value).is_none(), "{camp}");
    }
}

/// `skill('bashing')` answers a character's level in a skill, and an unpractised skill is level **0** rather than unknown.
///
/// Reference: `skill_eval` (`math_parser_diag.cpp:961-965`) with `Character::get_skill_level`: the character's map answers a default entry for a skill it has
/// never practised, which is why a survivor with no training reads 0 and not "cannot say".
#[test]
fn a_skill_question_answers_zero_for_an_unpractised_skill() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (0, 1, 0));
    let answer = |world: &World, text: &str, listener: Option<bevy_ecs::prelude::Entity>| {
        cdda_sim::conditions::value(
            world,
            survivor,
            listener,
            &cdda_content::dialogue::read_text_expression(text).expect("readable"),
        )
    };

    assert_eq!(
        cdda_sim::conditions::value(
            &world,
            survivor,
            None,
            &cdda_content::dialogue::read_text_expression("skill('bashing')").expect("readable"),
        ),
        Some(0.0),
        "an unpractised skill is level zero, which is an answer"
    );

    cdda_sim::skills_of::set_level(&mut world, survivor, "bashing", 4);
    assert_eq!(cdda_sim::skills_of::level_of(&world, survivor, "bashing"), 4);
    assert_eq!(
        cdda_sim::conditions::value(
            &world,
            survivor,
            None,
            &cdda_content::dialogue::read_text_expression("skill('bashing')").expect("readable"),
        ),
        Some(4.0)
    );

    // The other skill is still zero, so the answer is about the skill asked and not about the creature in general.
    assert_eq!(
        cdda_sim::conditions::value(
            &world,
            survivor,
            None,
            &cdda_content::dialogue::read_text_expression("skill('cooking')").expect("readable"),
        ),
        Some(0.0)
    );

    // The listener's spelling asks about the other creature, which has practised nothing.
    cdda_sim::skills_of::set_level(&mut world, talker, "bashing", 7);
    let about_talker = cdda_content::dialogue::read_text_expression("npc_skill('bashing')").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, Some(talker), &about_talker),
        Some(7.0),
        "npc_skill asks about the listener"
    );
    assert_eq!(
        answer(&world, "skill('bashing')", Some(talker)),
        Some(4.0),
        "and skill still asks about the speaker"
    );
}

/// `u_is_outside` asks whether the creature is under the sky, and a creature below ground never is.
///
/// Reference: `f_is_outside` (`condition.cpp:2650`) with `is_creature_outside` (`weather.cpp:76-82`): the actor's square is outside **and** the level is at or
/// above street level, so a cellar answers no however open the square above it is.
#[test]
fn an_outside_guard_asks_about_the_square_and_the_level() {
    use cdda_sim::singletons::set_prototypes;
    use cdda_sim::Prototypes;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -8..8 {
        for y in -8..8 {
            for z in 0..2 {
                level.set_terrain(x, y, z, 1);
            }
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (2, 2, 0));

    let outside = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"u_is_outside": true}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &outside),
        "a square with nothing over it is under the sky"
    );

    // Below ground is never outside, even though the square above is open.
    let below = cdda_sim::creature::spawn_creature(&mut world, "the digger", (2, 2, -1));
    assert!(!cdda_sim::conditions::holds(&world, below, &outside));

    // A square whose terrain carries the indoors flag is not outside either.
    let mut prototypes = Prototypes::default();
    prototypes.terrain.insert_indoors(1, true);
    set_prototypes(&mut world, prototypes);
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &outside),
        "the indoors flag on the terrain is the answer"
    );

    // The guard needs no argument: a member with a value is still the same question, and the listener's spelling asks about the other creature.
    let mut clear = Prototypes::default();
    clear.terrain.insert_indoors(1, false);
    set_prototypes(&mut world, clear);
    let theirs = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"npc_is_outside": true}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (3, 3, 0));
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(talker), &theirs));
    let buried = cdda_sim::creature::spawn_creature(&mut world, "the buried", (3, 3, -1));
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, Some(buried), &theirs),
        "the listener's level is the one that counts"
    );
}

/// A call may pass named arguments, and `time_since` answers in the unit one names.
///
/// Reference: `math_parser` reads a call's arguments positionally and then its named ones (`'unit': 'hour'`), and `time_since_eval`
/// (`math_parser_diag.cpp:1173-1200`) divides by the span the unit names. One turn is one second, so an hour is 3,600 turns.
#[test]
fn a_named_unit_scales_the_answer() {
    let mut world = World::new();
    cdda_sim::calendar::set_start_of_cataclysm(&mut world, 0);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let answer = |world: &mut World, text: &str| {
        let condition = cdda_content::dialogue::read_condition(
            &cdda_json::parse(
                format!(r#"{{"math": ["{text}"]}}"#).as_bytes(),
            )
            .expect("json"),
        )
        .expect("readable");
        cdda_sim::conditions::holds(world, survivor, &condition)
    };

    // Two days after the cataclysm: two days, forty-eight hours, and not zero in either.
    cdda_sim::calendar::set_turn(&mut world, 2 * cdda_sim::calendar::TURNS_PER_DAY);
    assert!(answer(&mut world, "time_since('cataclysm', 'unit': 'days') >= 2"));
    assert!(answer(&mut world, "time_since('cataclysm', 'unit': 'hours') >= 48"));
    assert!(!answer(&mut world, "time_since('cataclysm', 'unit': 'weeks') >= 1"));

    // The turn figure is what it always was when no unit is named, which is how every call without one still reads.
    assert!(answer(&mut world, &format!(
        "time_since('cataclysm') >= {}",
        2 * cdda_sim::calendar::TURNS_PER_DAY
    )));

    // A named argument the function does not know, and a unit the port has no span for, are refused rather than read as zero.
    assert!(cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"math": ["time_since('cataclysm', 'unit': 'fortnights') >= 1"]}"#.as_slice()).expect("json"),
    )
    .is_some());
    let unknown_unit = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#"{"math": ["time_since('cataclysm', 'unit': 'fortnights') >= 1"]}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &unknown_unit),
        "a span the port cannot name answers nothing, not a wrong number"
    );
}

/// The four opinion numbers answer from the creature's own opinion, and zero when there is nobody to ask.
///
/// Reference: `npc_value_eval`/`npc_anger_eval`/`npc_trust_eval`/`npc_fear_eval` (`math_parser_diag.cpp:1408-1440`): the name's scope prefix says whose
/// opinion it is, and an actor that does not exist answers **zero** rather than failing - which is what lets a program ask an opinion of a listener it may not have.
#[test]
fn an_opinion_number_comes_from_the_creature_and_is_zero_when_absent() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (1, 0, 0));

    let asks = |text: &str| {
        cdda_content::dialogue::read_condition(
            &cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json"),
        )
        .expect("readable")
    };

    // No opinion yet: every number is zero, which the guard reads as "not angry".
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(talker),
        &asks("n_npc_anger() < 5")
    ));

    cdda_sim::npc_opinion::set_opinion(
        &mut world,
        talker,
        cdda_sim::npc_opinion::Opinion {
            value: 3,
            anger: 7,
            trust: -2,
            fear: 1,
        },
    );

    // The NPC's numbers are the listener's; the `u_` form asks the speaker's, which has none.
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(talker),
        &asks("n_npc_value() >= 3")
    ));
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(talker),
        &asks("n_npc_anger() >= 5")
    ));
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(talker),
        &asks("n_npc_trust() < 0")
    ));
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(talker),
        &asks("u_npc_value() == 0")
    ));

    // A dialogue with nobody on the other side answers zero rather than refusing.
    assert!(cdda_sim::conditions::holds(&world, survivor, &asks("n_npc_value() == 0")));
}

/// `has_var` reads a bare name as the name being asked about, not as a value.
///
/// Reference: `has_var` takes the variable's **name**, and the data writes it bare as often as quoted:
/// `!has_var(u_dialogue_hub_rnd_u_project_ongoing)`. A bare name otherwise reads as a variable lookup, so without this the guard compares a value to a name.
#[test]
fn has_var_reads_the_name_the_data_writes() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let asks = |text: &str| {
        cdda_content::dialogue::read_condition(
            &cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json"),
        )
        .expect("readable")
    };

    // Nothing stored yet: both spellings answer no, and the negation answers yes.
    assert!(!cdda_sim::conditions::holds(&world, survivor, &asks("has_var(project_flag) == 1")));
    assert!(!cdda_sim::conditions::holds(
        &world,
        survivor,
        &asks("has_var('project_flag') == 1")
    ));
    assert!(cdda_sim::conditions::holds(&world, survivor, &asks("!has_var(project_flag)")));

    // Once it is set, both spellings answer yes and the negation answers no.
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "project_flag",
        cdda_sim::dialogue_values::Value::Number(1.0),
    );
    assert!(cdda_sim::conditions::holds(&world, survivor, &asks("has_var(project_flag) == 1")));
    assert!(cdda_sim::conditions::holds(
        &world,
        survivor,
        &asks("has_var('project_flag') == 1")
    ));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &asks("!has_var(project_flag)")));
}
