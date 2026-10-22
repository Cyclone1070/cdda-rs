//! Every content test, in one binary.
//!
//! Cargo gives each file in `tests/` its own binary, and every binary pays the same process startup
//! before a single test runs - measured at about 1.7 seconds each on this machine, which is the
//! dominant cost of the whole suite once there are a hundred and fifty of them. The files are
//! unchanged; this one lists them so the suite runs as a handful of binaries instead of one per file.
//!
//! A case is still named after its file: `frame_diff::the_same_frame_diffs_to_nothing`.

mod city_settings;
mod corpus;
mod digest;
mod eocs;
mod factory;
mod fields;
mod frame_diff;
mod frame_dump;
mod generated;
mod harvest_defs;
mod inheritance_corpus;
mod item_catalog;
mod item_flags;
mod item_group_draw;
mod item_groups;
mod item_nutrition;
mod items;
mod mapdata;
mod mapgen;
mod mapgen_objects;
mod mapgen_place;
mod materials;
mod mods;
mod monster_harvest;
mod monsters;
mod overmap;
mod overmap_locations;
mod overmap_specials;
mod prototype_ids;
mod recipe_requirements;
mod recipe_skills;
mod recipe_tools;
mod recipes;
mod rules;
mod starting_kit;
mod terrain;
mod traps;
mod vehicles;
