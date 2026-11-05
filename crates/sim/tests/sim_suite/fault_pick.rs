//! Which fault a damaging hit gives an item.
//!
//! Reference: `item::set_random_fault_of_type` (`src/item.cpp:7848-7865`), `item::can_have_fault` (`src/item.cpp:7809-7826`), and the weighted pick itself
//! (`src/weighted_list.h:120-129` and `247-267`), which draws `rng_bits()` — a raw 32-bit number — and not an `rng( lo, hi )` pair.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::rng::{Recording, Rolls};
use cdda_sim::{can_have_fault, pick_fault};
use std::collections::BTreeSet;

/// The fault table, in the order the type lists them, with the weights the reference resolved.
fn table() -> Vec<(String, i32)> {
    vec![
        ("fault_handle_chipping".to_owned(), 100),
        ("fault_handle_cracked".to_owned(), 50),
        ("fault_handle_broken".to_owned(), 5),
        ("fault_blade_chipped".to_owned(), 500),
    ]
}

/// Which kind each fault is: the handle ones are mechanical damage, the blade one is not.
///
/// Wrapped in an `Option` because the port asks the **content** for a fault's kind, and a fault the content does not know has none — so the test hands it one
/// through the same shape the real caller does rather than a string that cannot fail.
#[allow(clippy::unnecessary_wraps)]
fn kind(id: &str) -> Option<String> {
    let kind = match id {
        "fault_handle_chipping" | "fault_handle_cracked" | "fault_handle_broken" => {
            "mechanical_damage"
        }
        _ => "blade_damage",
    };
    Some(kind.to_owned())
}

/// The blocks the content states: a broken handle prevents chipping and cracking.
fn blocks(id: &str) -> BTreeSet<String> {
    if id == "fault_handle_broken" {
        ["fault_handle_chipping".to_owned(), "fault_handle_cracked".to_owned()]
            .into_iter()
            .collect()
    } else {
        BTreeSet::new()
    }
}

fn rolls_with_bits(value: u32) -> Rolls {
    Rolls::replaying(Recording::parse(&format!("bits {value}\n")).expect("one raw draw"))
}

#[test]
fn the_pick_reduces_a_raw_draw_modulo_the_total_weight() {
    // The three mechanical faults weigh 100 + 50 + 5 = 155 together, so the draw is reduced modulo 155 and then walked from the front:
    // 1..=100 chipping, 101..=150 cracked, 151..=155 broken.
    let empty = BTreeSet::new();
    let cases = [
        (0, "fault_handle_chipping"),
        (99, "fault_handle_chipping"),
        (100, "fault_handle_cracked"),
        (149, "fault_handle_cracked"),
        (150, "fault_handle_broken"),
        (154, "fault_handle_broken"),
        // And the modulo: 155 wraps back to the first fault, and so does a draw that is a whole multiple of it.
        (155, "fault_handle_chipping"),
        (310, "fault_handle_chipping"),
    ];
    for (bits, expected) in cases {
        let mut rolls = rolls_with_bits(bits);
        assert_eq!(
            pick_fault(&table(), &empty, "mechanical_damage", kind, blocks, &mut rolls),
            Some(expected.to_owned()),
            "a raw draw of {bits}"
        );
    }

    // **A blade fault is not on offer for mechanical damage**, even though it outweighs all three others together.
    let mut rolls = rolls_with_bits(0);
    assert_eq!(
        pick_fault(&table(), &empty, "blade_damage", kind, blocks, &mut rolls),
        Some("fault_blade_chipped".to_owned()),
        "the blade has its own kind"
    );
}

#[test]
fn a_blocked_or_already_present_fault_is_off_the_list_and_a_single_one_draws_nothing() {
    // An item that is already broken cannot be chipped or cracked again, and cannot be broken twice: only the blade remains.
    let broken: BTreeSet<String> = ["fault_handle_broken".to_owned()].into_iter().collect();
    let mut rolls = Rolls::replaying(Recording::parse("int 0 1 0\n").expect("one die"));
    assert_eq!(
        pick_fault(&table(), &broken, "mechanical_damage", kind, blocks, &mut rolls),
        None,
        "a broken handle blocks the other two and is already there, so nothing is left of that kind"
    );
    assert_eq!(
        rolls.rng(0, 1),
        0,
        "and no die was rolled for a list that came out empty: the only draw left is the one above"
    );

    // **A list of one draws nothing at all.** `weighted_int_list::pick_ent` returns zero when there is a single entry
    // (`src/weighted_list.h:249-251`), so the recording keeps its die.
    let chipped: BTreeSet<String> = ["fault_handle_chipping".to_owned(), "fault_handle_cracked".to_owned()]
        .into_iter()
        .collect();
    let mut rolls = Rolls::replaying(Recording::parse("").expect("an empty recording"));
    assert_eq!(
        pick_fault(
            &table(),
            &chipped,
            "mechanical_damage",
            kind,
            blocks,
            &mut rolls
        ),
        Some("fault_handle_broken".to_owned()),
        "only the break is left, and it is returned without a die"
    );

    // And `can_have_fault` says the same thing the filter does, one fault at a time.
    assert!(!can_have_fault(5, "fault_handle_chipping", &broken, blocks), "blocked");
    assert!(
        !can_have_fault(5, "fault_handle_broken", &broken, blocks),
        "already present"
    );
    assert!(can_have_fault(5, "fault_handle_broken", &BTreeSet::new(), blocks), "nothing stops it");
    assert!(!can_have_fault(0, "fault_blade_chipped", &BTreeSet::new(), blocks), "a zero weight is not on the list");
}
