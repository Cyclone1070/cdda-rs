# Appendix — file to phase map

Generated from the pinned baseline (`0.I-1`, commit `7b2efa5`). Every file in `src/`
outside vendored third-party code is assigned to exactly one roadmap phase, so the
phases in [05-roadmap.md](05-roadmap.md) cover the whole codebase with nothing dropped.
Line counts are as they exist at that commit.

| Phase | Files | LOC | Share | Scope |
|---|---|---|---|---|
| P0 | 189 | 62,866 | 11.1% | Foundation types, IDs, RNG, JSON, text, diagnostics |
| P1 | 131 | 61,911 | 11.0% | Content registry, mods, save format, oracle harness |
| P2 | 60 | 40,200 | 7.1% | World grid, environment, map generation |
| P3 | 86 | 91,446 | 16.2% | Items, inventory, crafting, interaction |
| P4 | 83 | 74,569 | 13.2% | Creatures, body, effects, combat maths |
| P5 | 10 | 9,444 | 1.7% | Spells, enchantments, relics |
| P6 | 22 | 26,317 | 4.7% | Activities and action pipeline |
| P7 | 57 | 38,090 | 6.7% | AI, pathfinding, dialogue and scripting |
| P8 | 21 | 25,752 | 4.6% | Vehicles |
| P9 | 43 | 47,547 | 8.4% | Overmap, world generation, factions, missions |
| P10 | 113 | 59,329 | 10.5% | Interface, rendering, audio |
| P11 | 25 | 27,612 | 4.9% | Runtime shell, platform, tooling, editors |
| **total** | **840** | **565,083** | **100%** | |

## P0 — Foundation types, IDs, RNG, JSON, text, diagnostics

189 files, 62,866 LOC.

| File | LOC |
|---|---|
| `debug_menu.cpp` | 4,558 |
| `options.cpp` | 4,206 |
| `colony.h` | 3,526 |
| `list.h` | 2,479 |
| `json.cpp` | 2,464 |
| `debug.cpp` | 1,964 |
| `math_parser_diag.cpp` | 1,767 |
| `json.h` | 1,625 |
| `generic_factory.h` | 1,601 |
| `units.h` | 1,328 |
| `zzip.cpp` | 1,211 |
| `coordinates.h` | 1,172 |
| `color.cpp` | 1,167 |
| `flexbuffer_json-inl.h` | 1,133 |
| `math_parser.cpp` | 1,074 |
| `line.cpp` | 1,035 |
| `calendar.cpp` | 943 |
| `cata_utility.cpp` | 940 |
| `cata_utility.h` | 773 |
| `flexbuffer_json.h` | 741 |
| `filesystem.cpp` | 714 |
| `calendar.h` | 680 |
| `catacharset.cpp` | 633 |
| `path_info.cpp` | 617 |
| `simplexnoise.cpp` | 579 |
| `color.h` | 547 |
| `cata_variant.h` | 542 |
| `cata_io.h` | 515 |
| `enums.h` | 515 |
| `point.h` | 514 |
| `options.h` | 508 |
| `flexbuffer_cache.cpp` | 507 |
| `flag.h` | 505 |
| `flag.cpp` | 469 |
| `translation.cpp` | 466 |
| `translation_plural_evaluator.cpp` | 466 |
| `string_formatter.h` | 445 |
| `units_utility.cpp` | 402 |
| `mmap_file.cpp` | 387 |
| `string_id.h` | 386 |
| `text_snippets.cpp` | 381 |
| `line.h` | 377 |
| `flexbuffer_json.cpp` | 372 |
| `math_parser_diag_value.cpp` | 368 |
| `math_parser_impl.h` | 363 |
| `weather_gen.cpp` | 363 |
| `zzip_stack.cpp` | 356 |
| `type_id.h` | 354 |
| `text.cpp` | 337 |
| `cata_bitset.h` | 336 |
| `weighted_list.h` | 315 |
| `debug.h` | 306 |
| `weighted_dbl_or_var_list.h` | 284 |
| `catacharset.h` | 250 |
| `units.cpp` | 250 |
| `translation_manager_impl.cpp` | 237 |
| `zzip.h` | 236 |
| `cata_small_literal_vector.h` | 229 |
| `crash.cpp` | 228 |
| `rng.cpp` | 228 |
| `translation.h` | 227 |
| `value_ptr.h` | 224 |
| `flat_set.h` | 221 |
| `translation_document.cpp` | 219 |
| `debug_menu.h` | 215 |
| `rng.h` | 211 |
| `text_style_check.h` | 208 |
| `distribution.cpp` | 205 |
| `cuboid_rectangle.h` | 197 |
| `point.cpp` | 193 |
| `system_locale.cpp` | 192 |
| `simplexnoise.h` | 189 |
| `math_parser_func.h` | 183 |
| `int_id.h` | 180 |
| `wcwidth.cpp` | 180 |
| `json_loader.cpp` | 173 |
| `cata_path.h` | 165 |
| `zzip_stack.h` | 158 |
| `text_snippets.h` | 155 |
| `units_utility.h` | 150 |
| `calendar_ui.cpp` | 148 |
| `mingw.thread.h` | 146 |
| `filesystem.h` | 145 |
| `enum_traits.h` | 143 |
| `math_parser_lex.cpp` | 143 |
| `string_formatter.cpp` | 137 |
| `coordinates.cpp` | 132 |
| `unicode.cpp` | 129 |
| `tgz_archiver.cpp` | 128 |
| `cata_algo.h` | 127 |
| `ret_val.h` | 127 |
| `translation_plural_evaluator.h` | 126 |
| `enum_bitset.h` | 124 |
| `mdarray.h` | 122 |
| `hash_utils.h` | 119 |
| `enum_conversions.h` | 117 |
| `translation_cache.h` | 115 |
| `translations.h` | 115 |
| `math_parser_diag_value.h` | 111 |
| `translations.cpp` | 108 |
| `path_info.h` | 107 |
| `coords_fwd.h` | 106 |
| `string_id_null_ids.cpp` | 102 |
| `try_parse_integer.cpp` | 97 |
| `cata_variant.cpp` | 95 |
| `lru_cache.h` | 88 |
| `math_parser_jmath.cpp` | 87 |
| `cata_views.h` | 86 |
| `units_fwd.h` | 85 |
| `color_loader.h` | 84 |
| `pinyin.cpp` | 84 |
| `perf.h` | 83 |
| `mmap_file.h` | 82 |
| `cata_lazy.h` | 80 |
| `pimpl.h` | 79 |
| `weather_gen.h` | 79 |
| `cached_options.h` | 76 |
| `flexbuffer_cache.h` | 76 |
| `safe_reference.h` | 75 |
| `global_vars.h` | 73 |
| `text.h` | 73 |
| `text_style_check_reader.cpp` | 73 |
| `translation_gendered.cpp` | 70 |
| `clone_ptr.h` | 69 |
| `translation_document.h` | 68 |
| `cata_scope_helpers.h` | 65 |
| `generic_factory.cpp` | 63 |
| `rect_range.h` | 63 |
| `math_parser_type.h` | 61 |
| `translation_manager.cpp` | 60 |
| `string_id.cpp` | 58 |
| `point_traits.h` | 57 |
| `localized_comparator.h` | 56 |
| `map_scale_constants.h` | 53 |
| `math_parser_diag.h` | 53 |
| `cartesian_product.h` | 52 |
| `cube_direction.h` | 50 |
| `localized_comparator.cpp` | 49 |
| `cata_bitset.cpp` | 48 |
| `char_validity_check.cpp` | 48 |
| `string_id_utils.h` | 48 |
| `math_parser_lex.h` | 46 |
| `ofstream_wrapper.h` | 45 |
| `translation_manager_impl.h` | 45 |
| `tgz_archiver.h` | 43 |
| `make_static.h` | 40 |
| `cata_assert.h` | 38 |
| `math_defines.h` | 38 |
| `translation_manager.h` | 38 |
| `math_parser.h` | 36 |
| `safe_reference.cpp` | 36 |
| `cached_options.cpp` | 35 |
| `demangle.cpp` | 35 |
| `math_parser_jmath.h` | 35 |
| `compatibility.h` | 33 |
| `calendar_ui.h` | 31 |
| `io_tags.h` | 31 |
| `sets_intersect.h` | 30 |
| `json_loader.h` | 29 |
| `platform_win.h` | 29 |
| `unicode.h` | 29 |
| `distribution.h` | 28 |
| `try_parse_integer.h` | 28 |
| `all_enum_values.h` | 27 |
| `memory_fast.h` | 25 |
| `translation_gendered.h` | 25 |
| `compatibility.cpp` | 24 |
| `coordinate_conversions.cpp` | 24 |
| `to_string_id.h` | 24 |
| `coordinate_conversions.h` | 22 |
| `lang_stats.cpp` | 22 |
| `system_locale.h` | 22 |
| `pinyin.h` | 21 |
| `cata_unreachable.h` | 20 |
| `lang_stats.h` | 18 |
| `version.cpp` | 18 |
| `json_error.h` | 17 |
| `std_hash_fs_path.h` | 17 |
| `cata_flatbuffers_assert.h` | 16 |
| `cata_type_traits.h` | 16 |
| `cata_inline.h` | 13 |
| `perf.cpp` | 13 |
| `demangle.h` | 12 |
| `ordered_static_globals.cpp` | 12 |
| `wcwidth.h` | 11 |
| `crash.h` | 8 |
| `char_validity_check.h` | 7 |
| `ordered_static_globals.h` | 7 |
| `get_version.h` | 5 |

## P1 — Content registry, mods, save format, oracle harness

131 files, 61,911 LOC.

| File | LOC |
|---|---|
| `savegame_json.cpp` | 5,353 |
| `item_factory.cpp` | 4,998 |
| `bionics.cpp` | 3,526 |
| `mutation.cpp` | 2,694 |
| `martialarts.cpp` | 2,557 |
| `savegame.cpp` | 1,951 |
| `veh_type.cpp` | 1,781 |
| `requirements.cpp` | 1,751 |
| `mapdata.cpp` | 1,515 |
| `event_statistics.cpp` | 1,308 |
| `mutation_data.cpp` | 1,173 |
| `memorial_logger.cpp` | 1,170 |
| `bionics_ui.cpp` | 1,049 |
| `profession.cpp` | 987 |
| `achievement.cpp` | 936 |
| `recipe_dictionary.cpp` | 926 |
| `init.cpp` | 919 |
| `damage.cpp` | 887 |
| `relic.cpp` | 839 |
| `mapdata.h` | 803 |
| `weakpoint.cpp` | 751 |
| `scenario.cpp` | 720 |
| `mutation_ui.cpp` | 673 |
| `skill.cpp` | 660 |
| `start_location.cpp` | 655 |
| `proficiency.cpp` | 650 |
| `mtype.cpp` | 639 |
| `mtype.h` | 615 |
| `mutation.h` | 586 |
| `veh_type.h` | 552 |
| `missiondef.cpp` | 547 |
| `mod_manager.cpp` | 532 |
| `requirements.h` | 492 |
| `trap.cpp` | 476 |
| `martialarts.h` | 468 |
| `assign.cpp` | 458 |
| `material.cpp` | 431 |
| `dependency_tree.cpp` | 428 |
| `field_type.cpp` | 428 |
| `proficiency_ui.cpp` | 427 |
| `bonuses.cpp` | 396 |
| `fault.cpp` | 392 |
| `trap.h` | 388 |
| `item_factory.h` | 385 |
| `stats_tracker.cpp` | 376 |
| `anatomy.cpp` | 368 |
| `assign.h` | 324 |
| `harvest.cpp` | 318 |
| `skill.h` | 314 |
| `trait_group.cpp` | 307 |
| `field_type.h` | 284 |
| `bionics.h` | 282 |
| `field.cpp` | 282 |
| `mod_manager_ui.cpp` | 275 |
| `relic.h` | 274 |
| `savegame_legacy.cpp` | 262 |
| `damage.h` | 259 |
| `achievement.h` | 251 |
| `stats_tracker.h` | 241 |
| `proficiency.h` | 229 |
| `recipe_dictionary.h` | 220 |
| `mondefense.cpp` | 209 |
| `mod_manager.h` | 208 |
| `weakpoint.h` | 208 |
| `trait_group.h` | 205 |
| `init.h` | 202 |
| `shop_cons_rate.cpp` | 198 |
| `past_games_info.cpp` | 197 |
| `field.h` | 192 |
| `profession.h` | 190 |
| `material.h` | 186 |
| `activity_type.cpp` | 184 |
| `scenario.h` | 182 |
| `event_bus.cpp` | 181 |
| `ammo_effect.cpp` | 176 |
| `loading_ui.cpp` | 174 |
| `vitamin.cpp` | 173 |
| `weather_type.cpp` | 173 |
| `clothing_mod.cpp` | 167 |
| `harvest.h` | 162 |
| `kill_tracker.cpp` | 144 |
| `weather_type.h` | 141 |
| `fault.h` | 137 |
| `vitamin.h` | 135 |
| `start_location.h` | 131 |
| `magic_type.cpp` | 128 |
| `event_field_transformations.cpp` | 124 |
| `event_statistics.h` | 120 |
| `bonuses.h` | 112 |
| `activity_type.h` | 108 |
| `dependency_tree.h` | 96 |
| `subbodypart.cpp` | 95 |
| `past_achievements_info.cpp` | 91 |
| `clothing_mod.h` | 88 |
| `subbodypart.h` | 88 |
| `mod_tracker.h` | 86 |
| `shop_cons_rate.h` | 86 |
| `ammo.cpp` | 82 |
| `ammo_effect.h` | 80 |
| `memorial_logger.h` | 78 |
| `anatomy.h` | 75 |
| `past_games_info.h` | 73 |
| `emit.h` | 72 |
| `magic_type.h` | 70 |
| `body_part_set.h` | 69 |
| `emit.cpp` | 69 |
| `speed_description.cpp` | 68 |
| `speed_description.h` | 65 |
| `profession_group.cpp` | 63 |
| `gun_mode.h` | 62 |
| `morale_types.cpp` | 62 |
| `mutation_type.cpp` | 60 |
| `common_types.h` | 55 |
| `mod_tileset.h` | 51 |
| `mod_tileset.cpp` | 47 |
| `kill_tracker.h` | 46 |
| `dispersion.cpp` | 42 |
| `morale_types.h` | 41 |
| `event_bus.h` | 39 |
| `dispersion.h` | 37 |
| `skill_ui.cpp` | 34 |
| `ammo.h` | 33 |
| `event_subscriber.h` | 30 |
| `past_achievements_info.h` | 30 |
| `profession_group.h` | 30 |
| `skill_ui.h` | 30 |
| `pocket_type.h` | 26 |
| `mondefense.h` | 24 |
| `event_field_transformations.h` | 21 |
| `overmap_types.h` | 19 |
| `loading_ui.h` | 13 |

## P2 — World grid, environment, map generation

60 files, 40,200 LOC.

| File | LOC |
|---|---|
| `map.cpp` | 10,698 |
| `map.h` | 2,557 |
| `map_field.cpp` | 2,291 |
| `sounds.cpp` | 2,070 |
| `computer_session.cpp` | 1,972 |
| `clzones.cpp` | 1,896 |
| `trapfunc.cpp` | 1,700 |
| `map_extras.cpp` | 1,566 |
| `lightmap.cpp` | 1,516 |
| `regional_settings.cpp` | 1,261 |
| `explosion.cpp` | 1,083 |
| `weather.cpp` | 1,000 |
| `clzones.h` | 685 |
| `shadowcasting.cpp` | 679 |
| `gates.cpp` | 615 |
| `map_memory.cpp` | 563 |
| `submap.cpp` | 494 |
| `teleport.cpp` | 494 |
| `timed_event.cpp` | 487 |
| `mapbuffer.cpp` | 460 |
| `computer.cpp` | 458 |
| `submap.h` | 438 |
| `climbing.cpp` | 410 |
| `regional_settings.h` | 361 |
| `grab.cpp` | 322 |
| `scent_map.cpp` | 318 |
| `fungal_effects.cpp` | 281 |
| `weather.h` | 236 |
| `map_iterator.h` | 206 |
| `climbing.h` | 197 |
| `map_memory.h` | 194 |
| `sounds.h` | 192 |
| `computer.h` | 181 |
| `active_item_cache.cpp` | 180 |
| `computer_session.h` | 156 |
| `map_item_stack.cpp` | 150 |
| `shadowcasting.h` | 146 |
| `explosion.h` | 124 |
| `timed_event.h` | 122 |
| `map_extras.h` | 106 |
| `map_selector.cpp` | 103 |
| `active_item_cache.h` | 101 |
| `level_cache.h` | 101 |
| `map_selector.h` | 98 |
| `mapbuffer.h` | 96 |
| `scent_map.h` | 93 |
| `cellular_automata.cpp` | 86 |
| `lightmap.h` | 86 |
| `gates.h` | 85 |
| `scent_block.cpp` | 81 |
| `level_cache.cpp` | 76 |
| `fire.h` | 58 |
| `map_item_stack.h` | 57 |
| `cellular_automata.h` | 46 |
| `scent_block.h` | 46 |
| `fragment_cloud.h` | 34 |
| `map_field.h` | 30 |
| `teleport.h` | 25 |
| `fungal_effects.h` | 21 |
| `maptile_fwd.h` | 12 |

## P3 — Items, inventory, crafting, interaction

86 files, 91,446 LOC.

| File | LOC |
|---|---|
| `item.cpp` | 16,206 |
| `iuse.cpp` | 9,257 |
| `iexamine.cpp` | 7,863 |
| `iuse_actor.cpp` | 6,161 |
| `inventory_ui.cpp` | 4,712 |
| `item.h` | 3,493 |
| `crafting.cpp` | 3,273 |
| `item_contents.cpp` | 2,847 |
| `construction.cpp` | 2,770 |
| `item_pocket.cpp` | 2,754 |
| `crafting_gui.cpp` | 2,573 |
| `consumption.cpp` | 1,972 |
| `itype.h` | 1,666 |
| `recipe.cpp` | 1,598 |
| `wish.cpp` | 1,476 |
| `butchery.cpp` | 1,441 |
| `iuse_actor.h` | 1,284 |
| `item_location.cpp` | 1,253 |
| `armor_layers.cpp` | 1,229 |
| `inventory_ui.h` | 1,205 |
| `inventory.cpp` | 1,182 |
| `item_group.cpp` | 1,117 |
| `visitable.cpp` | 1,043 |
| `auto_pickup.cpp` | 1,011 |
| `item_tname.cpp` | 878 |
| `handle_liquid.cpp` | 756 |
| `craft_command.cpp` | 600 |
| `item_pocket.h` | 580 |
| `itype.cpp` | 508 |
| `item_contents.h` | 453 |
| `iexamine_actors.cpp` | 452 |
| `item_action.cpp` | 436 |
| `item_group.h` | 424 |
| `iuse_software_sokoban.cpp` | 410 |
| `recipe.h` | 385 |
| `iuse.h` | 373 |
| `iuse_software_minesweeper.cpp` | 341 |
| `iuse_software_kitten.cpp` | 323 |
| `iteminfo_query.h` | 304 |
| `inventory.h` | 303 |
| `iuse_software_snake.cpp` | 256 |
| `item_search.cpp` | 220 |
| `iuse_software_lightson.cpp` | 215 |
| `recipe_groups.cpp` | 209 |
| `iexamine.h` | 197 |
| `item_location.h` | 193 |
| `item_components.cpp` | 192 |
| `auto_pickup.h` | 170 |
| `item_stack.cpp` | 166 |
| `item_category.cpp` | 156 |
| `item_tname.h` | 144 |
| `visitable.h` | 139 |
| `construction.h` | 137 |
| `craft_command.h` | 133 |
| `iexamine_actors.h` | 118 |
| `handle_liquid.h` | 117 |
| `butchery_requirements.cpp` | 112 |
| `item_category.h` | 112 |
| `rotatable_symbols.cpp` | 105 |
| `item_search.h` | 99 |
| `item_stack.h` | 93 |
| `iteminfo_query.cpp` | 90 |
| `iuse_software.cpp` | 79 |
| `construction_group.cpp` | 78 |
| `construction_category.cpp` | 73 |
| `item_action.h` | 70 |
| `butchery.h` | 62 |
| `iuse_software_kitten.h` | 62 |
| `iuse_software_sokoban.h` | 60 |
| `item_components.h` | 58 |
| `temp_crafting_inventory.h` | 56 |
| `contents_change_handler.cpp` | 55 |
| `contents_change_handler.h` | 52 |
| `crafting_gui.h` | 52 |
| `butchery_requirements.h` | 46 |
| `build_reqs.h` | 44 |
| `construction_category.h` | 42 |
| `construction_group.h` | 41 |
| `temp_crafting_inventory.cpp` | 40 |
| `iuse_software_lightson.h` | 39 |
| `iuse_software_minesweeper.h` | 36 |
| `recipe_groups.h` | 32 |
| `crafting.h` | 28 |
| `rotatable_symbols.h` | 24 |
| `iuse_software_snake.h` | 20 |
| `iuse_software.h` | 12 |

## P4 — Creatures, body, effects, combat maths

83 files, 74,569 LOC.

| File | LOC |
|---|---|
| `character.cpp` | 13,774 |
| `ranged.cpp` | 4,435 |
| `character.h` | 4,239 |
| `monster.cpp` | 4,185 |
| `npc.cpp` | 4,010 |
| `creature.cpp` | 3,583 |
| `melee.cpp` | 2,930 |
| `character_attire.cpp` | 2,656 |
| `suffer.cpp` | 2,125 |
| `monstergenerator.cpp` | 1,920 |
| `avatar.cpp` | 1,878 |
| `effect.cpp` | 1,789 |
| `player_hardcoded_effects.cpp` | 1,734 |
| `character_body.cpp` | 1,578 |
| `npc.h` | 1,517 |
| `creature.h` | 1,372 |
| `bodypart.cpp` | 1,254 |
| `morale.cpp` | 1,146 |
| `monexamine.cpp` | 1,049 |
| `medical_ui.cpp` | 1,017 |
| `npc_attack.cpp` | 865 |
| `character_inventory.cpp` | 827 |
| `bodygraph.cpp` | 724 |
| `turret.cpp` | 691 |
| `monster.h` | 682 |
| `ballistics.cpp` | 670 |
| `auto_note.cpp` | 602 |
| `effect_on_condition.cpp` | 583 |
| `bodypart.h` | 573 |
| `npc_class.cpp` | 543 |
| `stomach.cpp` | 541 |
| `character_escape.cpp` | 468 |
| `effect.h` | 462 |
| `creature_tracker.cpp` | 458 |
| `avatar.h` | 442 |
| `addiction.cpp` | 413 |
| `character_armor.cpp` | 408 |
| `character_modifier.cpp` | 400 |
| `sleep.cpp` | 347 |
| `character_guns.cpp` | 321 |
| `mondeath.cpp` | 290 |
| `character_attire.h` | 269 |
| `projectile.cpp` | 269 |
| `character_ammo.cpp` | 251 |
| `stomach.h` | 241 |
| `character_morale.cpp` | 234 |
| `character_crafting.cpp` | 231 |
| `creature_tracker.h` | 231 |
| `monfaction.cpp` | 211 |
| `morale.h` | 209 |
| `character_martial_arts.cpp` | 193 |
| `ballistics.h` | 185 |
| `auto_note.h` | 165 |
| `npc_class.h` | 165 |
| `npc_attack.h` | 146 |
| `character_proficiency.cpp` | 136 |
| `monster_oracle.cpp` | 125 |
| `monstergenerator.h` | 124 |
| `character_martial_arts.h` | 121 |
| `effect_on_condition.h` | 119 |
| `sleep.h` | 118 |
| `character_oracle.cpp` | 112 |
| `effect_source.cpp` | 103 |
| `monfaction.h` | 102 |
| `ranged.h` | 99 |
| `bodygraph.h` | 93 |
| `projectile.h` | 90 |
| `addiction.h` | 81 |
| `character_modifier.h` | 78 |
| `shearing.cpp` | 77 |
| `disease.cpp` | 63 |
| `character_id.h` | 54 |
| `shearing.h` | 51 |
| `effect_source.h` | 50 |
| `npc_opinion.h` | 43 |
| `disease.h` | 42 |
| `character_oracle.h` | 37 |
| `npc_favor.h` | 36 |
| `monster_oracle.h` | 34 |
| `melee.h` | 31 |
| `mondeath.h` | 24 |
| `monexamine.h` | 17 |
| `character_id.cpp` | 8 |

## P5 — Spells, enchantments, relics

10 files, 9,444 LOC.

| File | LOC |
|---|---|
| `magic.cpp` | 3,633 |
| `magic_spell_effect.cpp` | 2,018 |
| `magic_enchantment.cpp` | 1,667 |
| `magic.h` | 979 |
| `magic_enchantment.h` | 443 |
| `magic_ter_fur_transform.cpp` | 317 |
| `magic_teleporter_list.cpp` | 232 |
| `magic_ter_furn_transform.h` | 92 |
| `magic_teleporter_list.h` | 45 |
| `magic_spell_effect_helpers.h` | 18 |

## P6 — Activities and action pipeline

22 files, 26,317 LOC.

| File | LOC |
|---|---|
| `activity_actor.cpp` | 9,401 |
| `activity_item_handling.cpp` | 3,900 |
| `handle_action.cpp` | 3,295 |
| `activity_handlers.cpp` | 2,691 |
| `activity_actor_definitions.h` | 2,573 |
| `avatar_action.cpp` | 1,269 |
| `player_activity.cpp` | 560 |
| `pickup.cpp` | 520 |
| `gamemode_tutorial.cpp` | 387 |
| `activity_handlers.h` | 282 |
| `uistate.h` | 243 |
| `move_mode.cpp` | 233 |
| `player_activity.h` | 188 |
| `activity_tracker.cpp` | 179 |
| `activity_actor.h` | 139 |
| `move_mode.h` | 110 |
| `avatar_action.h` | 89 |
| `gamemode_tutorial.h` | 74 |
| `activity_tracker.h` | 57 |
| `pickup.h` | 53 |
| `gamemode.cpp` | 37 |
| `gamemode.h` | 37 |

## P7 — AI, pathfinding, dialogue and scripting

57 files, 38,090 LOC.

| File | LOC |
|---|---|
| `npctalk.cpp` | 9,054 |
| `npcmove.cpp` | 5,498 |
| `monattack.cpp` | 4,757 |
| `condition.cpp` | 2,836 |
| `monmove.cpp` | 2,554 |
| `talker_character.cpp` | 1,421 |
| `npctalk_funcs.cpp` | 1,363 |
| `mattack_actors.cpp` | 1,335 |
| `talker.h` | 906 |
| `talker_npc.cpp` | 904 |
| `pathfinding.cpp` | 671 |
| `dialogue.h` | 544 |
| `simple_pathfinding.cpp` | 520 |
| `npctalk_rules.cpp` | 442 |
| `npctrade.cpp` | 375 |
| `talker_character.h` | 327 |
| `mattack_actors.h` | 279 |
| `talker_monster.cpp` | 262 |
| `dialogue_win.cpp` | 215 |
| `flood_fill.h` | 212 |
| `behavior.cpp` | 210 |
| `npctrade_utils.cpp` | 202 |
| `talker_vehicle.cpp` | 197 |
| `pathfinding.h` | 190 |
| `talker_item.cpp` | 181 |
| `dialogue_helpers.h` | 180 |
| `dialogue_helpers.cpp` | 176 |
| `dialogue_chatbin.h` | 166 |
| `npctalk.h` | 150 |
| `simple_pathfinding.h` | 141 |
| `talker_npc.h` | 129 |
| `talker_monster.h` | 124 |
| `talker_avatar.cpp` | 111 |
| `condition.h` | 104 |
| `monattack.h` | 102 |
| `behavior.h` | 100 |
| `dialogue_win.h` | 99 |
| `talker_vehicle.h` | 99 |
| `talker_item.h` | 96 |
| `npctalk_rules.h` | 71 |
| `talker_furniture.cpp` | 70 |
| `talker_furniture.h` | 69 |
| `npctrade.h` | 68 |
| `mattack_common.h` | 67 |
| `behavior_strategy.cpp` | 61 |
| `dialogue_chatbin.cpp` | 60 |
| `talker_avatar.h` | 57 |
| `talker_zone.h` | 56 |
| `behavior_oracle.cpp` | 51 |
| `talker_topic.h` | 51 |
| `behavior_strategy.h` | 46 |
| `speech.cpp` | 42 |
| `behavior_oracle.h` | 31 |
| `speech.h` | 20 |
| `npctrade_utils.h` | 16 |
| `talker_topic.cpp` | 13 |
| `talker_zone.cpp` | 9 |

## P8 — Vehicles

21 files, 25,752 LOC.

| File | LOC |
|---|---|
| `vehicle.cpp` | 8,897 |
| `veh_interact.cpp` | 3,395 |
| `vehicle_use.cpp` | 2,613 |
| `vehicle.h` | 2,594 |
| `vehicle_move.cpp` | 2,347 |
| `vehicle_autodrive.cpp` | 1,537 |
| `vehicle_part.cpp` | 783 |
| `veh_appliance.cpp` | 694 |
| `vehicle_display.cpp` | 509 |
| `veh_utils.cpp` | 500 |
| `veh_interact.h` | 305 |
| `veh_shape.cpp` | 270 |
| `vehicle_group.cpp` | 254 |
| `vehicle_group.h` | 195 |
| `vpart_position.h` | 189 |
| `vpart_range.h` | 186 |
| `veh_appliance.h` | 167 |
| `vehicle_selector.h` | 116 |
| `veh_utils.h` | 106 |
| `veh_shape.h` | 61 |
| `vehicle_selector.cpp` | 34 |

## P9 — Overmap, world generation, factions, missions

43 files, 47,547 LOC.

| File | LOC |
|---|---|
| `mapgen.cpp` | 8,888 |
| `overmap.cpp` | 8,222 |
| `faction_camp.cpp` | 6,354 |
| `mission_companion.cpp` | 2,938 |
| `overmap_ui.cpp` | 2,743 |
| `worldfactory.cpp` | 2,475 |
| `mapgen_functions.cpp` | 2,257 |
| `overmapbuffer.cpp` | 1,964 |
| `faction.cpp` | 1,186 |
| `basecamp.cpp` | 1,074 |
| `mission.cpp` | 940 |
| `overmap.h` | 806 |
| `omdata.h` | 801 |
| `overmapbuffer.h` | 686 |
| `mapgen.h` | 660 |
| `mongroup.cpp` | 633 |
| `mission_util.cpp` | 577 |
| `basecamp.h` | 542 |
| `mission.h` | 468 |
| `mission_start.cpp` | 434 |
| `mapgendata.cpp` | 345 |
| `mongroup.h` | 282 |
| `mapgendata.h` | 279 |
| `mission_companion.h` | 250 |
| `faction.h` | 222 |
| `worldfactory.h` | 188 |
| `overmap_ui.h` | 179 |
| `overmap_connection.cpp` | 167 |
| `overmap_location.cpp` | 105 |
| `city.cpp` | 98 |
| `overmap_noise.h` | 94 |
| `mapgenformat.h` | 90 |
| `overmap_connection.h` | 90 |
| `mapgen_functions.h` | 77 |
| `mapgen_parameter.h` | 73 |
| `mapgenformat.cpp` | 64 |
| `city.h` | 55 |
| `faction_camp.h` | 54 |
| `overmap_location.h` | 49 |
| `overmap_noise.cpp` | 48 |
| `mapgen_primitives.h` | 47 |
| `jmapgen_flags.h` | 27 |
| `mission_place.cpp` | 16 |

## P10 — Interface, rendering, audio

113 files, 59,329 LOC.

| File | LOC |
|---|---|
| `cata_tiles.cpp` | 5,616 |
| `newcharacter.cpp` | 5,143 |
| `sdltiles.cpp` | 4,284 |
| `output.cpp` | 3,447 |
| `advanced_inv.cpp` | 2,481 |
| `widget.cpp` | 2,016 |
| `player_display.cpp` | 1,977 |
| `display.cpp` | 1,531 |
| `input_context.cpp` | 1,465 |
| `output.h` | 1,302 |
| `uilist.cpp` | 1,301 |
| `cata_imgui.cpp` | 1,262 |
| `action.cpp` | 1,259 |
| `sdlsound.cpp` | 1,116 |
| `animation.cpp` | 1,065 |
| `messages.cpp` | 1,051 |
| `input.cpp` | 923 |
| `cata_tiles.h` | 903 |
| `safemode_ui.cpp` | 884 |
| `panels.cpp` | 881 |
| `wincurse.cpp` | 827 |
| `diary.cpp` | 791 |
| `string_input_popup.cpp` | 722 |
| `uilist.h` | 707 |
| `action.h` | 675 |
| `ncurses_def.cpp` | 628 |
| `sdl_font.cpp` | 627 |
| `pixel_minimap.cpp` | 602 |
| `string_editor_window.cpp` | 557 |
| `cursesport.cpp` | 550 |
| `ui_manager.cpp` | 543 |
| `input_context.h` | 514 |
| `advanced_inv_pane.cpp` | 490 |
| `popup.cpp` | 456 |
| `input_popup.cpp` | 412 |
| `widget.h` | 410 |
| `diary_ui.cpp` | 409 |
| `input.h` | 383 |
| `scores_ui.cpp` | 376 |
| `mission_ui.cpp` | 365 |
| `sdl_gamepad.cpp` | 347 |
| `trade_ui.cpp` | 339 |
| `advanced_inv_area.cpp` | 314 |
| `string_input_popup.h` | 312 |
| `ui_manager.h` | 302 |
| `help.cpp` | 298 |
| `popup.h` | 282 |
| `ui_extended_description.cpp` | 250 |
| `font_loader.cpp` | 235 |
| `advanced_inv.h` | 227 |
| `display.h` | 212 |
| `sdl_utils.cpp` | 212 |
| `sdl_wrappers.cpp` | 197 |
| `smart_controller_ui.cpp` | 192 |
| `messages.h` | 184 |
| `sdl_font.h` | 176 |
| `cata_imgui.h` | 174 |
| `cursesdef.h` | 167 |
| `input_enums.h` | 167 |
| `tileray.cpp` | 163 |
| `panels.h` | 161 |
| `advanced_inv_pane.h` | 151 |
| `diary.h` | 150 |
| `sdl_wrappers.h` | 150 |
| `imgui_demo.cpp` | 141 |
| `trade_ui.h` | 134 |
| `safemode_ui.h` | 131 |
| `input_popup.h` | 125 |
| `mood_face.cpp` | 119 |
| `advanced_inv_area.h` | 109 |
| `pixel_minimap_projectors.cpp` | 98 |
| `ui_iteminfo.cpp` | 97 |
| `pixel_minimap.h` | 95 |
| `music.cpp` | 93 |
| `sdl_utils.h` | 92 |
| `cursesport.h` | 90 |
| `advanced_inv_listitem.h` | 89 |
| `live_view.cpp` | 83 |
| `font_loader.h` | 81 |
| `sdl_geometry.cpp` | 74 |
| `string_editor_window.h` | 74 |
| `drawing_primitives.cpp` | 72 |
| `sdltiles.h` | 72 |
| `tileray.h` | 70 |
| `mood_face.h` | 67 |
| `smart_controller_ui.h` | 66 |
| `sdl_geometry.h` | 59 |
| `pixel_minimap_projectors.h` | 56 |
| `ascii_art.cpp` | 53 |
| `ui_extended_description.h` | 53 |
| `advanced_inv_pagination.cpp` | 46 |
| `overlay_ordering.cpp` | 46 |
| `ui_style_picker.cpp` | 46 |
| `help.h` | 45 |
| `advanced_inv_listitem.cpp` | 44 |
| `end_screen.cpp` | 41 |
| `animation.h` | 39 |
| `music.h` | 37 |
| `sdlsound.h` | 35 |
| `advanced_inv_pagination.h` | 34 |
| `end_screen.h` | 32 |
| `ui_iteminfo.h` | 32 |
| `imgui_demo.h` | 31 |
| `live_view.h` | 30 |
| `ascii_art.h` | 28 |
| `game_ui.cpp` | 27 |
| `sdl_gamepad.h` | 27 |
| `drawing_primitives.h` | 26 |
| `viewer.h` | 22 |
| `overlay_ordering.h` | 19 |
| `game_ui.h` | 18 |
| `ui_style_picker.h` | 11 |
| `scores_ui.h` | 7 |

## P11 — Runtime shell, platform, tooling, editors

25 files, 27,612 LOC.

| File | LOC |
|---|---|
| `game.cpp` | 14,535 |
| `game_inventory.cpp` | 3,183 |
| `editmap.cpp` | 2,203 |
| `game.h` | 1,373 |
| `main_menu.cpp` | 1,278 |
| `event.h` | 1,053 |
| `main.cpp` | 869 |
| `do_turn.cpp` | 780 |
| `player_difficulty.cpp` | 455 |
| `event.cpp` | 274 |
| `test_data.cpp` | 256 |
| `game_inventory.h` | 204 |
| `game_constants.h` | 193 |
| `distraction_manager.cpp` | 183 |
| `mapsharing.cpp` | 171 |
| `test_data.h` | 136 |
| `editmap.h` | 129 |
| `main_menu.h` | 127 |
| `player_difficulty.h` | 68 |
| `mapsharing.h` | 42 |
| `current_map.h` | 39 |
| `current_map.cpp` | 21 |
| `distraction_manager.h` | 18 |
| `emscripten_exception.cpp` | 13 |
| `do_turn.h` | 9 |
