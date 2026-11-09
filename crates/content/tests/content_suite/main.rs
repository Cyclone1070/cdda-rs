//! Every content test, in one binary.
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

mod ammo_effect_registry;
mod ammo_effects;
mod ammo_rules;
mod books;
mod bash_info;
mod build_check;
mod city_settings;
mod comestibles;
mod constructions;
mod crafting;
mod crafting_results;
mod consume_drugs;
mod corpus;
mod deployables;
mod digest;
mod digest_ids;
mod effect_mods;
mod effects;
mod eocs;
mod examine_actions;
mod factory;
mod faults;
mod field_spawn;
mod field_spawn_end_to_end;
mod field_spawn_on_level;
mod field_spawn_place;
mod field_turn_by_intensity;
mod field_turn_data;
mod fields;
mod firestarters;
mod forest_biomes;
mod forest_chunk_alignment;
mod forest_chunk_diff;
mod forest_chunk_pair;
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
mod holsters;
mod inheritance_corpus;
mod inline_enchantments;
mod item_catalog;
mod item_damage;
mod item_faults;
mod item_flags;
mod item_group_draw;
mod item_groups;
mod item_length;
mod item_nutrition;
mod item_pockets;
mod item_melee_parity;
mod items;
mod link_ups;
mod map_oracle;
mod martial_arts;
mod mapdata;
mod mapgen;
mod mapgen_draw;
mod mapgen_keys;
mod dialogue_members;
mod mapgen_objects;
mod snippets;
mod mapgen_place;
mod material_vitamins;
mod materials;
mod melee_accuracy;
mod math_expr;
mod mod_graph;
mod mods;
mod monster_group_pick;
mod monster_groups;
mod mutation_categories;
mod mutations;
mod monster_harvest;
mod anatomies;
mod armour;
mod body_parts;
mod material_resist;
mod monsters;
mod monster_attack;
mod monster_armour;
mod npc_classes;
mod overmap;
mod overmap_build;
mod overmap_locations;
mod overmap_specials;
mod overmap_terrain_ids;
mod overmap_world_diff;
mod passable_through;
mod pile_rules;
mod prototype_ids;
mod refusals;
mod qualities;
mod recipe_requirements;
mod recipe_skills;
mod recipe_tools;
mod proficiencies;
mod recipes;
mod spells;
mod resolve_placeholders;
mod rules;
mod starting_kit;
mod terrain;
mod transforms;
mod traps;
mod turret_parts;
mod use_action_shapes;
mod use_methods;
mod vehicles;
mod weather;
mod weather_conditions;
mod weather_manager;
mod weather_order;
mod zones;
mod zones_probe;
mod thrown_damage;

mod monster_mending;
mod no_fist;

mod gun_rules;

mod shot_rules;
mod weakpoints;
mod harvest_drop_types;
mod mutation_activation;
