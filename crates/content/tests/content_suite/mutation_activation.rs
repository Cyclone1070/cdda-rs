//! What switching a mutation on costs, and what it runs when it is switched on.
//!
//! Reference: `mutation.h:205-290` for the data, `Character::activate_mutation` (`mutation.cpp:813-880`) and `can_power_mutation` (`:411-419`) for the rule that reads it. The
//! fields came into the dump in round 1426: before that the port could not carry out `u_activate_trait` at all, because the data said nothing about what activation does.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::mutations;

fn dump() -> mutations::Mutations {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/mutations.json");
    let value = cdda_content::read_json(&path).expect("the mutation dump");
    mutations::from_dump(&value).expect("a dump").0
}

#[test]
fn a_mutation_that_runs_a_program_when_switched_on_says_so() {
    let data = dump();
    let biolum = data.get("BIOLUM3").expect("BIOLUM3");
    assert_eq!(biolum.cost, 400, "it costs four hundred");
    assert!(biolum.hunger, "and it is paid in hunger");
    assert_eq!(
        biolum.activated_eocs,
        vec!["EOC_BIOLUM3_activated".to_owned()],
        "**and it runs its own program when it is switched on**"
    );

    let wings = data.get("WINGS_INSECT").expect("WINGS_INSECT");
    assert_eq!(wings.cost, 0, "while a mutation that switches for free says nothing");
    assert!(wings.activated_eocs.is_empty(), "and runs nothing");

    let with_programs = data.ids().filter(|id| {
        data.get(id)
            .is_some_and(|mutation| !mutation.activated_eocs.is_empty())
    });
    let counted = with_programs.count();
    assert!(
        counted >= 6,
        "the dump really did carry them: {counted} mutations run a program when switched on"
    );
}
