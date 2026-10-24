//! One pile cannot serve two groups.
//!
//! `requirement_data::check_enough_materials` (`src/requirements.cpp:1061-1146`) is the rule that makes a build check honest: a kit can satisfy every group on its own and
//! still be one welding torch short, because the torch the recipe consumes is also the torch the recipe needs to hold. The reference checks two overlaps —
//! **component against tool** and **component against the quality that component itself provides** — and one size limit.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::requirements::{Kit, RequirementData};
use std::collections::BTreeMap;

/// A kit with one item in it.
fn kit_of(id: &str, count: i32, by_charge: bool) -> Kit {
    let mut kit = Kit::default();
    kit.add(id, count, by_charge, BTreeMap::new());
    kit
}

#[test]
fn a_component_that_is_also_a_tool_has_to_be_there_twice() {
    // The reference's own example: one welder wanted as a component and one held as a tool needs **two** welders, because the one you consume cannot be the one you hold
    // (`src/requirements.cpp:1103-1127`). A kit with a single welder satisfies each group on its own and must still be refused.
    let need = RequirementData {
        components: vec![vec![("welder".to_owned(), 1, false)]],
        tools: vec![vec![("welder".to_owned(), -1, false)]],
        ..RequirementData::default()
    };
    assert!(!need.can_make(&kit_of("welder", 1, false)), "one welder is not enough");
    assert!(
        need.can_make(&kit_of("welder", 2, false)),
        "two welders are: one to spend and one to hold"
    );
}

#[test]
fn a_component_cannot_be_the_thing_that_provides_the_quality_the_build_needs() {
    // A pipe is a hammer at level one. A build that consumes a pipe **and** needs a hammer therefore needs two pipes, not one: the one in the recipe is not on the bench
    // any more when the hammering starts (`src/requirements.cpp:1128-1138`).
    let mut qualities = BTreeMap::new();
    qualities.insert("HAMMER".to_owned(), 1);
    let need = RequirementData {
        components: vec![vec![("pipe".to_owned(), 1, false)]],
        qualities: vec![vec![("HAMMER".to_owned(), 1, 1)]],
        ..RequirementData::default()
    };

    let mut one = Kit::default();
    one.add("pipe", 1, false, qualities.clone());
    assert!(!need.can_make(&one), "the only hammer is in the recipe");

    let mut two = Kit::default();
    two.add("pipe", 2, false, qualities);
    assert!(need.can_make(&two), "one to cut up and one to hammer with");
}

#[test]
fn the_components_of_one_craft_may_not_fill_more_than_a_tile() {
    // The volume side of the same function (`src/requirements.cpp:1086-1090`): a craft's components are the craft, and a craft cannot be bigger than a square. One
    // thousand litres is the limit, and a hundred and one of a ten-litre item is over it.
    let need = RequirementData {
        components: vec![vec![("10l_steel".to_owned(), 100, false)]],
        ..RequirementData::default()
    };
    let mut fitting = Kit::default();
    fitting.add_measured("10l_steel", 100, false, BTreeMap::new(), 10_000, 1);
    assert!(need.can_make(&fitting), "a thousand litres just fits");

    let bulky = RequirementData {
        components: vec![vec![("10l_steel".to_owned(), 101, false)]],
        ..RequirementData::default()
    };
    let mut over = Kit::default();
    over.add_measured("10l_steel", 101, false, BTreeMap::new(), 10_000, 1);
    assert!(!bulky.can_make(&over), "a thousand and ten does not");
}

#[test]
fn a_stack_counted_by_charge_is_measured_in_whole_items() {
    // And a charge count is turned into items by the type's stack size before it is measured (`src/requirements.cpp:1072-1074`): two hundred nails of a hundred to a
    // stack take the room of two nails, not of two hundred.
    let need = RequirementData {
        components: vec![vec![("nail".to_owned(), 200, false)]],
        ..RequirementData::default()
    };
    let mut kit = Kit::default();
    kit.add_measured("nail", 200, true, BTreeMap::new(), 5_000, 100);
    assert!(
        need.can_make(&kit),
        "two items' worth of room is well inside a tile"
    );
    assert_eq!(
        kit.stacks.get("nail").map(|stack| stack.stack_size),
        Some(100),
        "the kit remembers how many make a stack"
    );
}
