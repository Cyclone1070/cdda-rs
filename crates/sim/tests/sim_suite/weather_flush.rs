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
use cdda_sim::Prototypes;

fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::set_prototypes(&mut world, Prototypes::default());
    world
}

#[test]
fn only_a_sight_change_dirties_the_light() {
    let mut world = world();
    cdda_sim::cast_lights_with(&mut world);
    assert!(!cdda_sim::light_is_stale(&world), "a fresh answer is not stale");
    let builds = cdda_sim::light_rebuilds(&world);
    assert_eq!(builds, 1, "the answer was built once");

    // `sunny` to `clear`: a different sky with the same absorption. The reference does not touch the
    // light, and neither does the port.
    assert!(
        !cdda_sim::apply_weather_change(&mut world, Change::Weather { sight_changed: false }),
        "nothing to flush"
    );
    assert!(!cdda_sim::light_is_stale(&world), "the answer is still trusted");

    // `clear` to `fog`: every ray is dimmed, so the light has to be rebuilt.
    assert!(
        cdda_sim::apply_weather_change(&mut world, Change::Weather { sight_changed: true }),
        "the flush happened"
    );
    assert!(
        cdda_sim::light_is_stale(&world),
        "and the light will be rebuilt before it is read"
    );

    cdda_sim::cast_lights_with(&mut world);
    assert_eq!(
        cdda_sim::light_rebuilds(&world),
        builds + 1,
        "exactly one rebuild"
    );
    assert!(!cdda_sim::light_is_stale(&world), "and the answer is fresh again");
}
