//! Every core-types test, in one binary.
//!
//! Cargo gives each file in `tests/` its own binary, and every binary pays the same process startup
//! before a single test runs - measured at about 1.7 seconds each on this machine, which is the
//! dominant cost of the whole suite once there are a hundred and fifty of them. The files are
//! unchanged; this one lists them so the suite runs as a handful of binaries instead of one per file.
//!
//! A case is still named after its file: `frame_diff::the_same_frame_diffs_to_nothing`.

mod astronomy;
mod calendar;
mod colors;
mod containers;
mod coordinate_matrix;
mod daylight;
mod diagnostics;
mod field;
mod fire_ground;
mod fire_hot_air;
mod fire_rules;
mod fire_smoke;
mod fire_spread;
mod forest_border;
mod forest_corner_arithmetic;
mod forest_corners;
mod forest_feather;
mod forest_perimeter;
mod four_quadrants;
mod gas_spread;
mod ids;
mod level;
mod light;
mod light_source;
mod line;
mod map_cache;
mod map_reference_chunk;
mod memory;
mod noise_octaves;
mod noise_reference;
mod options;
mod outside_light;
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
mod recording;
mod rng;
mod rng_wrappers;
mod scent;
mod scent_turn;
mod scent_update;
mod seen;
mod shadowcasting;
mod sight;
mod sound;
mod sound_clusters;
mod sound_queue;
mod sound_vertical;
mod sound_weather;
mod strings;
mod submap_store;
mod sunlight;
mod text;
mod tiles;
mod units;
mod units_reader;
mod vision;
mod weather;
mod weighted;
