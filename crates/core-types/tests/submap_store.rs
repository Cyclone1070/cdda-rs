//! Owning submaps by id, in one place, so nothing aliases and nothing leaks.
//!
//! Reference: the map buffer (`src/mapbuffer.h`, `mapbuffer.cpp`) owns every submap
//! and the map holds pointers into it. Pointers are the part the port does not
//! take: a submap is addressed by an id that outlives the store's own growth, which
//! is the decision `docs/03-decisions.md` records. These tests pin the two
//! properties that decision is for - an id stays valid, and a missing submap is
//! absent rather than conjured.

// A test's job is to assert. Where a store says "no such submap" the test fails
// loudly instead of building a story around the absence.
#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::submap::{SUBMAP_SIDE, SubmapStore};

#[test]
fn an_id_survives_the_store_growing_past_it() {
    let mut store = SubmapStore::new();
    let first = store.add(1);
    for terrain in 2..200 {
        store.add(terrain);
    }
    // The store has moved its storage around by now; the id still names the first.
    assert_eq!(store.len(), 199);
    assert_eq!(
        store
            .get(first)
            .map(cdda_core_types::submap::Submap::uniform_terrain),
        Some(1)
    );
}

#[test]
fn a_submap_that_was_never_made_is_absent() {
    // The reference's lookup returns a null pointer for an ungenerated submap, and
    // the port returns nothing at all: the caller has to decide what a blank world
    // means instead of receiving a plausible-looking default.
    let mut store = SubmapStore::new();
    let made = store.add(5);
    assert!(store.get(made).is_some());
    assert!(
        store
            .get(cdda_core_types::submap::SubmapId::from_index(1))
            .is_none()
    );
}

#[test]
fn a_submap_is_twelve_cells_across_and_holds_its_tiles() {
    let mut store = SubmapStore::new();
    let id = store.add(0);
    assert_eq!(SUBMAP_SIDE, 12);
    let submap = store.get_mut(id).expect("the submap exists");
    submap.set_terrain(3, 4, 9);
    assert_eq!(store.get(id).expect("still there").terrain(3, 4), 9);
    // Everything else keeps the value the submap was made with.
    assert_eq!(store.get(id).expect("still there").terrain(3, 5), 0);
}
