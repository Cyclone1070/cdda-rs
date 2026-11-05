//! Giving an item a fault, and what that takes away.
//!
//! Reference: `item::set_fault` (`src/item.cpp:7828-7845`), `item::remove_fault` (`7869-7872`) and `item::remove_single_fault_of_type` (`7874-7882`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_sim::{ItemFaults, remove_fault, set_fault};
use std::collections::BTreeSet;

/// The content's block lists: a broken handle prevents chipping and cracking; a cracked one prevents chipping.
fn blocks(id: &str) -> BTreeSet<String> {
    match id {
        "fault_handle_broken" => ["fault_handle_chipping", "fault_handle_cracked"]
            .iter()
            .map(|blocked| (*blocked).to_owned())
            .collect(),
        "fault_handle_cracked" => ["fault_handle_chipping"]
            .iter()
            .map(|blocked| (*blocked).to_owned())
            .collect(),
        _ => BTreeSet::new(),
    }
}

#[test]
fn a_fault_takes_away_the_faults_it_prevents() {
    // **The removal comes first**, before the new fault is inserted: `set_fault` walks the new fault's `block_faults` and removes each of them
    // (`src/item.cpp:7834-7839`). A staff that chipped and then broke is broken and not chipped, and the item lends itself to exactly one description.
    let mut faults = ItemFaults::default();
    assert!(set_fault(&mut faults, "fault_handle_chipping", false, 100, blocks));
    assert_eq!(faults.0.len(), 1);

    assert!(set_fault(&mut faults, "fault_handle_cracked", false, 50, blocks));
    assert_eq!(
        faults.0,
        ["fault_handle_cracked".to_owned()].into_iter().collect(),
        "cracking takes chipping away rather than joining it"
    );

    assert!(set_fault(&mut faults, "fault_handle_broken", false, 5, blocks));
    assert_eq!(
        faults.0,
        ["fault_handle_broken".to_owned()].into_iter().collect(),
        "and breaking takes both of the others away"
    );

    // **A refused fault changes nothing at all.** The check runs first, so even the removal of blocked faults does not happen.
    assert!(!set_fault(&mut faults, "fault_handle_chipping", false, 100, blocks), "a broken handle cannot chip");
    assert_eq!(
        faults.0,
        ["fault_handle_broken".to_owned()].into_iter().collect(),
        "the item is untouched by a refusal"
    );

    // **Unless it is forced**, which skips the check - and then the fault arrives even though the item's other fault blocks it.
    assert!(set_fault(&mut faults, "fault_handle_chipping", true, 100, blocks));
    assert_eq!(
        faults.0,
        ["fault_handle_broken".to_owned(), "fault_handle_chipping".to_owned()]
            .into_iter()
            .collect(),
        "a forced fault ignores the block, and does not remove the blocker either"
    );
    remove_fault(&mut faults, "fault_handle_chipping");
    assert_eq!(faults.0.len(), 1, "and a fault can be taken back off");
}
