//! Every core-types test, in one binary.
//!
//! Cargo gives each file in `tests/` its own binary, and every binary pays the same process startup
//! before a single test runs - measured at about 1.7 seconds each on this machine, which is the
//! dominant cost of the whole suite once there are a hundred and fifty of them. The files are
//! unchanged; this one lists them so the suite runs as a handful of binaries instead of one per file.
//!
//! A case is still named after its file: `frame_diff::the_same_frame_diffs_to_nothing`.

/// A seed mixer for the tests that sample many dice.
///
/// `uniform_int` maps a small engine value to a small answer, and `minstd_rand0`'s first draw from a small seed is small too, so a test that counts `0, 1, 2, ...` and
/// reseeds samples one end of the distribution rather than the chance it means to test. (Measured, on `one_in( 100 )`: counting up fires 1,278 times in 2,000; spread
/// seeds fire the handful the chance promises.) The mixer is Knuth's multiplicative hash, so consecutive attempts land far apart in the engine's state.
#[must_use]
pub fn spread_seed(attempt: u32) -> u32 {
    attempt.wrapping_mul(2_654_435_761).wrapping_add(1)
}

mod apparent_light;
mod astronomy;
mod calendar;
mod rng_probe;
mod environment;
mod sun_internals;
mod colors;
mod containers;
mod coordinate_matrix;
mod diagnostics;
mod field;
mod fire_flashpoint;
mod fire_fuel_gate;
mod fire_ground;
mod fire_hot_air;
mod fire_rules;
mod fire_smoke;
mod fire_smoke_source;
mod fire_spread;
mod fire_spread_items;
mod fire_spread_order;
mod floor_cache;
mod forest_border;
mod forest_corner_arithmetic;
mod forest_corners;
mod forest_feather;
mod forest_perimeter;
mod four_quadrants;
mod gas_spread;
mod gas_wind;
mod grid;
mod ids;
mod item_burn;
mod item_burn_loop;
mod level;
mod level_lighting;
mod light;
mod light_buffer;
mod light_cast;
mod light_invalidation;
mod light_source;
mod light_source_rules;
mod line;
mod line_of_sight;
mod line_walk;
mod map_cache;
mod map_cache_build;
mod map_reference_chunk;
mod memory;
mod move_along;
mod noise_octaves;
mod noise_reference;
mod options;
mod outside_cache;
mod overmap;
mod overmap_building_kind;
mod overmap_buildings;
mod overmap_cities;
mod overmap_city_street;
mod overmap_forests;
mod overmap_placement;
mod overmap_placement_rule;
mod overmap_reference;
mod overmap_render;
mod overmap_road_shape;
mod overmap_street_layout;
mod overmap_streets;
mod paths;
mod projection;
mod quadrant_light;
mod recording;
mod recording_real;
mod rng;
mod rng_wrappers;
mod scent;
mod scent_turn;
mod scent_update;
mod seen;
mod shadowcasting;
mod shuffle;
mod sight;
mod sight_calc;
mod sound;
mod sound_clusters;
mod sound_queue;
mod sound_vertical;
mod sound_weather;
mod span_split;
mod squares_closer;
mod strings;
mod submap;
mod submap_store;
mod sunlight;
mod sunlight_cache;
mod text;
mod tiles;
mod transparency;
mod units;
mod units_reader;
mod valid_move;
mod visibility;
mod vision;
mod vision_threshold;
mod vision_transparency;
mod weather;
mod weighted;
mod zlight;
