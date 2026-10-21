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
mod four_quadrants;
mod ids;
mod level;
mod light;
mod light_source;
mod line;
mod map_cache;
mod memory;
mod options;
mod outside_light;
mod overmap;
mod overmap_cities;
mod overmap_city_street;
mod overmap_placement;
mod overmap_street_layout;
mod overmap_streets;
mod paths;
mod projection;
mod rng;
mod rng_wrappers;
mod scent;
mod seen;
mod shadowcasting;
mod sight;
mod sound;
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
