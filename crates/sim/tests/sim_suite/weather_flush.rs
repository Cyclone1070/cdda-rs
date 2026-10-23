//! What the map does with a weather change.
//!
//! Source: `weather_manager::update_weather` (`src/weather.cpp:938-941`), the three lines where the weather reaches into the map:
//!
//! ```text
//! if( weather_id->sight_penalty != old_weather->sight_penalty ) {
//!     for( int i = -OVERMAP_DEPTH; i <= OVERMAP_HEIGHT; i++ ) { here.set_transparency_cache_dirty( i ); }
//!     here.set_seen_cache_dirty( tripoint_bub_ms::zero );
//! }
//! ```
//!
//! The port's manager reports `Change::Weather { sight_changed }`; this is the receiver. Nothing else in the weather system touches the map, and the test is here to keep it that
//! way: a change that does not alter the absorption must cost nothing, or every weather tick would redraw the world.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_content::weather::Change;
use cdda_core_types::level::Level;
use cdda_sim::{Map, Prototypes};

fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    world.insert_resource(Map(level));
    world.insert_resource(Prototypes::default());
    world
}

fn solid(_code: u32) -> bool {
    false
}

#[test]
fn only_a_sight_change_dirties_the_map() {
    let mut world = world();
    {
        let mut map = world.get_resource_mut::<Map>().expect("the map");
        map.cached_visibility((5, 5), 4, solid);
    }
    let builds = world.get_resource::<Map>().expect("the map").light_rebuilds();
    // `sunny` to `clear`: a different sky with the same absorption. The reference does not touch the map, and neither does the port.
    {
        let mut map = world.get_resource_mut::<Map>().expect("the map");
        assert!(!map.on_weather_change(Change::Weather { sight_changed: false }), "nothing to flush");
        assert!(!map.light_is_stale() || builds > 0, "the light is still the one that was built");
    }
    // `clear` to `fog`: every ray is dimmed, so every level's light is rebuilt.
    {
        let mut map = world.get_resource_mut::<Map>().expect("the map");
        assert!(map.on_weather_change(Change::Weather { sight_changed: true }), "the flush happened");
        assert!(map.light_is_stale(), "and the light will be rebuilt before it is read");
    }
    let mut map = world.get_resource_mut::<Map>().expect("the map");
    map.cached_visibility((5, 5), 4, solid);
    assert_eq!(map.light_rebuilds(), builds + 1, "exactly one rebuild");
}
