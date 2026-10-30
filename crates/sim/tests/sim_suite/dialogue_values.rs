//! The variables a dialogue reads and writes.
//!
//! Reference: `dialogue::get_value` / `set_value` (`dialogue.cpp`), which answer by variable type - the player's, an NPC's,
//! a global one, the dialogue's context, or one addressed by name. The port had no store for them, which is why the 518
//! `math` and 238 `compare_string` conditions were refused: they name variables nothing could answer for.

#![allow(clippy::float_cmp, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::dialogue_values::{self, Value};

#[test]
fn a_value_is_remembered_under_its_name_and_absent_is_not_zero() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    assert_eq!(dialogue_values::number_of(&world, survivor, "mission_counter"), None);
    dialogue_values::set(&mut world, survivor, "mission_counter", Value::Number(3.0));
    assert_eq!(dialogue_values::number_of(&world, survivor, "mission_counter"), Some(3.0));
    assert_eq!(
        dialogue_values::text_of(&world, survivor, "mission_counter"),
        None,
        "a number is not text, and asking for the wrong kind gets nothing rather than a conversion"
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "never_set"),
        None,
        "an absent variable is absent, not zero: the reference's maybe_get_value answers a null pointer"
    );
}

#[test]
fn a_global_value_is_the_same_value_for_everyone_and_u_values_are_not() {
    let mut world = World::new();
    let one = cdda_sim::creature::spawn_creature(&mut world, "one", (0, 0, 0));
    let two = cdda_sim::creature::spawn_creature(&mut world, "two", (1, 0, 0));

    dialogue_values::set(&mut world, one, "my_own", Value::Number(1.0));
    assert_eq!(
        dialogue_values::number_of(&world, two, "my_own"),
        None,
        "a u_val belongs to the speaker"
    );

    let globe = dialogue_values::holder_for(&mut world, one, true);
    dialogue_values::set(&mut world, globe, "world_first_visit", Value::Number(1.0));
    let from_two = dialogue_values::holder_for(&mut world, two, true);
    assert_eq!(
        dialogue_values::number_of(&world, from_two, "world_first_visit"),
        Some(1.0),
        "a global_val is shared, which is what `global` means"
    );
    // The same name can mean different things in the two namespaces, and neither shadows the other.
    dialogue_values::set(&mut world, two, "world_first_visit", Value::Number(9.0));
    assert_eq!(
        dialogue_values::number_of(&world, two, "world_first_visit"),
        Some(9.0)
    );
    assert_eq!(
        dialogue_values::number_of(&world, from_two, "world_first_visit"),
        Some(1.0),
        "the global value did not move"
    );
}

#[test]
fn a_value_can_be_replaced_and_the_kind_can_change() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    dialogue_values::set(&mut world, survivor, "name_of_thing", Value::Text("rope".to_owned()));
    assert_eq!(
        dialogue_values::text_of(&world, survivor, "name_of_thing").as_deref(),
        Some("rope")
    );
    dialogue_values::set(&mut world, survivor, "name_of_thing", Value::Number(7.0));
    assert_eq!(dialogue_values::number_of(&world, survivor, "name_of_thing"), Some(7.0));
    assert_eq!(
        dialogue_values::text_of(&world, survivor, "name_of_thing"),
        None,
        "the last write decides the kind, as one map of one value per name does in the reference"
    );
    assert_eq!(dialogue_values::all(&world, survivor).len(), 1, "and one name holds one value");
}

/// An arithmetic condition works out its expression, and a variable it cannot read makes it unanswerable.
///
/// Reference: a `math` condition (`condition.cpp:194-196`) reads an array expression and holds when the result is non-zero.
/// The array is fully parenthesised - `[left, op, right]` - so its nesting carries the order of operations.
#[test]
fn a_math_condition_does_the_arithmetic_the_data_writes() {
    use cdda_sim::conditions::holds;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let three_plus_four = cdda_json::parse(br#"{"math": [[3, "+", 4], "==", 7]}"#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&three_plus_four).expect("readable");
    assert!(holds(&world, survivor, &condition), "three and four is seven");

    // Nesting decides the order: (3 + 4) * 2 is 14, and 3 + (4 * 2) is 11.
    let nested = cdda_json::parse(br#"{"math": [[[3, "+", 4], "*", 2], ">", 11]}"#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&nested).expect("readable");
    assert!(holds(&world, survivor, &condition), "(3 + 4) * 2 is 14, which is more than 11");

    // A variable the store holds.
    dialogue_values::set(&mut world, survivor, "mission_counter", Value::Number(2.0));
    let from_a_variable = cdda_json::parse(
        br#"{"math": ["u_val('mission_counter')", ">=", 2]}"#.as_slice(),
    )
    .expect("json");
    let condition = cdda_content::dialogue::read_condition(&from_a_variable).expect("readable");
    assert!(holds(&world, survivor, &condition), "the counter is two, which is at least two");

    // A variable nobody set makes the expression unanswerable, which is false rather than zero.
    let unset = cdda_json::parse(br#"{"math": ["u_val('never_set')", ">", 0]}"#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&unset).expect("readable");
    assert!(!holds(&world, survivor, &condition));

    // An operator nobody ported, and a variable scope nobody ported, are both refused at read time.
    let an_unknown_op = cdda_json::parse(br#"{"math": [1, "and", 1]}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_condition(&an_unknown_op).is_none());
    // A function call is not a variable: `ceil(3.5)` names a function the port has no rule for, and reading it as a
    // variable called `ceil` would answer a question nobody asked.
    let a_function = cdda_json::parse(br#"{"math": ["ceil(3.5)", "==", 4]}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_condition(&a_function).is_none());
    // The five variable types are all readable, bare identifiers included - the reference looks a bare name up in the
    // dialogue's context (`const_dialogue::maybe_get_value`, `npctalk.cpp:2659-2662`).
    let a_context_variable =
        cdda_json::parse(br#"{"math": ["context_val('x')", "==", 1]}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_condition(&a_context_variable).is_some());
    let bare = cdda_json::parse(br#"{"math": ["Nemesis_iteration * -1"]}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_condition(&bare).is_some());
}
