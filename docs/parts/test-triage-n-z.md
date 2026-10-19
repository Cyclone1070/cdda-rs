# Test triage, files n–z (Cataclysm-DDA @ 0.I-1, 7b2efa5cea38e4d4d97dd0e63b28b9148623da59)

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
