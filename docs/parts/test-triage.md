# Reference test triage (all 218 files)

Every test file in `tests/` and what happens to it in the port. Two halves were
read separately; each row comes from reading the file, and the per-file numbers
come from the machine-generated index ([test-triage-index.json](test-triage-index.json)).

## Totals

| Reuse | a–m files | n–z files | Total | Meaning |
|---|---|---|---|---|
| RUST-TEST | 103 | 66 | **169** | Re-express as a test in the new language |
| INVARIANT | 10 | 12 | **22** | Keep as a property or range check |
| SESSION | 5 | 2 | **7** | Only reproducible by driving the game with recorded input |
| DROP | 12 | 8 | **20** | Helpers, fixtures, suite meta-tests, or rendered-image-specific |
| **total** | **130** | **88** | **218** | 1,217 test cases, 85,073 lines |

The headline: **most of the reference suite is portable as ordinary tests.** Only
seven files genuinely need scripted sessions — and those are the interface-driven
ones (`act_build`, `advanced_inventory`, `clzones`, `crafting`, `firstaid`,
`npc_behavior_rules`, plus the files listed in the n–z half). Sessions are
therefore needed for *integration and interface*, not for basic behaviour.

## The exact-number problem

These files assert tuned values that a faithful rewrite can still legitimately
miss. Each needs a written tolerance decision rather than a blind rewrite:

| File | What is pinned |
|---|---|
| `archery_damage_test` | Kill / no-kill table per bow and animal pair |
| `effective_dps_test` | 1,000-trial damage-per-second against checked-in expected values |
| `explosion_balance_test` | Lethality brackets (0.99 / 0.95 / 0.40) and vehicle hit-point fractions |
| `ranged_balance_test` | Hit-rate and damage tables over million-draw distributions |
| `monster_attack_test` | Exact damage table per attack |
| `throwing_test` | Hit and damage per skill level |
| `water_movement_test` | Move cost compared with zero margin |
| `sun_test` | Exact light levels and clock strings |
| `rot_test` | Exact decay arithmetic |
| `reload_time_test` | Move budgets within ±5% |
| `vehicle_drag_test` | Speed to 0.01 mph |
| `weary_test` | Exact minute thresholds |
| `moon_test` | Phase light values |
| `addiction_test` | Sums with approximate margins over 100k iterations |
| `char_biometrics_test` | 156 numeric assertions |
| `recipe_test`, `vehicle_export_test` | Exact serialised JSON strings |

## Coverage gaps that only sessions can close

Named in the n–z half, and the reason the session corpus in
[09-testing-plan.md](../09-testing-plan.md) is weighted toward interaction:

dialogue-window conversation · ally order menu · driving by input · vehicle
refuel and charge menus · crafting interface including batch and interruption ·
construction menu · reading a book to completion · zone management and zone
activities · save and reload of a played world · overmap travel and special
entry · throw and aim targeting · monster behaviour as experienced by a player ·
medical play · sidebar feedback · language switching.

## Dropped files and why

Helpers and fixtures with no test cases (`activity_scheduling_helper`,
`options_helpers`, `player_helpers`, `stringmaker`, `test_main`,
`test_statistics`), suite meta-tests (`normalized_test_names`,
`map_test_case_test`), rendering-specific code (`connect_rotate`, compiled only
for the tiles build), and C++-specific constructs that have no counterpart
(`make_static`, `mdarray`, `force_load_game`, `overmap_noise`'s assertion-free
exporter, `translations_test`'s pointer-identity checks).

## Known limits

Eight files in the n–z half and three in the a–m half were read partially; the
unread regions are flagged in the per-file rows. No file was left unclassified.

---

## Per-file detail: names a–m


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
---

## Per-file detail: names n–z


Scope: 88 `.cpp` files in `tests/` starting n–z (33,838 lines, 434 cases, from `test-triage-index.json`; numbers not recounted). Every row comes from reading the file. Risk tags: **(a)** exact tuned numbers, **(b)** hidden global state / created world, **(c)** needs the user interface, **(d)** helper or fixture rather than a test.

| file | what behaviour it pins down | port area | reuse | risk note |
|---|---|---|---|---|
| name_test.cpp | Snippet-driven town/world/nick/given/family name expansion from name.json | content-loading | RUST-TEST | (b) reloads the global SNIPPET registry from tests/data; asserts exact literal names, so any data edit breaks it |
| nest_conditional_placement_test.cpp | Overmap nest conditional placement: unconditional, neighbours, joins, flags, flags_any, predecessors, z-check | worldgen | RUST-TEST | (b) builds a custom overmap at hard-coded (122,122) and loads a tinymap; depends on a TEST_DATA overmap special |
| new_character_test.cpp | Profession starting items fit storage for every trait subset and both genders; category mutations grant traits | content-loading | INVARIANT | (a) item counts come from content data; (b) global avatar + scenario; slow (all professions × trait subsets) |
| normalized_test_names_test.cpp | Meta: every Catch2 test-case name uses a safe character set | tools | DROP | (d) suite meta-test over Catch2's registry; only worth keeping as a lint rule |
| npc_attack_test.cpp | NPC attack selection (melee/gun/throw by range and rules) and exoskeleton auto-activation | ai | RUST-TEST | (b) needs map, creature tracker and faction manager; (a) rules flags and target choice, not numbers |
| npc_behavior_rules_test.cpp | Ally rules avoid_doors / close_doors / avoid_locks actually steer NPC pathing | ai | SESSION | (b) manual mapgen house + spawned vehicle; behaviour only visible after many accumulated NPC moves |
| npc_blacklist_test.cpp | Trader refuses a blacklisted item once the NPC var is set | creatures | RUST-TEST | (b) needs the test trader template; the var name is a hidden string key |
| npc_shop_cons_rates_test.cpp | Shopkeeper consumption-rate lookup: default, junk threshold, typeid, category, item group, condition | items | RUST-TEST | (a) asserts exact rates (2, 99, 10, 100, 50, 25, −1) baked into TEST_DATA |
| npc_shopkeeper_item_groups_test.cpp | Shopkeeper item groups: restock conditions and wants_to_sell for owned and worn items | creatures | RUST-TEST | (b) places the NPC on the global map; ownership and worn state decide the result |
| npc_talk_test.cpp | Dialogue engine: response generation, conditions, dynamic lines, vars, speaker effects, arithmetic | interface | RUST-TEST | (b) cleared map + NPC at (25,25) + faction manager; (a) exact response strings and response counts from TEST_DATA topics |
| npc_test.cpp | NPC on_load needs vs per-turn, snippet tags, boarding the player's vehicle, movement over acid, gun preference | ai | SESSION | (b) mutates global calendar::turn and uses g/map; (c) includes output.h; two cases are hidden `[.]` |
| options_helpers.cpp | Fixture: RAII option override plus weather/windspeed override | tools | DROP | (d) helper with no assertions; mutates global options and weather |
| options_test.cpp | World-difficulty slider writes the expected option value at each level | foundation | RUST-TEST | (a) exact slider values (100%/120%, 3.00, 0.75, night); depends on loaded options and slider JSON |
| overmap_noise_test.cpp | Exports forest/floodplain/lake noise layers as PGM images | worldgen | DROP | (d) file-writing diagnostic with no assertions, hidden `[.]`; a debug tool, not a spec |
| overmap_test.cpp | Scents, ot_match modes, mandatory special placement success, mutable placement, item enumeration, terrain coverage | worldgen | INVARIANT | (b) global overmap_buffer and full overmap generation; slow; coverage depends on world seed |
| player_activities_test.cpp | Safecracking/shearing/boltcut/hacksaw/oxytorch/prying times and start gates, efile ops, distraction interrupts | activities | RUST-TEST | (a) exact tuned durations (630/330/110 min, 17 s) and tool-quality gates; (b) map, avatar and spawned monsters |
| player_helpers.cpp | Fixture: clear_character/clear_avatar, arm_shooter, process_activity, spawn_npc, tool_with_ammo | tools | DROP | (d) helper; (b) its normalisation of global avatar state is assumed by most other tests |
| player_test.cpp | Body temperature converges on tuned per-clothing targets; sweating | creatures | INVARIANT | (a) exact target arrays (−12…101 F) are the current status quo; (b) 10k iterations on the global avatar; hidden `[.bodytemp]` |
| point_test.cpp | Rectangle/cuboid containment, overlap, shrink, string round-trip, closest_points_first order | foundation | RUST-TEST | (a) locale variant silently skips when en_US.UTF-8 is missing; pure geometry otherwise |
| projectile_test.cpp | A fast projectile penetrates a chain-link fence, a slow one stops | items | RUST-TEST | (b) builds a 3-tile area on the global map; (a) speed thresholds 1000 vs 10 |
| pulping_test.cpp | Corpse pulp time from butchering tool, skills and proficiencies | items | RUST-TEST | (a) expected pulp seconds live in TEST_DATA; (b) needs map/game to compute pulpability |
| ranged_balance_test.cpp | Shooter dispersion/accuracy vs range and skill, shotgun damage vs armour, targeting-graph and anatomy hit distributions | items | INVARIANT | (a) large tuned hit-rate/damage tables and 1e6-iteration distributions with statistical margins; (b) map + NPC shooter; slow |
| reading_test.cpp | Book identify/clear, fun morale, read speed by INT, refusal reasons, skill gain, ereader | activities | RUST-TEST | (a) exact move costs (6000/6180/6480…) and exact reason strings; (b) global avatar and time of day |
| recipe_test.cpp | Blueprint build-requirement auto-calc: time, skills, consolidated requirement JSON | content-loading | RUST-TEST | (a) compares exact serialised JSON and move times; brittle to key order and formatting |
| reload_magazine_test.cpp | Magazine reload from inventory: fill to capacity, partial fill, ammo mixing, leftover stack | items | RUST-TEST | (b) global avatar inventory; (a) exact capacities and counts (30, 5) |
| reload_option_test.cpp | reload_option quantity for revolver, magazine, ammo belt and canteen | items | RUST-TEST | (b) local avatar but content-defined items; RELOAD_ONE flag semantics |
| reload_time_test.cpp | Moves spent to fire and reload from backpack, pouch, belt, pocket and ground | items | RUST-TEST | (a) exact tuned move budgets (400/137/200/180…) checked to ±5%; (b) map plus pseudo debug monster |
| reloading_test.cpp | Reloading guns, magazines, liquids, speedloaders, gunmods, integral magazines, auto-reload | items | RUST-TEST | (b) drives g->reload_wielded and activities on the global avatar; assertions are weak "contents changed" checks |
| requirements_test.cpp | Requirement deduplication and alternative collapsing algebra | content-loading | RUST-TEST | (b) two cases read static requirement JSON ids; pure algebra otherwise |
| rewrite_vsnprintf_test.cpp | Rewrites %N$ positional printf specifiers to sequential ones | foundation | RUST-TEST | Compile guard excludes MSVC; the index flags a UI include but the test is pure string handling |
| rng_test.cpp | roll_remainder/x_in_y distributions within 5%, random_entry constness | foundation | INVARIANT | (a) statistical thresholds; x_in_y sweeps ~1e4 draw pairs (slow, flaky under a different RNG) |
| rot_test.cpp | Rot rate vs temperature, freezer and sealed suppression, rot-away threshold, hourly rotpoint curve | items | RUST-TEST | (a) exact tuned values (20364.67, 3600, 1117.672); (b) mutates global calendar::turn and weather temperature |
| safe_reference_test.cpp | safe_reference invalidation on destruction, assignment and copy | foundation | RUST-TEST | C++-only weak-reference idiom; in a rewrite it is either a language feature or a drop |
| shadowcasting_test.cpp | FOV visibility golden grids incl. 3D/2D, walls, runoffs, float/quad equivalence, performance | world | RUST-TEST | (a) golden visibility grids snapshot current behaviour; hidden performance cases; (b) allocates level_cache grids directly |
| simple_pathfinding_test.cpp | Greedy paths (line, U-bend) and overmap paths (bend, bridge) | world | RUST-TEST | Two cases need real overmap terrain; the rest is pure |
| skill_test.cpp | Skills rust roughly 1%/day after a grace day while knowledge stays | creatures | RUST-TEST | (a) only asserts the 1.5–2.0 band; (b) simulates three days second-by-second on global avatar/calendar |
| speed_description_test.cpp | Monster speed description selection by speed ratio; immobile and unknown ids | creatures | RUST-TEST | (a) thresholds are tuned content; (b) player speed ratio comes from the global avatar |
| start_date_test.cpp | Scenario custom game and cataclysm start dates; invalid dates clamp to cataclysm | world | RUST-TEST | (b) mutates global calendar::start_of_game/start_of_cataclysm (reset at the end); (a) assumes 91-day seasons |
| stats_tracker_test.cpp | Event counting and statistics, achievement and conduct firing, legacy stats save loading | foundation | RUST-TEST | (b) in-game cases use the global event bus, stats and achievements; the legacy JSON blob is a save-format fixture |
| stomach_contents_test.cpp | Starvation duration, vitamin processing and equilibrium, tapeworm halving, hunger onset | creatures | INVARIANT | (a) tuned kcal/vitamin bounds; (b) global calendar and avatar, minute-by-minute simulation (slow) |
| string_formatter_test.cpp | string_formatter matches printf across a wide specifier table; malformed uses throw | foundation | RUST-TEST | (a) hundreds of exact expected strings including locale-dependent output; needs a printf-compatible formatter in the rewrite |
| string_ids_test.cpp | string_id interning, equality, sorting, sizeof, benchmark | foundation | RUST-TEST | Interned-id pointer identity and `&id.str()` stability are implementation-specific; sizeof/benchmark cases hidden |
| string_test.cpp | Colour-tag stripping, trim_by_length on CJK/Cyrillic with debugmsg text, str_cat/str_append | interface | RUST-TEST | (a) exact trimmed strings and debugmsg wording; needs matching UTF-8 width rules |
| stringmaker.cpp | Catch2 StringMaker specialisations used in failure reporting | tools | DROP | (d) test-harness helper; no test cases |
| submap_load_test.cpp | Submap JSON load for terrain RLE, furniture, traps, radiation, items, fields, graffiti, cosmetics, spawns, vehicles, construction, computer, ter/furn/fd migration | content-loading | RUST-TEST | (b) save-format fixtures; (a) exact counters/ids plus one exact debugmsg string; migration cases encode save-compat promises |
| submap_test.cpp | Submap rotation moves corner and centre tiles correctly | world | RUST-TEST | Pure logic; low risk |
| sun_test.cpp | Solar cycle phases, sunlight/moonlight levels, sunrise/sunset per season, eternal day/night, sun movement over day and year | world | RUST-TEST | (a) exact tuned levels and time strings ("5:57:58 AM", 125.0f) tied to 91-day seasons and a Boston-like model; (b) global calendar/options |
| temp_crafting_inv_test.cpp | temp_crafting_inventory amounts, charges and quality lookup | items | RUST-TEST | (a) quality levels (30, 4, HAMMER 2) from test JSON; local object only |
| temperature_test.cpp | Item thermal constants, temperature change over time, heater-held items staying normal | items | RUST-TEST | (a) exact tuned constants (3.7, 2.15, 260 J/g, 323.15 K); (b) global weather temperature and calendar |
| test_main.cpp | Catch2 runner: game state init, mod and option selection, listeners and reporters | tools | DROP | (d) harness entry point; documents the global game bootstrap every other test silently assumes |
| test_statistics.cpp | BinomialMatcher helper for statistical assertions | tools | DROP | (d) helper; its tolerance model must be reimplemented in the new test framework |
| text_snippets_test.cpp | Snippet choice varies with seed; `<lt>`/`<gt>` escaping | content-loading | RUST-TEST | (a) weak statistical threshold (≥90% change) that any different RNG passes; (b) needs loaded snippet categories |
| throw_activation_test.cpp | A thrown acid bomb creates an acid field at the target | items | RUST-TEST | (b) needs the global map; the assertion is tautological (same call made twice) |
| throwing_test.cpp | Throw range, hit/damage vs skill and stats, kill-before-contact, throw cost independent of stack size | items | INVARIANT | (a) exact hit-rate and damage tables tuned to current balance; long RNG loops; one case simulates combat |
| tool_quality_test.cpp | get_quality: inherent, charged, nested and BOIL-requires-empty rules | items | RUST-TEST | (a) exact quality levels incl. the INT_MIN sentinel for a non-empty BOIL container; battery magazine plumbing |
| translation_system_test.cpp | MO file load/validate, manager translate, plural rules, benchmarks | foundation | RUST-TEST | (b) needs a LOCALIZE build and lang/mo files on disk; the pointer-overlap case only means something under ASan; hidden benchmarks |
| translations_test.cpp | Translation macros return input unchanged when untranslated and keep addresses stable | foundation | DROP | C++ macro, static-cache and pointer-identity semantics with no analogue in a rewrite |
| try_parse_integer_test.cpp | Integer parsing incl. sign/whitespace, overflow rejection, exact error text, locale variants | foundation | RUST-TEST | (a) exact error strings; locale availability varies by machine |
| uncraft_test.cpp | Disassembly yield vs recipe difficulty and skill; damage reduces yield | activities | INVARIANT | (a) expected yields with ±55–60 margins are RNG-derived; (b) avatar, map and time of day |
| units_test.cpp | Unit ratios and arithmetic, JSON parsing of energy/power/duration/angle, conversions, rounding, display strings | foundation | RUST-TEST | (a) exact display strings and rounding edges ("1001001 mJ"); mostly pure |
| unload_naked_test.cpp | Unloading a revolver with no inventory space drops the rounds (issue 29839) | items | RUST-TEST | (b) global avatar and map; only checks the inventory is empty, not floor items; needs two do_turn loops |
| unseal_and_spill_test.cpp | Combinatorial unsealing and spilling across ground, worn, wielded and vehicle cargo | items | RUST-TEST | (b) enumerated scenario machine over global map and vehicle; volume-accounting expectations; hard to port 1:1 |
| value_ptr_test.cpp | value_ptr copy, move and reset semantics | foundation | RUST-TEST | C++ ownership idiom; port as a value-semantics test or drop |
| vehicle_drag_test.cpp | Air, rolling and water drag coefficients plus safe and max speed per prototype | vehicles | RUST-TEST | (a) values tuned to 0.01 mph and tied to TEST_DATA; hidden generator cases; (b) builds test map and blinds the player |
| vehicle_efficiency_test.cpp | Fuel/electric efficiency over pavement and dirt, with stops and in reverse | vehicles | INVARIANT | (a) efficiency targets are tuned energy budgets; long simulations; hidden generator cases |
| vehicle_export_test.cpp | A vehicle exported to prototype JSON reloads to identical parts, items and zones | vehicles | RUST-TEST | (a) compares one exact JSON string — formatting-brittle; (b) needs map, owner and zones |
| vehicle_fake_part_test.cpp | Fake parts activate on place and turn, weight stays constant, collisions, opacity, doors | vehicles | RUST-TEST | (a) exact part counts and per-angle active-fake vectors (120/18, {0,2,5,15,7,2}); (b) map loop over angles |
| vehicle_interact_test.cpp | Repairing a part needs welder + charges + goggles + consumables (UPS variants) | vehicles | RUST-TEST | (b) crafting inventory plus a map vehicle; no tuned numbers |
| vehicle_part_test.cpp | Part copy-from damage reduction, BELTABLE/DOOR/category flag invariants, faucet cold-water morale, rig crafting, tank refill | vehicles | INVARIANT | Content-wide flag invariants break whenever a new part is added; (b) map and vehicle for the behaviour cases |
| vehicle_power_test.cpp | Cable power loss, solar output (instant and daily), reverse speed a quarter of forward | vehicles | RUST-TEST | (a) exact 1/4 ratio and solar values; (b) map plus weather options and a blind-player hack |
| vehicle_ramp_test.cpp | Ramp transitions up and down between z-levels at three offsets; vehicle levelling | vehicles | RUST-TEST | (b) builds multi-z terrain, boards the player, asserts per movement cycle; brittle to map geometry constants |
| vehicle_split_test.cpp | Destroying a frame splits a vehicle into pieces with disjoint tile sets | vehicles | RUST-TEST | (a) exact part counts (12/12/2/3, 38) derived from prototypes; (b) needs a tinymap; the "conjoined vehicles" case asserts nothing |
| vehicle_test.cpp | Detach unboards passengers, grabbed sections, folding damage transfer, cable stretch, bike rack, autopilot, enchantments, effects | vehicles | RUST-TEST | (a) exact hp/damage/weight percentages and autopilot distance 9; (b) map and avatar; long |
| vehicle_turrets_test.cpp | Every turret type installs, loads and fires | vehicles | INVARIANT | Data-driven over all TURRET parts so a new part can break it; RECYCLED ammo can misfire; explosions and avatar healing between sections |
| vision_test.cpp | Avatar/creature vision and light golden maps: daylight, indoors, walls, crouching, moncam, vehicle camera and mirrors, reciprocity | world | RUST-TEST | (a) expected per-tile light/visibility grids snapshot current behaviour; (b) map, vehicles, blindfold and weather setup |
| visitable_remove_test.cpp | Removing items (with liquids) from character, vehicle and ground visitables; binning-cache invalidation | items | RUST-TEST | (b) random walk to find a clear tile on the global map; the cache case is a C++ use-after-free regression |
| visitable_test.cpp | charges_of sums contained charges plus INFINITE_CHARGES items | items | RUST-TEST | INFINITE_CHARGES sentinel semantics must exist in the rewrite |
| visitable_zone_test.cpp | Zone containment for monsters across surface and underground buildings | world | RUST-TEST | (b) rewrites terrain across the whole map then spawns monsters; slow and global |
| water_movement_test.cpp | Diving and swim move cost/distance vs stats, skill, gear and traits | creatures | RUST-TEST | (a) cost compared with margin 0 against a tuned table (114–400 moves) — a rewrite may legitimately differ; hidden exporter cases |
| weakpoint_test.cpp | Weakpoint hit chance and damage, set ordering, copy-from inheritance, deferred loading, proficiency practice | creatures | RUST-TEST | (a) statistical hit probabilities with tight epsilons over 1e5–1e6 iterations; content JSON sets |
| weary_test.cpp | Weariness level transitions across scheduled light, moderate and heavy activity days | creatures | RUST-TEST | (a) exact transition minutes (165/295/390/370…); (b) global avatar plus a scheduling helper that lives outside this range |
| weather_test.cpp | Weather realism bands vs NOAA data, eternal season, wind-chill formulas | world | INVARIANT | (a) bands (5.5–7.5 F day-to-day change, 7.5–10% precip hours) tuned to real climate; year-long sim × 3 seeds (slow) |
| weighted_list_test.cpp | Weighted list add/remove/replace and pick distribution | foundation | RUST-TEST | Distribution cases are statistical; the rest is pure |
| widget_test.cpp | Widget value, layout, colour, graph, compass and overmap text rendering, mostly golden strings | interface | RUST-TEST | (c) UI: some sections need a curses window and screen scraping under TILES; (a) exact rendered strings incl. colours; needs avatar, weather, overmap |
| wield_test.cpp | Wield cost from inventory vs ground, and retrieval from nested containers | items | RUST-TEST | (a) exact move costs (105/300/325) with a documented 1-move platform difference; (b) map |
| worldfactory_test.cpp | World creation timestamp is 20 digits | foundation | RUST-TEST | (a) length asserted via sizeof of a literal; trivial but formatting-locked |
| zones_custom_test.cpp | Custom LOOT zones with item/group filters, overlapping zones and nested containers | world | RUST-TEST | (b) needs map and faction globals; zone filter semantics are content-driven |
| zzip_test.cpp | zzip add/read/delete/compact and corruption recovery; deterministic compaction | foundation | RUST-TEST | Hidden `[.]`; uses rand() bytes yet asserts compaction determinism; the archive format is a save-compat promise |

## Counts for this range

Reuse (88 rows): RUST-TEST 66 · INVARIANT 12 · SESSION 2 · DROP 8.

Port area: items 19 · foundation 16 · vehicles 11 · creatures 9 · world 9 · content-loading 6 · tools 6 · ai 3 · activities 3 · interface 3 · worldgen 3 · spells 0.

## Coverage gaps: player-visible behaviour with no test in this range

- Talking to an NPC through the dialogue window (choosing responses, topic lists, mission offers) — only the engine is tested.
- Ordering allies around in play: follow, guard, stay, flee, retreat from combat, and the order menu itself.
- Driving by input: accelerate, steer, brake, collide with terrain and creatures, headlights, cruise control, board and exit.
- Refuelling and charging a vehicle through its menu (pump, siphon, battery swap) and watching solar charge over days.
- Crafting through the crafting UI: recipe search and filtering, batch crafting, interruption then resume.
- Player construction from the build menu, including resuming a partial construction.
- Reading a book as a player activity to completion, with interruption, darkness and ereader battery handling.
- Zone management by the player: create, edit, delete zones and then run zone activities (loot sorting, farm plots).
- Saving and reloading a played world (character, map, vehicles, submaps, achievements) and continuing play.
- Overmap travel: auto-travel to a destination and discovering/entering overmap specials.
- Throwing and aiming through the interface: entering targeting mode, aiming, reading hit chance, throwing a stack.
- Monster behaviour as felt by the player: pathing, pulping a corpse from the action menu, acid and fire fields on movement.
- Medical play: bandaging, bleeding, infection treatment and recovery messages over days.
- In-game feedback loops: weariness, hunger, temperature warnings and skill-rust notices shown while playing.
- Language switching in options and seeing the interface and messages refresh.

## Files I could not fully judge (judged from partial reads, not guesses)

- widget_test.cpp (2,993 lines, 34 cases): read ~250 lines plus the case list; individual golden strings are inferred from case names, not each verified.
- npc_talk_test.cpp (1,552 lines): read 8 of 35 cases; the arithmetic, tag and item-mutator cases were judged from names and neighbouring assertions.
- player_activities_test.cpp: read 6 of 8 cases in full; the two remaining cases were judged from their headers and sections.
- submap_load_test.cpp: the fixture JSON blobs (roughly lines 60–900) were not read; rows rest on the loading assertions.
- unseal_and_spill_test.cpp: the scenario-enumeration machinery was only partly read, so I cannot state how many combinations are covered.
- ranged_balance_test.cpp: the statistical helper thresholds (firing_test / Threshold) were only partly read; the pass margins are unverified.
- stats_tracker_test.cpp: achievement and score internals (roughly lines 211–570) were partly read.
- shadowcasting_test.cpp: the helper and reference-implementation region (roughly lines 24–640) was skimmed rather than read line by line.
