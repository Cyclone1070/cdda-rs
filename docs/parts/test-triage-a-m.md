# Test triage, files a-m

Scope: 130 files in `tests/` whose names begin a-m, from the index at [test-triage-index.json](docs/parts/test-triage-index.json) (783 test cases, 51,235 lines). Repo pinned at `0.I-1`, commit `7b2efa5cea38e4d4d97dd0e63b28b9148623da59` (verified with `git log -1`). Every row below comes from reading the file; counts in the file column are not recounted.

Reuse legend: **RUST-TEST** = unit test worth re-expressing in the new language; **SESSION** = drive the game with a recorded input sequence; **INVARIANT** = keep as a property/range check; **DROP** = do not port.

| file | what the tests pin down | port area | reuse | risk note |
|---|---|---|---|---|
| `act_build_test.cpp` | Multi-construction activity consumes blueprint zones and finishes builds for avatar and NPC | activities | SESSION | (b) needs clear_map + placed zones/faction; timing loop drives turns directly |
| `active_item_cache_test.cpp` | Active items placed at edge/interior coordinates are still found by the map active-item cache | items | RUST-TEST | (b) needs a live map; no tuned numbers |
| `active_item_test.cpp` | Active items tick each turn; tool discharge rate matches battery capacity | items | RUST-TEST | (a) asserts 56 kJ and a 9.5-10 h discharge window |
| `activity_scheduling_helper.cpp` | No test cases: scheduling fixture that assigns and sustains an activity across turns | activities | DROP | (d) helper/fixture only |
| `activity_tracker_test.cpp` | Rolling activity tracker returns latest value and average over recent turns | activities | RUST-TEST | mutates calendar::turn; uses rng_float but only for values below the logged max |
| `addiction_test.cpp` | Each addiction drains morale/stim and adds health penalty at a fixed per-turn rate | creatures | RUST-TEST | (a) heavy: Approx margins on 100k-iteration sums (50+/-40, 1300+/-500, 30000+/-10000) |
| `advanced_inventory_test.cpp` | Advanced-inventory panes move items between ground, inventory and vehicle cargo | interface | SESSION | (c) drives the AIM UI object via named actions; (b) needs map and avatar |
| `algo_test.cpp` | find_cycles returns every elementary cycle of a directed graph | foundation | RUST-TEST | pure algorithm, no game state |
| `ammo_set_test.cpp` | ammo_set fills magazine, magazine-well and container pockets and respects capacity | items | RUST-TEST | (a) exact capacities/remaining counts taken from JSON content |
| `ammo_test.cpp` | Ammo type/default resolution, barrel length limits and battery energy per charge | items | RUST-TEST | (a) exact energy and capacity values; (b) needs loaded item database |
| `archery_damage_test.cpp` | Bow/arrow tiers bracket which test animals a shot can kill and whether it breaks a window | items | INVARIANT | (a) worst offender: hard-coded kill/no-kill table per weapon-animal pair |
| `battery_density_test.cpp` | Every battery chemistry stays under a per-chemistry energy density ceiling | items | INVARIANT | (b) scans loaded item database; ceilings are external real-world figures |
| `battery_mod_test.cpp` | Battery toolmods install, charge and report capacity like built-in cells | items | RUST-TEST | (b) avatar inventory fixture; (a) a few exact charge counts |
| `behavior_test.cpp` | Behaviour-tree nodes and oracles choose NPC/monster goals correctly | ai | RUST-TEST | (b) monster cases need nested mapgen setup and a created map |
| `bionics_test.cpp` | Bionic power capacity, UIDs, weapon bionics and fueled bionics behave as defined | creatures | RUST-TEST | (a) exact power capacity arithmetic; (b) avatar/map fixture |
| `calendar_test.cpp` | time_duration formatting strings for normal and eternal-season modes | foundation | RUST-TEST | (a) exact rendered strings; [nogame] |
| `cardio_test.cpp` | Cardio fitness combines traits, health, athletics and daily activity into stamina/weariness | creatures | RUST-TEST | (a) tuned formula constants; (b) avatar + map fixture |
| `cartesian_product_test.cpp` | cartesian_product handles empty, singleton and pair inputs | foundation | RUST-TEST | pure, [nogame] |
| `cata_generators.cpp` | No test cases: Catch2 random point/tripoint generators | tools | DROP | (d) test-harness generator only |
| `cata_utility_test.cpp` | String prefix/suffix helpers, divide_round_up, erase_if and map filters | foundation | RUST-TEST | contains one [benchmark] case; (a) exact unit-conversion results |
| `cata_variant_test.cpp` | cata_variant constructs, copies, serializes and validates typed values | foundation | RUST-TEST | (b) one case touches debug menu ids; mostly [nogame] |
| `catacharset_test.cpp` | UTF-8 width, split, base64, case folding and accent removal | foundation | RUST-TEST | (a) exact widths/codepoints; [nogame] |
| `char_biometrics_test.cpp` | BMI, bodyweight, height, BMR and speed derive from size traits and stored kcal | creatures | RUST-TEST | (a) 156 numeric asserts on tuned constants; (c) reads display::weight_string |
| `char_edible_rating_test.cpp` | can_eat/will_eat gating by traits, allergies, nausea, frozen, rotten and cannibalism | creatures | RUST-TEST | boolean outcomes, good portability; needs loaded item data |
| `char_exposure_test.cpp` | Bodypart exposure computed from worn clothing coverage | creatures | RUST-TEST | (a) exact exposure fractions |
| `char_healing_test.cpp` | Healing rate from rest quality, healing traits, health stat, bandage and disinfect | creatures | RUST-TEST | (a) 73 asserts on tuned multipliers; rest values are constants |
| `char_sight_test.cpp` | Sight limits, impaired vision and fine-detail vision modifier vs light level | creatures | RUST-TEST | (b) needs map, lightmap and NPC fixtures |
| `char_stamina_test.cpp` | Stamina cost and regeneration from movement mode, encumbrance and limb state | creatures | RUST-TEST | (a) 64 exact stamina numbers |
| `char_suffer_test.cpp` | Sunburn, albinism and asphyxiation damage accumulate per turn | creatures | RUST-TEST | (a) tuned per-turn damage; (b) needs map/avatar |
| `char_validity_check_test.cpp` | is_char_allowed rejects control, path and wildcard characters | foundation | RUST-TEST | pure, no state |
| `char_volume_test.cpp` | Base body volume and height scale with size mutations | creatures | RUST-TEST | (a) exact volume/height numbers |
| `character_modifier_test.cpp` | Limb scores feed character modifiers that scale armor, damage and slip prevention | creatures | RUST-TEST | (a) 83 numeric asserts on modifier arithmetic |
| `clzones_test.cpp` | Zone activities unload ammo belts and sort comestibles into matching zones | world | SESSION | (b) needs map, zones, vehicle and loot activity loop |
| `colony_test.cpp` | colony container insert/erase/sort/splice semantics and capacity growth | foundation | RUST-TEST | pure data structure, deterministic |
| `comestible_test.cpp` | Comestible health bounds, recipe nutrition and calorie density sanity checks | items | INVARIANT | (a) exact kcal/satiety; (c) satiety bar is a colourised UI string |
| `connect_rotate_test.cpp` | Multitile walls/windows connect and rotate to the right neighbours | interface | DROP | (c) TILES-only file (guarded by #if defined(TILES)); rendering-specific |
| `consumption_time_test.cpp` | Consuming a comestible takes at least one second | items | INVARIANT | lower-bound only, very portable |
| `coordinate_test.cpp` | Coordinate types convert, compare, hash and combine consistently | foundation | RUST-TEST | pure, [nogame] |
| `coverage_test.cpp` | Armor coverage and material portions decide protection from melee and bullets | items | RUST-TEST | (a) exact resistances; ablative-vest cases are tuned |
| `crafting_test.cpp` | Crafting: recipe subsets, proficiencies, failure rates, tool charges, UPS, byproducts, rot | activities | SESSION | (a) failure-rate and time numbers; (b) map/vehicle/companion; (c) liquid-container prompts |
| `creature_effect_test.cpp` | Effects add, stack, remove and are blocked by monster/character immunity | creatures | RUST-TEST | (a) a few exact durations; mostly structural |
| `creature_in_field_test.cpp` | A monster on an acid field takes damage unless inside a vehicle | world | RUST-TEST | (b) needs clear_map, vehicle and monster spawn |
| `creature_test.cpp` | Bodypart hit distribution weights and mtype species membership | creatures | RUST-TEST | (a) hard-coded weight tables (e.g. 1032.63) and a statistical hit-roll loop |
| `cube_direction_test.cpp` | cube_direction arithmetic with overmap directions and raw ints | foundation | RUST-TEST | pure enum arithmetic |
| `degradation_test.cpp` | Item degradation accumulates, thresholds map to damage levels, repair restores it | items | RUST-TEST | (a) 113 numeric asserts incl. averaging over 4000 iterations; (b) map fixture |
| `distribution_test.cpp` | Poisson int_distribution reports description and minimum | foundation | RUST-TEST | tiny, JSON-driven |
| `effect_test.cpp` | Effect duration, intensity, decay, permanence, bodypart and vitamin effects | creatures | RUST-TEST | (a) 93 numeric asserts; (b) heavy global-state clearing |
| `effective_dps_test.cpp` | Weapon DPS against fixed monsters matches values stored in TEST_DATA | items | INVARIANT | (a) 1000-trial averages compared to a checked-in expected table; (c) emits UI warnings |
| `enchantments_test.cpp` | Worn/bionic/mutation enchantments alter stats, speed, dodge and stamina use | creatures | RUST-TEST | (a) exact stat/speed deltas; (b) map fixture |
| `encumbrance_test.cpp` | Clothing layers, fit and extra pockets produce expected encumbrance | items | RUST-TEST | exact integers, no RNG |
| `eoc_test.cpp` | Effect-on-condition scripting: math, dialogue, timed events, mutations, spells, recipes | spells | RUST-TEST | (a) 236 numeric asserts; (b) needs loaded TEST_DATA EOCs and map |
| `event_test.cpp` | Event construction, bus dispatch, subscriber lifetime and notification order | foundation | RUST-TEST | pure event plumbing |
| `explosion_balance_test.cpp` | Grenade damage is independent of monster size; lethality and vehicle damage brackets | items | INVARIANT | (a) tuned probability brackets (0.99/0.95/0.40, 0.998); (b) RNG over 10000 hits |
| `faction_camp_test.cpp` | Camp food supply counts kcal and vitamins from items and their components | activities | RUST-TEST | (a) exact kcal/vitamin totals; (b) needs overmap camp and zones |
| `faction_price_rules_test.cpp` | NPC trade prices follow faction price rules and item category multipliers | world | RUST-TEST | (a) 21 exact price values; (b) avatar/NPC fixture |
| `fake_messages.cpp` | No test cases: stub Messages implementation that collects log lines for failures | tools | DROP | (d) link-time stub, not a test |
| `field_test.cpp` | Fields expire, spread, burn, radiate and apply effects to characters standing in them | world | RUST-TEST | (a) exact field ages/intensities; (b) needs map; one case is [!mayfail] |
| `firstaid_test.cpp` | Bandaging activity heals over time and is interrupted by pain for avatar and NPC | activities | SESSION | (b) drives activity turns on a created map; interrupt timing is tuned |
| `flat_set_test.cpp` | flat_set supports insert, range ops, ordering, comparison and transparent lookup | foundation | RUST-TEST | pure container semantics |
| `focus_test.cpp` | Focus drifts down on failure and up on practice, staying within bounds | creatures | INVARIANT | assertions are mostly inequalities (>=50, <150) |
| `fold_string_test.cpp` | foldstring wraps text at a column limit | interface | RUST-TEST | (c) lives in output.h/UI layer; exact wrapped strings |
| `food_fun_for_test.cpp` | fun_for adjusts by sickness, rot, temperature, cat/dog food, gourmand, monotony | items | RUST-TEST | (a) exact fun values; needs loaded item data |
| `force_load_game_test.cpp` | Empty placeholder that forces the game data to load | tools | DROP | no behaviour asserted |
| `fuel_test.cpp` | fuel_energy returns per-charge energy for battery and gasoline | items | RUST-TEST | (a) exact 1 kJ and 34200 J constants |
| `gates_test.cpp` | Doors and windows open/close, emit a sound and cost moves | world | RUST-TEST | headless; sound checks go through the sounds bus, not the UI |
| `generic_factory_test.cpp` | generic_factory insert/overwrite/invalidate and string_id ordering | foundation | RUST-TEST | contains three [benchmark] cases to skip on port |
| `ground_destroy_test.cpp` | Destroying pavement or explosions never leave a flat roof; collapse checks | world | RUST-TEST | (b) needs clear_map_and_put_player_underground; hidden [.] tag |
| `harvest_test.cpp` | Dissecting corpses yields samples, CBMs or other drops per monster | creatures | RUST-TEST | (a) statistical over 1000 butcher iterations; (b) map fixture |
| `hash_test.cpp` | point/tripoint hashes are near-unique over the sampled coordinate space | foundation | RUST-TEST | pure; 0.9 uniqueness threshold is a tuned bound |
| `health_test.cpp` | Daily routine of stamina burn, vitamins and sleep shifts lifestyle health | creatures | INVARIANT | only asserts lifestyle moves up or down, no exact numbers; mutates calendar::turn and uses a standard_npc |
| `iexamine_test.cpp` | Examine data lookups and seasonal harvestability of trees | world | RUST-TEST | (b) needs clear_map; calendar is set explicitly |
| `invlet_test.cpp` | Inventory letters are assigned, cached and reassigned as items move between locations | items | RUST-TEST | (b) large combinatorial fixture over inventory locations/actions |
| `item_autopickup_test.cpp` | Autopickup rules honour container contents, rigidity, volume, weight and ownership | items | RUST-TEST | (b) needs map/avatar; a few exact volume/weight limits |
| `item_contents_test.cpp` | Item contents combine, overflow to parent or ground, and keep counts | items | RUST-TEST | (a) exact overflow counts; (b) map fixture |
| `item_countdown_test.cpp` | Countdown items trigger their action at zero and can revert to a prior type | items | RUST-TEST | calendar-driven; one exact duration |
| `item_group_test.cpp` | Item groups truncate, spill, apply damage/charges and respect event gating | items | RUST-TEST | (a) spawn counts; RNG-driven with explicit seeding |
| `item_location_test.cpp` | item_location survives item removal and never returns a stale map item | items | RUST-TEST | (b) map + avatar fixture |
| `item_pickup_test.cpp` | put_in/i_add/wear_item copy items, so original references are not the stored items | items | RUST-TEST | (b) map + avatar fixture |
| `item_pocket_test.cpp` | Pockets enforce length/volume/flag/favourite rules, seal, liquid and best-pocket choice | items | RUST-TEST | (b) heavy map/avatar fixtures; (a) exact volumes and capacities |
| `item_spawn_test.cpp` | Items spawned by groups, recipes, vehicles, professions and map all land in their container | content-loading | INVARIANT | driven by a JSON expectation table; (b) needs loaded data |
| `item_stackable_test.cpp` | Every non-solid comestible stacks by charges | content-loading | INVARIANT | sweeps the whole item database, no fixed numbers |
| `item_test.cpp` | Item volume, layers, stacking, temperature, armor compliance and nested contents | items | RUST-TEST | (a) exact volumes/lengths; (b) avatar fixture |
| `item_tick_action_test.cpp` | An unfuelled chainsaw's tick_action turns it off and swaps its type | items | RUST-TEST | single deterministic case |
| `item_tname_test.cpp` | Item display names reflect temperature, wetness, filth, damage bar and nested contents | interface | RUST-TEST | (c) user-facing strings; (a) 112 numeric asserts |
| `item_type_name_test.cpp` | type_name pluralises, honours custom names and renders blood and corpses | items | RUST-TEST | (a) exact strings incl. plural forms |
| `iteminfo_test.cpp` | The item-info panel renders volume, armor, gun, food, pocket and flag lines | interface | RUST-TEST | (c) asserts rendered UI text; (a) 59 numeric asserts on tuned values |
| `itemname_test.cpp` | display_name adds sizing and contents annotations, and marks rotten food | interface | RUST-TEST | (c) user-facing string assembly; (a) exact names |
| `iuse_actor_test.cpp` | Manhack deployment, tool transform on activation and cut-up yields | items | RUST-TEST | (b) needs map/creature tracker; cut-up case is a table-driven sweep over many items, yields come from JSON |
| `iuse_test.cpp` | Item activation effects: eyedrops, antifungals, oxygen, caffeine, towel, drugs, pur tablets | items | RUST-TEST | (a) 110 numeric asserts on durations/charges; (b) avatar fixture |
| `json_load_test.cpp` | JSON load inheritance: damage instances do not extend, attack cooldowns do | content-loading | RUST-TEST | (a) exact loaded values; needs TEST_DATA mod |
| `json_test.cpp` | Serialization round-trips, translation style checks and unicode error positions | foundation | RUST-TEST | (a) exact serialized strings; no [nogame] tag, so the file runs against loaded game data |
| `limb_test.cpp` | Gaining/losing limbs, limb flags, ugliness, mending, drying and limb armor coverage | creatures | RUST-TEST | (a) 40 numeric asserts; (b) map fixture |
| `line_test.cpp` | Bresenham line drawing, direction enums and squares-closer-to ordering | foundation | RUST-TEST | (a) 136 numeric asserts; one [.] performance case |
| `list_test.cpp` | Intrusive list basics, splice, sort, unique, resize and insertion styles | foundation | RUST-TEST | pure container semantics |
| `magic_spell_effect_test.cpp` | Line-attack spells place fields along a line and remove fatigue fields | spells | RUST-TEST | (b) needs map + TEST_DATA spells |
| `magic_spell_test.cpp` | Spell name, level, damage, duration, range, AOE and target/summon/recover effects | spells | RUST-TEST | (a) 161 numeric asserts on level-scaled damage and range |
| `make_static_test.cpp` | make_static macro caches string_id values | foundation | DROP | (d) C++ macro/optimisation detail with no cross-language meaning |
| `map_bash_test.cpp` | Bashing terrain uses expected chances; shooting terrain damages and persists | world | RUST-TEST | (a) tuned bash chances; (b) needs map; RNG-heavy |
| `map_helpers.cpp` | No test cases: shared helpers to clear map, creatures, vehicles, fields and zones | tools | DROP | (d) helper/fixture module |
| `map_iterator_test.cpp` | tripoint_range iterates squares/circles in the documented order and bounds | foundation | RUST-TEST | pure geometry |
| `map_memory_test.cpp` | Map memory remembers, overwrites and forgets tiles across z-levels | world | RUST-TEST | (b) needs map; includes an lru_cache [.] performance case |
| `map_path_test.cpp` | Path finding routes around obstacles, into danger, up stairs and when unreachable | world | RUST-TEST | (b) needs clear_map, caches and player placement; (a) path lengths |
| `map_test.cpp` | Map bounds checking, coordinate conversion, active contents and milk rotting | world | RUST-TEST | (b) needs map + avatar; (a) rot timing |
| `map_test_case.cpp` | No test cases: map_test_case fixture that sets up, transforms and checks map regions | tools | DROP | (d) test-harness framework |
| `map_test_case_test.cpp` | The map_test_case harness itself transforms and compares regions correctly | tools | DROP | (d) tests the harness, not the game |
| `mapgen_function_test.cpp` | Overmap terrain sewer variants connect to the expected neighbours | worldgen | RUST-TEST | (a) hard-coded connection table; needs overmap terrain data |
| `mapgen_helpers.cpp` | No test cases: helpers to run update/nested mapgen manually | tools | DROP | (d) helper/fixture module |
| `mapgen_place_vehicles_test.cpp` | Update and nested mapgen can place a vehicle at a location | worldgen | RUST-TEST | (b) needs mapgen run on a created map |
| `mapgen_remove_npcs_test.cpp` | Update mapgen places and removes NPCs by template | worldgen | RUST-TEST | (b) needs creature tracker and overmap position |
| `mapgen_remove_vehicles_test.cpp` | Mapgen removes one or all vehicles while leaving the rest functional | vehicles | RUST-TEST | (b) needs map, vehicles and driving check; (a) velocity 1000 |
| `martial_art_test.cpp` | Martial arts gate on weapon categories and techniques fire on their conditions | creatures | RUST-TEST | (a) 48 numeric asserts; (b) map/creature fixtures |
| `materials_test.cpp` | Material portions drive resistance, flammability and glass breakability | items | RUST-TEST | (a) exact resistance splits; (b) 1000-iteration loops |
| `math_parser_test.cpp` | Math parser evaluates arithmetic, functions and dialogue-linked variables | spells | RUST-TEST | (a) 240 numeric asserts; (b) needs avatar/dialogue context |
| `mdarray_test.cpp` | mdarray default dimensions match map/overmap constants | foundation | DROP | compile-time static_asserts only; no runtime behaviour to port |
| `melee_dodge_hit_test.cpp` | Hit and dodge bases scale with DEX, skill, effects and stamina | creatures | RUST-TEST | (a) 63 exact float values |
| `melee_test.cpp` | Melee hit probability, dodge, skill training caps and damage-type EOCs | creatures | RUST-TEST | (a) tuned probability windows (0.6+/-0.1, 0.975+/-0.025); (b) map |
| `memorial_test.cpp` | Memorial log records events and dumps/parses the expected lines | foundation | RUST-TEST | (a) exact log strings and EOL handling; load/save round-trip through a stream |
| `mission_test.cpp` | Mission goal conditions complete the mission and grant morale only when met | world | RUST-TEST | (b) needs avatar/NPC global state; (a) morale 10 |
| `modify_morale_test.cpp` | Eating and drugs modify morale by fun, table/chair, temperature, allergies and traits | creatures | RUST-TEST | (a) 33 exact morale deltas; (b) map fixture |
| `mondefense_test.cpp` | Zapback shocks attackers unless insulated, ranged or electricity immune | creatures | RUST-TEST | boolean outcomes, good portability |
| `monfactions_test.cpp` | Monster faction attitudes are reciprocal, inherited and overridable | ai | RUST-TEST | (a) hard-coded attitude matrix reflecting current JSON |
| `mongroup_test.cpp` | Monster group spawning respects event gating, nesting, pack size and quantity | ai | RUST-TEST | (a) statistical spawn probabilities over 10000 draws; (b) RNG seeding |
| `monster_attack_test.cpp` | Monster attacks, grabs, drags and thrown-weapon damage by range | ai | RUST-TEST | (a) tuned damage table {0,0,8.5,6.5,5,3.25}; (b) heavy map/cache resets |
| `monster_helpers.cpp` | No test cases: helper that advances a monster one turn of movement | tools | DROP | (d) helper module |
| `monster_test.cpp` | Monster speed, pathing moves, flag extension, broken-verify and egg/baby spawning | creatures | RUST-TEST | (a) exact move statistics; (b) needs clear_creatures and map |
| `monster_vision_test.cpp` | Monsters cannot see targets through a floor | ai | RUST-TEST | (b) needs map and midday calendar |
| `moon_test.cpp` | Moon phase cycles per synodic month, changes at noon and sets moonlight per phase | foundation | RUST-TEST | (a) exact phase light values (1.00/3.25/5.50/7.75/10.00) and 29.53-day month |
| `morale_test.cpp` | Player morale stacks, decays and is modified by traits, kills and temperature | creatures | RUST-TEST | (a) 119 numeric asserts on exact morale points |
| `move_cost_test.cpp` | run_cost triples when downed and varies with footwear, mutations and limb state | creatures | RUST-TEST | (a) 91 exact move-cost numbers |
| `mutation_test.cpp` | Mutation categories, strongest category, threshold breach, purification and instability | creatures | RUST-TEST | (a) 62 numeric asserts on tuned probabilities; (b) avatar fixture |

## Counts for the a-m range

| reuse | files | test cases |
|---|---|---|
| RUST-TEST | 103 | 715 |
| SESSION | 5 | 32 |
| INVARIANT | 10 | 22 |
| DROP | 12 | 14 |
| **total** | **130** | **783** |

| port area | files | reuse split |
|---|---|---|
| foundation | 23 | RUST-TEST:21, DROP:2 |
| content-loading | 3 | INVARIANT:2, RUST-TEST:1 |
| world | 12 | RUST-TEST:11, SESSION:1 |
| items | 30 | RUST-TEST:24, INVARIANT:6 |
| creatures | 29 | RUST-TEST:27, INVARIANT:2 |
| activities | 6 | SESSION:3, RUST-TEST:2, DROP:1 |
| ai | 5 | RUST-TEST:5 |
| vehicles | 1 | RUST-TEST:1 |
| worldgen | 3 | RUST-TEST:3 |
| spells | 4 | RUST-TEST:4 |
| interface | 6 | RUST-TEST:4, SESSION:1, DROP:1 |
| tools | 8 | DROP:8 |

Cross-check: the reuse counts sum to 130 and the port-area counts sum to 130, matching the 130 files in scope. 7 files carry zero test cases (helpers/fixtures) and are all marked DROP; 3 files are benchmark-only or contain benchmark cases that should be skipped on port (`cata_utility_test.cpp`, `generic_factory_test.cpp`, `make_static_test.cpp`).

## Files I could not fully judge

None were left unclassified: all 130 files were read and given a row. Three judgements are weaker than the rest and are marked here rather than guessed at:

- `connect_rotate_test.cpp` - the whole file is inside `#if defined(TILES)`, so its behaviour only exists in the tiles build. I classified it DROP as rendering-specific; if the rewrite keeps a tiles layer, it becomes a SESSION candidate instead.
- `advanced_inventory_test.cpp` - it constructs the AIM UI object directly and calls `init`/`recalc_panes`, then checks pane contents. It is UI-driven, but whether the rewrite can drive the equivalent without a screen depends on how that screen is rebuilt.
- `effective_dps_test.cpp` - the expected values live in `data/mods/TEST_DATA/expected_dps_data`, not in the test file. I could not see the numbers themselves, only that a 1000-trial average is compared against them.

## Marked inferences

- The index's `ui` flag is true for 8 files in range. I treated it as a hint only and re-read each file: `gates_test.cpp` and `effective_dps_test.cpp` are flagged but do not need the interface (they use the sounds bus and `string_format`); `fold_string_test.cpp`, `iteminfo_test.cpp` and `char_biometrics_test.cpp` do reach into `output.h`/`display.h` text assembly.
- Row risk notes cite (a) exact tuned numbers, (b) hidden global state or a created world, (c) user interface, (d) helper/fixture rather than a test, as requested. Where a file has none of these, the note says what it does depend on instead.
