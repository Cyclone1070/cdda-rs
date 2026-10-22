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
mod digest_ids;
mod eocs;
mod factory;
mod fields;
mod forest_biomes;
mod forest_chunk_diff;
mod forest_chunk_pair;

mod field_spawn;
mod field_spawn_end_to_end;
mod field_spawn_on_level;
mod field_spawn_place;
mod field_turn_by_intensity;
mod field_turn_data;
mod forest_chunk_alignment;
mod forest_edge_blend;
mod forest_ground_trace;
mod forest_items;
mod forest_layers;
mod forest_mapgen;
mod forest_neighbours;
mod forest_palette_weights;
mod forest_placeholders_placed;
mod forest_pool_arithmetic;
mod forest_pool_real_dice;
mod forest_pool_real_pipeline;
mod forest_pool_real_tickets;
mod forest_pool_trace;
mod forest_recorded_inputs;
mod forest_region_masks;
mod forest_stage_parity;
mod forest_stage_port_run;
mod forest_stage_triple;
mod forest_trace;
mod frame_diff;
mod frame_dump;
mod generated;
mod groundcover;
mod harvest_defs;
mod inheritance_corpus;
mod inline_enchantments;
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
mod monster_group_pick;
mod monster_groups;
mod monster_harvest;
mod monsters;
mod npc_classes;
mod overmap;
mod overmap_build;
mod overmap_locations;
mod overmap_specials;
mod overmap_world_diff;
mod passable_through;
mod prototype_ids;
mod recipe_requirements;
mod recipe_skills;
mod recipe_tools;
mod recipes;
mod resolve_placeholders;
mod rules;
mod starting_kit;
mod terrain;
mod traps;
mod turret_parts;
mod vehicles;
