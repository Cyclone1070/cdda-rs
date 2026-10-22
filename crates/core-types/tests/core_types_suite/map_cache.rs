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

#[test]
fn a_level_cache_starts_with_nothing_marked() {
    let cache = cdda_core_types::map_cache::LevelCache::new();
    assert!(!cache.transparency_dirty_anywhere(), "no submap is stale");
    assert!(!cache.seen_dirty());
    assert!(!cache.floor_dirty());
    assert!(!cache.outside_dirty());
}

#[test]
fn a_square_marks_its_own_submap_and_no_other() {
    // Reference: `map::set_transparency_cache_dirty( p, field )` (`map.cpp:302-311`). The bit is `sm.x * MAPSIZE + sm.y` - x outer again - so submap (2,1) is bit 23
    // and not bit 13.
    let mut cache = cdda_core_types::map_cache::LevelCache::new();
    // Submap (2,1) starts at map square (24,12).
    cache.mark_transparency_dirty_at(24, 12, false);
    assert!(cache.transparency_dirty_anywhere());
    assert!(cache.transparency_dirty_at(2, 1), "its own submap");
    assert!(!cache.transparency_dirty_at(1, 2), "and not the transpose");
    assert!(!cache.transparency_dirty_at(0, 0));
}

#[test]
fn a_change_that_is_not_a_field_also_invalidates_reachability() {
    // The same function has a second effect: `if( !field ) get_creature_tracker().invalidate_reachability_cache();` (`map.cpp:307-309`). Smoke moving is not a change
    // a creature's path can care about; a wall appearing is.
    let mut field_change = cdda_core_types::map_cache::LevelCache::new();
    field_change.mark_transparency_dirty_at(0, 0, true);
    assert!(
        !field_change.reachability_dirty(),
        "smoke does not block a path"
    );

    let mut terrain_change = cdda_core_types::map_cache::LevelCache::new();
    terrain_change.mark_transparency_dirty_at(0, 0, false);
    assert!(terrain_change.reachability_dirty(), "a wall does");
}

#[test]
fn marking_a_whole_level_marks_every_submap_and_keeps_the_narrow_marks() {
    // `map::invalidate_map_cache` (`map.cpp:397-406`) sets all four: the transparency bitset wholesale, floor, seen and outside.
    let mut cache = cdda_core_types::map_cache::LevelCache::new();
    cache.invalidate_all();
    assert!(cache.transparency_dirty_anywhere());
    assert!(cache.transparency_dirty_at(0, 0));
    assert!(cache.transparency_dirty_at(10, 10), "the far corner too");
    assert!(cache.seen_dirty());
    assert!(cache.floor_dirty());
    assert!(cache.outside_dirty());
}

#[test]
fn a_square_nobody_could_see_does_not_mark_the_seen_cache() {
    // Reference: `map::set_seen_cache_dirty( p )` (`map.cpp:313-325`). Two early-outs: already dirty is nothing to do, and a square whose seen **and** camera values are
    // both zero cannot have been shadowing anything, so a change there cannot change what is seen.
    let mut cache = cdda_core_types::map_cache::LevelCache::new();
    cache.set_seen_value(4, 4, 0.0);
    cache.mark_seen_dirty_at(4, 4);
    assert!(!cache.seen_dirty(), "nothing was visible there to lose");

    cache.set_seen_value(5, 5, 0.5);
    cache.mark_seen_dirty_at(5, 5);
    assert!(cache.seen_dirty(), "this square was visible");

    // Already dirty: the second call is cheap and changes nothing.
    cache.mark_seen_dirty_at(5, 5);
    assert!(cache.seen_dirty());
}
