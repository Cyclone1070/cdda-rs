//! Per-level caches that rebuild only when something they depend on moved.
//!
//! Reference: `map_cache` (`src/map_cache.h`, `src/map_cache.cpp`). The map keeps
//! derived per-cell values - light, transparency, visibility, floor - and marks the
//! submap holding a changed tile dirty. The next read of a dirty submap rebuilds it;
//! a read of a clean one does not. Invariant I8 in `docs/09-testing-plan.md` is the
//! thing these tests check: a change is visible to the next read, and no read
//! rebuilds more than it has to.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::map_cache::MapCache;
use cdda_core_types::submap::SubmapId;

fn submap(index: usize) -> SubmapId {
    SubmapId::from_index(index)
}

#[test]
fn a_clean_cache_answers_without_rebuilding() {
    let mut cache = MapCache::new();
    let mut builds = 0;
    let first = cache.value(submap(0), |_| {
        builds += 1;
        7
    });
    let second = cache.value(submap(0), |_| {
        builds += 1;
        9
    });
    assert_eq!(first, 7);
    assert_eq!(
        second, 7,
        "a clean submap keeps the value it was built with"
    );
    assert_eq!(builds, 1, "the second read costs nothing");
}

#[test]
fn a_change_is_visible_to_the_next_read_and_costs_one_rebuild() {
    let mut cache = MapCache::new();
    let mut builds = 0;
    let _ = cache.value(submap(0), |_| {
        builds += 1;
        1
    });

    cache.invalidate(submap(0));
    let rebuilt = cache.value(submap(0), |_| {
        builds += 1;
        2
    });
    assert_eq!(rebuilt, 2);
    assert_eq!(builds, 2, "one invalidation, one rebuild");
    assert_eq!(cache.rebuilds(), 2);
}

#[test]
fn invalidating_the_same_submap_twice_still_costs_one_rebuild() {
    // Twenty tiles changing in one submap is twenty marks and one rebuild. A cache
    // that rebuilt per mark would be correct and useless.
    let mut cache = MapCache::new();
    let mut builds = 0;
    let _ = cache.value(submap(3), |_| {
        builds += 1;
        0
    });
    for _ in 0..20 {
        cache.invalidate(submap(3));
    }
    let _ = cache.value(submap(3), |_| {
        builds += 1;
        5
    });
    assert_eq!(builds, 2);
}

#[test]
fn invalidating_one_submap_leaves_its_neighbours_alone() {
    let mut cache = MapCache::new();
    let mut builds = 0;
    for index in [0, 1, 2] {
        let _ = cache.value(submap(index), |_| {
            builds += 1;
            u8::try_from(index).expect("small index")
        });
    }
    assert_eq!(builds, 3);

    cache.invalidate(submap(1));
    for index in [0, 1, 2] {
        let _ = cache.value(submap(index), |_| {
            builds += 1;
            0
        });
    }
    assert_eq!(builds, 4, "only the invalidated submap was rebuilt");
}

#[test]
fn invalidating_everything_rebuilds_everything() {
    let mut cache = MapCache::new();
    let mut builds = 0;
    for index in 0..4 {
        let _ = cache.value(submap(index), |_| {
            builds += 1;
            0
        });
    }
    cache.invalidate_all();
    for index in 0..4 {
        let _ = cache.value(submap(index), |_| {
            builds += 1;
            0
        });
    }
    assert_eq!(builds, 8);
}
