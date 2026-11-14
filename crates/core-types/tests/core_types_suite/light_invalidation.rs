//! When the light has to be rebuilt: a square changed, or the sky did.
//!
//! Source: `map::set_transparency_cache_dirty` (`src/map.cpp:295-300`) and `map::set_seen_cache_dirty` (`:313-325`) for the per-square case, and
//! `weather_manager::update_weather` (`src/weather.cpp:938-941`) for the sky - which dirties **every** level at once, because fog dims the whole map and not one square of it.
//!
//! The light is cached per origin and radius (the port's stand-in for the reference's per-level light map), so anything that changes what a ray passes through - a wall, a closed
//! door, or the weather's absorption - has to say so. A flush that never fired would show a foggy map at clear-weather brightness, which is the sort of bug that looks like a
//! rendering problem and is not.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_core_types::level::Level;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    level
}

fn solid(_code: u32) -> bool {
    false
}

#[test]
fn a_weather_change_dirties_the_light_so_it_is_rebuilt_once() {
    // The reference's flush marks the cache dirty; the rebuild itself happens the next time the map asks, not inside the flush. Both halves matter: dirtying on every square
    // would be slow, and rebuilding eagerly inside a weather update would cast light from inside a system that has nothing to cast it onto.
    let mut level = level();
    level.cached_visibility((5, 5), 4, solid);
    let builds = level.light_rebuilds();
    level.mark_weather_changed();
    assert!(level.light_is_stale(), "the flush says the light is stale");
    let cached = level.cached_visibility((5, 5), 4, solid);
    assert_eq!(cached.len(), 9, "a radius of four gives nine rows");
    assert_eq!(
        level.light_rebuilds(),
        builds + 1,
        "and the read rebuilt it exactly once"
    );
    level.cached_visibility((5, 5), 4, solid);
    assert_eq!(
        level.light_rebuilds(),
        builds + 1,
        "a second read of unchanged light rebuilds nothing"
    );
}

#[test]
fn a_flush_on_an_unbuilt_level_costs_nothing_and_still_reports_stale() {
    // At the start of a game the weather is set before any light is built. The flush still has to mark, because the first build must use the new weather's penalty - a flush that
    // skipped an empty cache would leave the first frame of a foggy morning as bright as noon.
    let mut level = level();
    assert!(
        level.light_is_stale(),
        "a level with no light must build one, whether or not anything dirtied it"
    );
    level.mark_weather_changed();
    assert!(level.light_is_stale());
    // The point is that the mark is **not lost**: a flush written as `if let Some( cache ) = self.light` would drop this one, and the first frame of a foggy morning would be drawn
    // at the brightness the level had before the weather existed.
    level.cached_visibility((5, 5), 2, solid);
    assert!(!level.light_is_stale(), "and building clears the mark");
    level.mark_weather_changed();
    assert!(
        level.light_is_stale(),
        "a flush after a build is remembered too"
    );
}

#[test]
fn a_terrain_change_still_dirties_the_light_without_a_weather_change() {
    // The two reasons to rebuild are independent. A wall built in fog still has to be seen, and a port that only listened to the weather would draw the map as it was before the wall.
    let mut level = level();
    level.cached_visibility((5, 5), 4, solid);
    let builds = level.light_rebuilds();
    level.set_terrain(5, 5, 0, 1);
    level.cached_visibility((5, 5), 4, solid);
    assert_eq!(
        level.light_rebuilds(),
        builds + 1,
        "the wall rebuilt the light"
    );
}
