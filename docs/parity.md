# Parity ledger

The single place where port status lives. One row per system: what it is, which
phase it belongs to, and the gate that proves it matches the reference. Update it
in the same change that ports the system.

Legend for **Status**: `not started` · `porting` · `ported` (gates red) ·
`gated` (gate green, evidence linked) · `declared divergence` (differs on purpose,
reason recorded in [04-validation.md](04-validation.md)).

Nothing is `gated` without a link to the run that proved it.

---

## Single-authority audit (round 294)

The field container was not an isolated slip, so the same question was asked of everything the port has built since:

| Question | Answer |
|---|---|
Does `Level` hold anything the store owns? | nothing but terrain and two caches - and the caches say in their own docs that they are answers, not authority |
Is terrain readable from two places? | no: there is no `Map` any more. A loaded level is entities - one square, one piece of ground, one piece of furniture per square - and every reader asks the store |
Do two functions walk fields? | there were **two** - `process_fields` aged and despawned, `process_field_turns` did the same plus processors - so the first is now a call to the second with no processors registered, rather than a second implementation that would drift |
Dead state from the deleted container? | one leftover struct, `FieldProcessing`, which nothing read; removed |

The audit is recorded rather than assumed because the failure it looks for is silent by construction: a second authority passes every
test that does not compare the two, and nothing compares them unless someone writes the comparison.

## A receipt error worth naming (round 293)

For several rounds the round receipts quoted "1,2xx tests" from `tools/test_inventory.py` as if they were the port's own count. They
are not: that script inventories the **reference's** C++ suite - 206 files and 1,217 upstream cases - and the port's figure is what
`cargo test --workspace` prints. The two numbers happened to be close, which is exactly why the mistake survived: a receipt that is
merely plausible is not a receipt.

Correct as of round 293: **1,141 Rust tests pass, 1 is ignored, 0 fail**. The inventory page now says in its own header what it
counts, so the next receipt cannot borrow its numbers.

## P0 — Foundation

| System | Status | Gate | Notes |
|---|---|---|---|
| Coordinates, origins and scales | done (P0 scope) | M0 | 13 tests. Projection floors toward negative infinity (`coordinates.h:601-644`) and the vertical axis is never scaled. The origin/scale legality matrix is in the type system: six origins, four legal `project_remain` targets, `combine` only where the fine origin is the one the coarse scale names, and a segment that divides terrain tiles but not overmaps. Distances, midpoint and hashing match `coordinate_test.cpp` |
| Units and quantities | done (P0 scope) | M0 | 23 tests; 83 of the reference's 202 assertions reproduced, the rest being time/JSON/option-driven conversions that belong to later tasks. Rounding truncation rules pinned from the source |
| Calendar and time | done (P0 scope) | M0 | 28 tests = every assertion in `tests/calendar_test.cpp`; sun and moonlight cross-checked against the reference's own formulas over 309 configurations (max deviation 2e-16 rad) |
| Random numbers | done (P0 scope) | M0 | Engine stream matches the reference draw for draw over 4 seeds ([corpus/rng/engine.csv](../corpus/rng/engine.csv)). The wrappers the game actually calls — `rng`, `rng_float`, `normal_roll`, `rng_normal`, `rng_bits`, `one_in`, `x_in_y`, `dice`, `roll_remainder`, `random_entry`, `rng_sequence`, the exponential rolls and the world-seed hash — reproduce 93 calls bit for bit against the same libc++ ([corpus/rng/wrappers.csv](../corpus/rng/wrappers.csv), generator recorded beside it). `normal_roll_chance` is ported and held to 4e-8 rather than to equality, because the error function is the platform's; the centre case is exact. See [03-decisions.md](03-decisions.md). Not ported: the chi-squared roll, which needs libc++'s gamma sampler - and it is not dead code, it drives NPC mutation timing (`src/npc.cpp:663`), so that path cannot be claimed until it lands |
| Identifiers (string, int, type) | done (P0 scope) | M0 | 17 tests, 88 assertions: interned string identity, integer ids as ordinal caches with version invalidation, invalid use returns the dummy rather than panicking; misuse of an int id without its registry fails to compile |
| Parse cache | done (P0 scope) | M0 | 5 tests: a hit avoids the parse, a changed source is a miss, a damaged entry is a miss and never an error, unused entries are pruned at open. All 6,220 files read identically cold and warm; cold 11.0 s, warm 3.5 s |
| Registry machinery | done (P1-T1 core) | M1 | Abstracts kept apart but usable as bases, deferred `copy-from` retried at finalisation, cycles reported as unresolved ids with no recursion, duplicate ids from one source rejected (as the reference does), an override from another source replacing in place. All 8,743 `copy-from` chains in the core content resolve except 25 overmap-terrain objects whose base the map generator creates |
| Terrain and furniture | first family done | M1 | All 1,769 prototypes match the reference on name, symbol per season, colour, move cost, light, coverage and the full flag set — including inheritance, `flags` replacement, `extend`/`delete`, and both null prototypes |
| Traps | done | M1 | All 125 match on name, glyph, colour, visibility, avoidance, difficulty, radius, benign-ness, invisibility and flag set |
| Materials | done | M1 | All 193 match on name, density, chip resistance, breathability rating and the three heat constants |
| Field types | done | M1 | All 145 match, including the rule that a level inherits the level before it inside the same object |
| Item types: identity, measure, category, flags, names | done (first slice) | M1 | All 9,912 match on id set, name, plural, glyph, colour, weight, volume, price, category, flags and materials — including armour that derives its materials from its per-part entries, and materials added or dropped through `extend` and `delete`. No outstanding notes |
| Monster types | done (first slice) | M1 | 1,154 monster types match on id set, name, glyph, colour, body type, hp, speed, difficulty, melee skill, dodge, morale, aggression, weight, volume, species, categories and materials, including the null type and the dormant twins the reference generates |
| Generated prototypes | in progress | M1 | the ids the reference builds for itself are reproduced: the null monster and its dormant twins, 311 inline enchantments, 113 martial-art buffs, 239 turret parts |
| Content registry | done, 90 of 90 registries | M1 | 90 of 90 registries hold exactly the reference's ids, and **no invented ids anywhere**. The gate runs on every build and names any gap with its exact shortfall, so a regression in any of the 90 fails it and finishing a gap means editing the list |
| JSON reading | done (P0 scope) | M0 | Reader matches the reference on **all 6,220** data files — accept or reject in agreement, and the same structural digest; duplicate keys are a hard error, member order is sorted, trailing content is ignored. A further run parses the whole 107 MB tree in one go |
| JSON writing | done (P0 scope) | M0 | Writer matches the reference byte for byte on **all 6,220** data files, compact and pretty, including its fixed 6-decimal float format. Raised from a 127-file sample: the shapes a sample skips are where the escaping and rounding rules bite |
| JSON parse cache | done (P0 scope) | M0 | 5 tests: a hit avoids the parse, a changed source is a miss, a damaged entry is a miss and never an error, unused entries are pruned at open; plus a corpus run over the reference's own cache directory |
| Options table | done (P0 scope) | M0 | 21 tests, 276 options, all compared with the reference's own table: name, page, type, range, default, the four spellings a bool accepts, the percent-sign stripping on integers, the fallbacks for text that does not parse, migration of old names and values, and the saved-file format. Four options are broken in the reference itself (see 03-decisions.md) |
| Paths and filesystem roots | done (P0 scope) | M0 | Seven logical roots, 12 tests over the platform x build-option matrix; the trailing-separator bug that bit us earlier is now a pinned behaviour |
| Text lookup and formatting | done (P0 scope) | M0 | Formatter agrees with all 339 reference format rows and its error cases; 71/71 reference string assertions; 11,058 message ids extracted and the lookup returns each unchanged with no catalogue. Byte/column width and C-locale quirks recorded rather than fixed (B4) |
| Colours | done (P0 scope) | M0 | 23 tests: packed attribute layout, the 145-name table, pair table, name parsing, highlights and inversion. RGB exists only in the graphics build and is ported from there |
| Containers and hashing | done (P0 scope) | M0 | 26 tests: stable-address pools that keep an id pointing at the right slot when a neighbour is removed, dense bit sets over a fixed universe, and the hashing the reference relies on for its identifier caches |
| Diagnostics and debug levels | done (P0 scope) | M0 | 10 tests: level and class filtering, log formatting, the error-observed flag, prompt as a caller decision, assertion policy |

## P1 — Content and persistence

| System | Status | Gate | Notes |
|---|---|---|---|
| Prototype registry machinery | done (P1-T1 core) | Prototype digest (M1) | Load, inherit, finalise: abstracts kept apart but usable as bases, `copy-from` retried at finalisation, cycles reported rather than recursed, a same-source duplicate rejected while a cross-source override replaces in place, the factories' null prototypes created. 9 tests ([factory.rs](../crates/content/tests/content_suite/factory.rs), [rules.rs](../crates/content/tests/content_suite/rules.rs)) |
| Content type loaders (160 types) | in progress, 7 families | M1 | Terrain and furniture, traps, materials, field types, item types, monster types, plus the generated-prototype families. Each is held to a field-by-field corpus dumped by the reference. See [06-inventory.md](06-inventory.md) for the rest |
| Inheritance and merge operators | done | M1 | `copy-from`, `extend`, `delete`, `relative` and `proportional`, with the two orders the reference uses: readers check proportional before relative, `assign` checks relative first. All 8,743 `copy-from` chains in the core content resolve. [inheritance_corpus.rs](../crates/content/tests/content_suite/inheritance_corpus.rs) |
| Mod discovery and load order | **done: discovery, the graph and the order** | M1 | `cdda_sim`'s sibling `cdda_content::mods` reads every `data/mods/*/modinfo.json` and builds the dependency graph, gated on `corpus/mods.json` - the reference's own list of every mod it found, the dependencies each declares and the set it resolves them to. Discovery agrees on every mod, and the resolution agrees mod by mod (`content_suite/mod_graph.rs`, commit `d70bbbd`). **Not ported, declared:** the user-ordered list a world stores, which needs saves |
| Mod dependency graph and cycles | **done: order and the cycle refusal** | M1 | `mods::dependency_order` puts dependencies before dependants and refuses a cycle by naming it; `mods::resolve_all` answers the closure for every mod, and the corpus gate compares all of them against what the reference resolved |
| The sun's internals | **done: 147 samples, five of the six numbers exact to a part in ten million** | P2 | `cdda_core_types::calendar::sun_internals` answers the six numbers a sun position is built from - right ascension, declination, sidereal time, hour angle, days since the epoch and latitude - because `harness_sun_internals` (`calendar.cpp:192`, `CATA_HARNESS` only) can dump the same six. `corpus/env/sun-internals.json` is that dump over 147 samples: every hundred turns across the first four hours of the first day, plus the reference's own sunrise and sunset. **All 147 agree**: right ascension, declination and days to 5e-10, sidereal time and hour angle to 2.4e-6 degrees - the port's own single-precision vector rotations, which is 0.009 arc seconds. The tolerance is set at 1e-4 degrees so a wrong rule cannot hide behind it. |
| The dice probe | **gated: the avatar draws nothing, and the numbers are the engine's** | M0 | `corpus/rng-probe.json` sits at the root of the corpus and was read by nobody - a comment mentioned the dump and stopped there. It pins two facts and both are now tests (`core_types_suite/rng_probe.rs`): building the avatar draws from the engine **not at all** (the `before_avatar` and `after_avatar` halves are identical, three seeds), and each seed's three `rng( 1, 100 )` draws match the port's engine exactly. Two artifacts of the probe are recorded rather than smoothed over: the bounds are `1..=100`, so an off-by-one shows as three wrong numbers per seed; and the printed list is **reversed**, because the harness prints three draws as arguments of one `fprintf` (`harness.cpp:2264`) and gcc evaluates those right to left. The test compares against the reversed list and says why |
| The environment dump | **gated: the clock and the sun; the engine's position is declared** | M2 | `corpus/env/env-seed12345.json` version two, regenerated after the calendar fix, and `core_types_suite/environment.rs` now compares it: **sunrise 21478 and sunset 65767 agree exactly** with the port, and so do the season, the day of the season and the hour. Version one is kept in the tree's history as the example of the failure this gate was built to catch: taken before `calendar::set_season_length`, its sunrise was 18056 - the port's 21478 was right and the dump was measuring an unconfigured object. **Not compared, declared:** `rng_next`, because the harness draws from a copy of the engine **after** the reference's startup, so those four numbers pin a position the port cannot reach without replaying startup itself; the gate asserts the dump's version instead of pretending to check them |
| Creatures: the entity itself | **first slice done** - a creature is an entity with a place, a name, a size and its own action points | P4 | `crates/sim/src/creature.rs`. The reference's `Creature` (`src/creature.h:47`) with its `monster`/`npc`/`avatar` subclasses is a hierarchy held in parallel containers; the port takes only what its rules read. A creature is an entity carrying `Position` (shared with everything else that has a place), `Creature { name, size }` and `Moves { left }`, spawned by `spawn_creature` and found by `creature_at`, which returns a **list** because two dogs in a doorway is an ordinary state. Gated by `sim_suite/creatures.rs`: the creature is found where it stands, the index built from the store agrees, action points are one creature's and not its neighbour's, and two creatures share a square without sharing a name. Speed and the banking of action points are in too: `Speed { base, bonus }`, `speed_of`, `bank_a_turn` and `scaled_speed`, the last transcribed from `monstergenerator.cpp:346` where the `MONSTER_SPEED` option multiplies an `int`, so 130 at 75% is 97 and not 98 - a truncation the test pins. **A creature moves**: `try_step` spends the step's cost and updates both its `Position` and the index, `StepOutcome` says whether it moved or was too slow (and a creature too slow pays nothing), and `run_cost` is `Character::run_cost` (`character.cpp:10363`) transcribed - the diagonal divides by the square root of two and multiplies back before truncating, so 100 comes back as 99 and 400 as 399, exactly as upstream. Moving also turned up a real gap: the index had no way to take a thing out of a square, so a moved creature stood on two squares until `PositionIndex::remove` was added. **What a step costs is settled**: `combined_move_cost` is `map::combined_movecost` (`map.cpp:2466`) transcribed - both squares charged for, the sum scaled by `{0, 50, 71, 100}` for the number of differing axes and halved, so floor to floor is 100 moves, a diagonal 142, and a level change 200, all in integers. That answers the unit question the previous round left open: terrain JSON's `move_cost: 2` is a hundredth of a turn per point, and 71 is the reference's integer stand-in for a hundred over the square root of two - which is why `run_cost`'s divide-and-multiply can cancel the root afterwards. **Pathfinding exists**: `crates/sim/src/pathfinding.rs` is `pf::greedy_path` (`simple_pathfinding.cpp:48-135`) - greedy best-first over a rectangle, scored by a caller-supplied function that may refuse a square, returning the path **from the destination back to the source** with each node pointing at the next one towards the start, exactly as upstream builds it. Gated by two cases transcribed from the reference's own test file (`oracle/src/tests/simple_pathfinding_test.cpp`): the four-node straight line and the seven-node U-bend around a wall. The search's `closed`/`open`/`came_from` arrays are scratch inside the call, discarded on return - not a per-square array beside the truth. **A route across real squares**: `crates/sim/src/routing.rs` builds the search's scorer from the world - a square's terrain code, that code's `move_cost` from the loaded table, and `combined_move_cost` to turn two of them into a step. The table learned to hold move cost for exactly this (`TerrainTable::insert_move_cost` / `move_cost`, from `ter_t::movecost`, `mapdata.cpp:1225`, which is never scaled). `square_cost` returns `None` for a square nobody described rather than a guessed number, and an impassable one is refused instead of charged for. Gated by `sim_suite/routing.rs`: a route around a wall matches the shape of the reference's own u-bend, a wall with no way round ends the search empty, and the scorer's numbers are `combined_move_cost`'s. **How a prototype becomes a code is now one place**: `TerrainTable::from_map_data` assigns the codes and carries across transparency, emitted light, the glyph and `move_cost`, handing back the id-to-code map so a square and a route cannot disagree. **The reference's own data loads, gated:** `corpus/mapdata.json` is a new `--dump-mapdata` probe that writes all 1,769 terrain and furniture prototypes in the shape `map_data_common_t::load` reads - `type` not `kind`, `symbol` as an escaped string not code points - and `sim_suite/routing.rs` loads it, assigns codes and checks `t_floor` at 2 and `t_wall` at 0. Three probe bugs were found and fixed getting it: the writer sat before the data was loaded (it dumped `[]`), the symbols were unescaped (a railroad tie's backslash made the file invalid JSON), and the dump had to run beside `--dump-terrain` rather than early. Every one of them was a dump measuring something that did not exist yet - the same failure the sun-internals probe had. **A creature walks a route it asked the world for**: `routing::walk_towards` reads the creature's place, asks `route` for the way, steps to the square before the path's own start (the path runs destination-first, so the source is last and has no direction), pays `combined_move_cost` for the step and returns `Walk::Stepped` / `Arrived` / `TooSlow` / `NoRoute`. The creature keeps no path: it asks again every step, which is the reference's own shape (`monster::move` re-runs its search) and means a door closing needs no invalidation. Gated by `sim_suite/walking.rs` over a **loaded level**: a two-square corridor walk costs exactly two hundred moves and lands on the target, fifty points cannot pay for a hundred-move step and the creature stays put, and a wall across the corridor is `NoRoute` rather than a detour. **Creatures act inside the turn**: the schedule has a fifth stage, `Creatures`, placed where the reference puts `monmove` - after the fields have burned and before the light is recomputed - and `creature::take_turns` banks every creature's speed and walks each one with a `Goal` towards it, dropping the goal on arrival or when no route exists. The stage order is written out by hand in `turn_schedule.rs`, so a reordering has to break a test. Gated by `sim_suite/walking.rs`: a creature with speed 100 and a goal two squares away stands one square on after one `run_turn` and on the target after the second. **Whether an attack connects is settled, damage is not**: `crates/sim/src/melee.rs` holds the three numbers `Creature::deal_melee_attack` (`creature.cpp:903-929`) composes - the dodge roll (`monster.cpp:2717`, five per point), the size penalty (30/15/0/-10/-20, `creature.cpp:836`), and the forty added for a target that cannot move - plus `is_a_hit` / `dodged`, which disagree **at exactly zero on purpose**: `monster::melee_attack` deals damage on `hitspread >= 0` (`monster.cpp:2151`) while `Creature::deal_melee_attack` fires the dodge event on `hit_spread <= 0` (`creature.cpp:924`), one line of the reference contradicting the other at a single point. The port keeps both questions separate rather than tidying them into one, and the test names the two line numbers. An earlier round of this port asserted zero was a miss - that was wrong, and the correction is the receipt. `melee::melee_hit_range` is ported too, and gated against the reference's own dice: `corpus/melee-rolls.json` is `--dump-melee-rolls`, three seeds by three accuracies with four rolls each, and all **36** match the port to a thousandth. That pins both numbers at once - a wrong mean moves every value and a wrong deviation changes their spread - which a single "it hit" assertion could never do. **Damage begins**: `melee::dice_total` is `dice` (`rng.cpp:211-218`, a separate `rng(1, sides)` per die - a single flat roll would give the same range and a different distribution), and `monster_melee_damage` is `monster::melee_attack`'s `2143-2146`: the type's fixed damage plus its dice, the dice only when the type has any, carrying the type's armour penetration. **Armour subtracts**: `effective_resist` is `resistances::get_effective_resist` (`damage.cpp:724-727`), `max( resist - penetration, 0 )`, and `after_armour` is `monster::absorb_hit`'s `monster.cpp:2085` line - `amount -= min( effective_resist + worn_armour, amount )` - so a blow can be stopped entirely and never pushed below zero into healing the target. Both clamps are tested in both directions, because each is a place where a plausible "cleaner" version differs. **A whole swing**: `melee::resolve` composes the roll, the spread, the dice and the armour into what got through, and `melee::strike` is the same rule with the engine's dice - split so the rule can be tested with a stated roll instead of a lucky one, and so a miss still spends its damage roll as the reference does (`monster.cpp:2143-2151`). Creatures carry `Health { hp, max_hp }` with `hurt`/`health_of`, following the reference's int subtraction and `monster::is_dead_state`'s `hp <= 0`. The dead are cleared at the **start** of the creature stage (`creature::reap_dead_creatures`), so a creature killed this turn does not also walk, and the reap removes its index entry by hand - dropping only the entity would leave a square claiming an occupant, which is the ghost the architecture gate exists to catch. Death leaves a body when content says what the creature was: `CreatureKind(MonsterKindId)` is optional, and `reap_dead_creatures_as_corpses` drops the same `CorpseOf` item the item layer already measures, so a corpse is an item on a square rather than a creature the world keeps. A creature with no kind leaves nothing - a survivor the port invented has no prototype, and inventing one would put a number in the world that content never gave. A creature with **no** `Health` is alive rather than dead: the reference gives every monster hit points from its type and that number is not carried across yet, so "untracked" and "at zero" are kept apart instead of collapsing into one default. **A monster type carries what an attack needs**: `--dump-monsters` version two adds `melee_dice`, `melee_sides`, `melee_dice_ap`, `armor_bash` and `armor_cut`, and `MonsterType` reads them - a zombie is 80 hit points at speed 70, two dice of three faces, no penetration, no armour, all from the reference's own generator. `monsters::load_objects` reads a dump-shaped list (no `type` field to filter on) and derives size from volume afterwards, as the reference does. **One description of a creature**: the older `Monster { kind, hp, moves }` component is **deleted** - every system that read it now reads `Creature`, `Health`, `Moves` and `CreatureKind`, so a creature is one set of components rather than two views that could drift. `spawn_monster` is kept as the "ready to act" door (it grants a turn's moves, where `spawn_creature` is the reference's constructor and leaves them at zero), `reap_dead_monsters` now runs the creature module's own sweep, and `occupied` reads health from the store. Deleting it also removed a real bug: the old damage path despawned a creature without clearing its index entry, which is a ghost.  **A monster is placed by its type**: `creature::spawn_monster_of_type` gives a creature its hit points, speed (scaled by the `MONSTER_SPEED` option, taken as a parameter because options are content), melee dice, accuracy and armour from `MonsterType` - `monster::monster( const mtype * )` plus `monster::get_speed()`. It spawns with **no goal**, because wanting something is a system's business and a monster that has just been placed has decided nothing. `size_named` maps the reference's five size names, and a name it does not know leaves the default rather than inventing a size. **The player's blow goes through the same rules**: `player_turn` used to take armour off a blow without asking whether it landed, so no target could dodge the avatar and a second set of combat numbers existed. Now the player's swing builds a `melee::Attack` (skill from `TurnRules::player_accuracy_tenths`, the reference taking it from the character's melee skill and weapon) and calls `melee::strike` with the target's `defence_of` - the same rule a monster's swing uses. Gated by a differential on one seed: a target with no dodge takes the blow, and a target with twenty points of dodge - a hundred points of roll - turns it aside entirely. **Dodge is carried, not assumed**: `creature::Dodge { points }` holds what `monster::get_dodge_base()` returns, `spawn_monster_of_type` fills it from the type, and `defence_of` reads it - so a dodging target is measurably harder to hit on the same seed rather than quietly as easy as a wall. A creature with no `Dodge` is not dodging at all, which is the reference's zero. **The player is a creature**: `Player` is now only the marker that says which entity the player is looking through, and the avatar carries the same `Creature`, `Health`, `Moves`, `Speed` and `Armour` as a monster - so `player_dead`, `strike_player` and the player's own turn read the components every other creature has, instead of a parallel struct with its own `hp`, `hp_max`, `moves` and `armour`. Reference: `avatar` is a `Character` and a `Character` is a `Creature`; the port's earlier `Player` struct was the one place that was not true. **A creature fights in its own stage**: `creature::take_turns` asks each creature with a `Goal` whether the goal square is in reach; if it is, it swings at whoever stands there (`monster::move_to` refuses an occupied square and `monster::attack` is what a monster does instead - the swing *is* the move), and otherwise it walks. The defence is read from the target's components: `defence_of` takes its size, its `Armour`, and its `Dodge` - the type's `sk_dodge` (`monster.cpp:2643-2646`), five points of roll per point, exactly cancelling a point of the attacker's skill, the roll comes from the turn's own dice, and a creature with no `MeleeAttack` cannot wound anyone however close it stands. Four tests drive `run_turn`, not `strike`, so what is pinned is that the behaviour is wired into the declared stage. **Where a blow lands**: `crates/sim/src/hit_location.rs` is `anatomy::select_body_part` (`anatomy.cpp:208-256`) over `weighted_float_list::pick_ent` (`weighted_list.h`) - a part is dropped when it is below `min_hit` or above `max_hit`, an attacker that cannot reach high loses the upper parts and **triples** the lower ones, and a positive roll raises each weight by `hit_roll ^ hit_difficulty`. The draw is a fraction rather than a die, so each test states the boundary it pins; two of the first assertions were written against the wrong side of the boundary and the code was right, which is the sort of mistake a boundary test exists to expose. A monster has no parts and never asks. **The body's numbers come from the reference**: `--dump-anatomy` writes every `body_part_type` with `hit_size`, `hit_difficulty` and the upper/lower flags, and `crates/content/src/body_parts.rs` loads them - a torso is 36 and no harder to aim at, a head 4 and 1.2 and out of a short attacker's reach, eyes half a unit, and the null part zero so it can never be picked. The dump was **empty on the first run** because it dispatched before the data load: the same trap the mapdata probe fell into, found the same way - by a count assertion rather than by trusting the file. Two flags were then read from the wrong field (the dump states `upper`/`lower` booleans, not the game's `LIMB_UPPER` flag list), which the gate caught as "a leg is marked low". **Which parts a body has**: `crates/content/src/anatomies.rs` reads the game's own `anatomy.json` (`"type": "anatomy"`, `id`, `parts`) - a person has twelve parts, the fallback has a torso and a head - and the gate crosses it with `--dump-anatomy`: **every part an anatomy lists is a part the reference built**. That is the shape the reference itself cannot give in one file, because `anatomy_factory` is file-local, and it is stronger than either source alone: one says which names appear, the other says which names exist. **A blow lands on a part, and the store remembers**: `creature::Body { parts }` holds a creature's parts, `creature::Wounds` holds the running total per part, and `strike_what_the_goal_holds` asks `hit_location::struck_part` - `Creature::deal_melee_hit`'s call (`creature.cpp:959`), `select_body_part( -1, -1, can_attack_high, hit_spread )` - only of a creature that has a body. The gate is the **join of the two corpus files**: the human anatomy's twelve parts from `anatomy-parts.json`, their sizes from `--dump-anatomy`, a blow through `run_turn`, and a wound recorded against a name that is one of those twelve. A creature with no body still takes the damage and records no part - "nothing to record against" is not the same as "the blow did not happen". **Armour is asked per part**: `creature::PartArmour` holds the resistance on each part and `armour_on` answers for the part a blow reached, falling back to the creature's whole-body `Armour` and then to nothing - a bare arm and an unstated arm are the same arm. The order the reference uses is now the order the port uses: the margin is known before the part (`creature.cpp:959`), and the part before what protects it (`Character::get_armor_type`, `character_armor.cpp:70-86`). Gated on **one seed, three worlds**: the bare blow, armour on the part that blow lands on (six through four is two), and armour on a *different* part (six, unchanged) - the second half is exactly what a whole-body armour number could never express. **The avatar has a body in the actual game**: `creature::body_named` joins an anatomy's membership to the numbers each part states - `anatomy::finalize`'s join (`anatomy.cpp:78-90`) - and answers `None` when the anatomy is unknown **or** lists a part nobody describes, because half a body makes every later rule quietly wrong. The headless binary now builds the avatar's body from the same two corpus files the gates use: `cdda-cli play` prints "the avatar has 12 parts of its body" and runs a turn. **A material's resistance is data now**: `Material` carries `bash_resist` and `cut_resist` from the `resist` object in `materials.json` - steel six of each, flesh one, per millimetre of thickness - which is the first factor of every protection in the game (`item::_resist`, `item.cpp:9199-9231`). Gated against the loaded data, not a literal: the first version of the test asserted flesh resisted **no** cut and the data said one; the data won. **A garment's protection is resistance times thickness, over the materials the hit meets**: `crates/content/src/armour.rs` is `item::_resist`'s sum (`item.cpp:9199-9231`) - a material counts only when the hit's roll beats its `covered_by_mat` coverage (**`<`, so the boundary belongs to the hit's loss**), and what it adds is `resist x thickness`, not resist alone. `covered_by_mat` (default 100, refused by the reference outside 1..=100) joined `ArmorPart`, and `layers_on` is the one place a garment's layers are worked out. Gated over **every garment in the data that covers a torso** - more than fifty of them - each one's total checked against its own layers, so a garment whose protection and whose data disagree cannot pass. **What you wear is what protects you, and it is derived**: `creature::WornBy` puts the link on the **item** and `creature::Garment` carries what content worked out; `creature::dress` runs at the start of the creature stage and writes every creature's `PartArmour` from what it is wearing, removing the component when nothing is. That is the port's own vocabulary applied to armour - a computed answer in a component, one writer, never saved - and it is why a vest taken off takes its armour with it: the test checks both that half a millimetre of steel is three points on the torso **and** that despawning the vest leaves no stale armour for a blow to read. **The avatar in the game is dressed**: `cdda_cli::dress_the_avatar` picks a garment from the game's own item data and gives it to the store as an item with a `WornBy` link, which is what `creature::dress` reads - so `cdda-cli play` prints "the avatar wears carpet_cuirass - 5 point(s) on the torso" and the armour in the game comes from the same `item::_resist` sum the gates check. The choice is a **demo heuristic, not a reference rule** (the reference dresses a character from its profession, and no profession is ported): the garment covering a torso and an arm whose protection is closest to five points, ties by id. Two failures found on the way are worth the record - "the first item with torso armour" is a canteen on a strap, and "the least protective garment" is a tactical jacket whose materials add up to zero, which would have looked dressed and been bare. Summing armour in **tenths** while the layers are added is also what lets two thin layers make a point between them, the way `Character::get_armor_type`'s single `int` conversion does. **Getting dressed on purpose**: `creature::wear` puts a garment on - refusing anything that says nothing about what it protects (`NotAWearable`) and anything already worn (`AlreadyWorn`) - and `creature::take_off` removes it, with `dress` re-deriving the armour both times. Reference: `Character::wear`'s two refusals, turned into an answer rather than a message, because a silent failure to put a vest on would be the worst kind of bug in a game about scavenging. The gate checks the failure that matters in a store: wearing the same vest twice leaves the protection at three, not six. **Picking something up**: `creature::pick_up` takes an item off the creature's own square and charges `handling_cost` - `Character::item_handling_cost`'s base hundred plus fifty moves a litre, **capped at two hundred for the volume term** (`character.cpp:7283-7290`), so four hundred litres is still three hundred. Success removes the item's `Position` and its index entry and adds `Carried(who)`: an item carried that kept a square would be on two surfaces at once, which is the ghost the architecture gate exists for. Refusals are answers (`NotHere`, `TooSlow`, `AlreadyCarried`), and a refusal costs nothing - the test checks the moves as well as the message. The reference's hand-encumbrance term is **not** here, because the port has no encumbrance and a term that is always zero would be a lie about work done. **The whole errand works, and it found a hole**: an end-to-end test - something on the floor, picked up, put on, a blow met by the difference - which passed, and in passing showed that nothing stopped a survivor **wearing a jacket through the floor**. `Character::wear` takes an inventory item, so `wear` now answers `NotInHand` for anything still on a square, and the errand test asserts that order rather than working around it: floor -> `NotInHand`, pick up -> `Worn` -> three points on the torso, floor empty. Rules that are right on their own can still be wired wrongly to each other, which is the failure a composition test exists to catch. **P5 begins: spells are data**: `--dump-spells` is a new oracle probe writing every `spell_type` (`magic.h:191-380`) - id, name, `effect_name`, `effect_str`, damage type, shape, energy source, and the five level-scaled triples (damage, range, area, energy, casting time) plus difficulty and maximum level. `crates/content/src/spells.rs` reads them through `Scaling`, whose `at(level)` is the reference's climb-and-clamp; **a number the data writes as an expression is `None`, not its mean**, because a loader that averaged a dice expression would be wrong in a way no test of the loader could show. 245 spells in `corpus/spells.json`; the gate counts how many carry plain damage numbers so the loader's blind spot cannot grow quietly. Two probe mistakes paid for: the flag was registered in the wrong shape (a `.case_()` chain against a block), and the dump path is resolved **on the server**, so a relative path wrote to `~/cve-test` and the copy-back found nothing. **A spell can be cast at a creature**: `spellcasting::cast_attack` is `spell::cast` -> `spell_effect::target_attack` -> `damage_target` for the `attack` effect - the spell reaches its level-scaled range (measured the way `rl_dist` measures, the larger axis, not the diagonal), deals its level-scaled damage, lands on one part of a body, and the armour **on that part** meets it through the same `through_armour` the melee path uses. So a vest stops a taser for the same reason it stops a fist, and the gates say so: fifteen on a bare torso, ten through a five-point vest, nothing at one square past its reach. A spell whose damage the loader would not read as a number **does nothing** rather than guessing - the cast has no number to use, and inventing one would be a silent divergence in every fight involving it. The hit-location fraction is a parameter, not a draw inside the rule: a rule that reached for its own randomness could not be tested at all. **Not started:** the other 244 spells' effects, armour breakage, weakpoints, what a monster does when it reaches its goal, NPC conversation | | Save reading and writing | not started | M1 | Scope depends on blocker B2 |
| Save migrations | not started | M1 | 14 content-level migration types plus per-field fix-ups |
| Compression and archives | not started | M1 | Dictionary content must match bit-for-bit |
| Overmap terrain ids | done | P1 | The `overmap terrain type` registry is exactly the distinct `type: overmap_terrain` ids in the load set - 6,269 measured against 6,269 reported. The `overmap terrain` registry is those types expanded into rotation and line-drawing variants; the shape is known and the rotation rule is proven exactly: `!NO_ROTATE && !LINEAR` with flags inherited through `copy-from` and `delete` applied last, giving 5,901 rotatable types with no disagreement against the dump, plus 256 line-drawing shapes for the 14 `LINEAR` types. The type registry is **exact**: 6,269 against 6,269. **24,218 of 24,218 exact**, with rotations, the sixteen line-drawing shapes per `LINEAR` type and plain types all matching. The last eight came down to flag semantics: a plain `flags` member *replaces* what inheritance supplied, `extend.flags` adds, `delete.flags` removes last. See [15-overmap-terrain-ids.md](15-overmap-terrain-ids.md) |
| Effect-on-conditions | done | P1 | **913 of 913 exact**, 692 defined and 221 inline. An inline effect is registered under the `id` inside the slot it sits in (`activated_eocs`, `true_eocs`, `result_eocs`, `eocs` and the rest, `effect_on_condition.cpp:119-133`). Collection stops at the slot boundary: a sound effect inside a dialogue effect also has an `id`, and collecting it invented seven effects that do not exist |
| Overmap specials | done | P1 | **742 of 742 exact.** `overmap_special` and `city_building` are one namespace, and the `FakeSpecial_<id>` ids the reference mints for single-tile buildings named by a region but defined by no special are generated from the `city` lists of `region_settings`. `overmap_special` and `city_building` are one namespace - the reference's `city_buildings::load` is an alias for `overmap_specials::load` - so both are read into one set: 707 of 742. What is missing is exactly the 35 `FakeSpecial_<oter>` ids the reference mints for single-tile city buildings a region names but no special defines. See [15-overmap-terrain-ids.md](15-overmap-terrain-ids.md) |
| Weighted lists | done (whole-number lists) | P1 | `WeightedIntList` reproduces `src/weighted_list.h`: a non-positive weight is refused, `add_or_replace` with zero deletes, a single entry short-circuits, and the draw is `(randi % total) + 1` against the running total. Picking from an empty list still costs one draw, because the reference evaluates `rng_bits()` before it checks the total - a port that returned early would put every later roll out of step. The fraction-weighted list scales the draw across the whole 32-bit range instead of wrapping it modulo the total, and has no one-entry shortcut, so it is ported as its own type rather than a flag |
| Test regime | in place | every gate | Corpus gates carry receipts (what was compared, how much, how long); failures name the id and field and count the differences by kind; the id gate asserts the exact size of each known gap rather than being skipped. Rules: [14-test-porting.md](14-test-porting.md); inventory: [13-test-inventory.md](13-test-inventory.md) |
| Oracle harness modes | done (record/replay/state/digest) | Gate 0 | Replays a recorded session with no terminal and reaches a byte-identical state; see [11-oracle-harness.md](11-oracle-harness.md) |
| Placing a map | **partial: glyphs into a level** | P9 | `MapgenObject::place` walks a map's rows and writes each square into a level at an origin, resolving symbols through the map's own terrain table and reporting the furniture that stands on a square. Two rules are the reference's and not the port's invention: **a symbol the map does not explain is a data error**, collected rather than silently becoming floor, and a furniture-only square is floor underneath. `terrain_id_of` is a caller's closure, because turning a content id into the level's number is the terrain loader's job. Five tests: a map written glyph by glyph, the origin moving it, an unexplained symbol reported **and left blank**, a furniture symbol that is not unknown, and an id the content does not know reported rather than guessed |
| Mapgen objects | **partial: the rows and their symbols** | P9 | `MapgenObject` and `load_objects` read the JSON method's `rows`, its `terrain` and `furniture` symbol tables, and its weight, and answer the questions a placer asks first: the map's size, whether the rows agree (the reference rejects ragged maps), and what a square resolves to. Four tests against hand-written objects. **The bad gate was the useful part.** Last round's corpus gate read zero maps and the loader "worked": the rows live under `object`, not beside `om_terrain`, and my hand-written fixture had them at the top level, so the test agreed with the port instead of with the data. Fixed, the loader finds **1,071 mapgen objects across 845 files**, and the fixture now uses the real shape with a companion test that a flat object is **not** a mapgen. Width is counted in **characters, not bytes** - a non-ASCII glyph made 200 sound maps look ragged - and exactly **three** maps in the reference tree really are ragged (`lighthouse_small_z1..z3`); the reference loads them anyway, so that is pinned as a fact rather than treated as a fault | 
| Frame diff | **machinery done, comparison pending** | P9 | `frames::read` and `frames::diff` are where both sides meet: the dump is read into squares and two frames are compared field by field - **glyph, terrain, item count and light** - with each difference named by square and by field, carrying both values. Light is compared to a hundredth because two renderers rounding the same value differently is not something a player could see, and would drown the real differences. Six tests: a frame against itself is empty, a changed glyph is named once rather than as a whole frame, each field is reported separately, a sub-hundredth light difference is **not** a difference, a missing square is reported as such, and the reference dump reads back identically twice. **Not yet done:** the comparison itself, which needs the port and the oracle to build the *same* map - the oracle can only dump the shelter it generated at world start, and asking it for a named map at a known rotation needs a dump built on `mapgendata` |
| Frames | **ported and gated: the live draw, the daylight it needs and the memory it fills** | P9 | `--dump-frame` writes what a player would **see**: for every square of a 41x41 view, the glyph the interface would draw (furniture over terrain, an item over that), the terrain id, the furniture id, how many items lie there, and the ambient light. `corpus/frames/start.json` is the reference's own view of the starting shelter - **1,681 squares**, the player on `t_floor` at light **4.0**. Four tests guard the dump itself while the renderer does not exist yet: the view is full, every square has a glyph and a terrain, no square appears twice, and the player stands on a lit floor. The port's half is `render_frame`: given a level, a centre and the light, it produces the same square list, **in reading order so the two can be compared row by row**. Precedence is the reference's - an item hides furniture, furniture hides the floor - because a diff is worthless if the two disagree about what covers what. Five tests: coverage, glyph precedence, an item covering a table, light passed through, and the reading order itself. **Not yet possible:** the actual diff, because the shelter the reference dumps comes from map generation, which the port does not have; the renderer is exercised against hand-built levels until P9 |
| Prototype digest | **done; 88 of 90 registries match (the other two hold nothing)** | P1 | Verified against the reference digest: `material`, `trap`, `ITEM`, `item_category`, `monster type`, `overmap terrain`, `overmap terrain type`, `overmap special`, `field types`, `terrain`, `furniture`, `vehicle`, `vehicle_part`, `profession`, `scenario`. The last eight matched first time, which is the useful part: the gate now covers ~38,000 prototypes and the only disagreement it has ever found was the terrain/furniture mix-up, so the loaders are in better shape than the count of untested registries suggested. 75 registries remain, most belonging to subsystems that are not ported yet (magic, martial arts, missions, NPC classes) | Registries now verified against the reference digest: `material`, `trap`, `ITEM` (9,912), `monster type` (1,154), `overmap terrain` (24,218), `overmap terrain type`, `field types` and `terrain` (1,109). Checking them found a real defect in the terrain loader: the dump carries terrains **and furniture** in one tagged list, and the port was loading both as terrains, so its "terrain" registry had 1,769 entries against the reference's 1,109. The loader now splits them by `kind` and exposes furniture separately. Eight registries of ninety remain to be wired |
| Prototype digest (start) | **done** | P1 | The harness's digest is FNV-1a over a registry's sorted, unique ids with a zero byte after each, which makes it a gate on the *id set* rather than a count. The port's implementation reproduces **all 90** digests in the artifact from the reference's own id lists, and - the part that matters - the port's **material** and **trap** id sets hash to exactly the reference's digests. That is the first time the port has claimed "same content" with a number instead of an eyeball. The remaining 88 registries need their loaders to expose id sets the same way; each is a one-line addition to that test |
| Mapgen keys | **done; B9 was a bug in my dump** | P1 | The mapgen registry is **fully loaded**: 24,191 of 24,218 overmap terrains have one. The "only 325 built-ins" result that blocked this family for several rounds came from looking keys up by the terrain's **own id** instead of its **mapgen id** - 325 is exactly how many terrains happen to match that way. The dump now keys by `oter.get_mapgen_id()` and the artifact holds **6,306 unique keys**, of which 26 are built in C++ (rivers, subways, forests, lake and ocean shores, ravines) and the rest come from 6,105 JSON mapgen objects. The loader scans the data for `om_terrain` written as a string, a list, or a grid of lists, plus `nested_mapgen_id` and `update_mapgen_id`. Three tests: every reference key is either found in the data or on the built-in list, the built-in list is exactly the 26 expected, and the four keys present in the data but not the registry are the ones the reference itself erases or uses only from C++ |
| Field prototypes | **done** | P1 | 145 field types read from the reference's dump: id, priority, and every intensity's glyph, colour, danger flag, transparency, move cost and light. Levels are the intensity ceiling, so "intensity 3" now means what the data says it means. Two facts the port had wrong in its head and now has from the data: **thin smoke is see-through and thick smoke is not** (`[true, false, false]`), which is why smoke dims a room instead of blanking it; and **smoke draws over fire** (priority 8 against 4) |
| Item state | not started | Item replay (M3a) | |

## P2 — World and environment

and its convention is the opposite of the obvious one: a **positive** shift makes each destination read from a **higher** coordinate,
so the contents travel toward lower ones, because the map moved and the scent stayed put in the world. The walk starts from the side
the data is moving towards, so a square is read before it is written over; walking the other way smears each value across the window,
which the round-trip test catches. **Not ported:** scent types, reading the terrain flags off the map (the caller marks squares), and
the layer that decides when to shift
 `Level::seen_at` answers "can the character see this square" over the cast the level already caches, which is the shape of `map::seen` reading `seen_cache` (`src/lightmap.cpp:1039-1100`). Three tests: an open square and a diagonal seen, **the wall seen but the square behind it not**, and nothing beyond the sight range. **Not ported, declared:** the reference's cache holds a per-square *transparency value* rather than a bool, is filled across all z-levels in one pass and can be cumulative between turns, and vision is also limited by light (`apparent_light`), which the port's cast does not consult yet - a lit and an unlit corridor are visible alike here |

| System | Status | Gate | Notes |
|---|---|---|---|
| Fields as things in the store | done (P2-T0c), single authority restored round 290 | P2 | `Field` is a component and a patch of smoke is an `Entity` with a square, a strength and an age; `process_field_turns` walks them in the reference's order (x outer, y inner, insertion order within a square) and despawns the dead. The three steps of `map::process_fields_in_submap` are the rule: a dead field goes before anything runs, a field **created this turn** is spared its processors and only decays, and the decay comes **last**. The turn is taken per field at its **live** intensity, so a field that upgrades uses the new level's numbers next turn. Five tests. **Correction, round 290:** rounds 285 to 289 had built a second authority - a per-square container on `Level` - and the five green tests that guarded it proved nothing, because nothing compared it against the store. The container and its tests are deleted; the turn rules live in the store's own system, and `FieldTurn` moved to `cdda-core-types` so the sim can name a turn without depending on the data crate. The claim that a `Level` container still fed the light was wrong: nothing feeds it but the store |
| Gas spread | done | P2 | `gas_spread_to` reproduces `map::gas_spread_to` (`src/map_field.cpp:250-273`): gas thins as it moves and gives up the fraction of its age equal to age divided by intensity, and a square already holding the gas gets one thicker instead of a second field. The **direction** is ported too (rounds 500-520): the spread chance is `rng(1, 100 - windpower)`, the starting neighbour is a die, the walk goes clockwise from it, and a step into the wind is resisted by its own roll. Vertical movement - below, then a neighbour, then above - is gated by two recorded sessions, one of them dropping a gas down a stairwell. Three sessions replay it die for die, and each has a receipt: `gas-60` `0xbb4513a5ce73269a`, `gas-fall` `0x033342d5a7845618`, `gas-wind` `0x12ca5c3c692106d4` |
| Sunlight as a system | done (P2-T0c) | P2 | `cast_sunlight` writes the sun's answer into the same [`Light`] resource the torches write, so daylight and lamps meet in one place. Four tests: open sky is bright at noon and black at midnight; a room with no sky depends on its torch; a torch outdoors never makes a square darker and never doubles it, because sources take the brighter corner rather than summing (`update_light_quadrants`); and clearing the answer leaves nothing, because a stale answer is not an answer. The pipeline is clear, then sun, then sources, and a test caught the reason: a merge by maximum will happily keep last turn's brighter value and hide a dimming |
| Light as a system | done (P2-T0c) | P2 | `cast_lights` reads the store - terrain from the map resource, sources and smoke as components - and writes a [`Light`] resource of four quarters per square. The smoke test that used to live in the map layer now lives here: a torch lights a room, and an entity standing between torch and wall dims the wall. Terrain transparency is still a two-valued rule (solid or open air) because per-terrain transparency has not been ported; the ledger says so rather than pretending otherwise |
| Point light sources | done (memoised transparency) | P2 | `Level::add_light_source` casts from a source with the ported caster and raises the quarter facing it by `sight_calc` of the luminance, falling off with distance. Five properties hold: nearer is brighter, the corner facing the lamp is the lit one, a wall shadows what is behind it, a source under `LIGHT_AMBIENT_LOW` casts nothing (its range is zero), and a second lamp raises a tile without doubling it. The ray accumulation the reference does is now ported too: `cast_light_values` averages transparency along each ray as it travels (`accumulate_transparency`), so smoke dims what is behind it - gated by a test that puts a band of smoke in front of a wall and requires the wall behind it to be darker. The sign is worth stating because it cost a round: the value is attenuation *per tile*, so open air is 2.3/72 = 0.032, smoke is 0.38, and higher means dimmer - while solid is 0.0 and special-cased as blocking |
| Daylight on the level | done | P2 | `cast_sunlight_with` reproduces `build_sunlight_cache` (`src/lightmap.cpp:306-437`) as a cascade over three rungs - the empty sky, the roof layer and the ground - so each square takes its light from the square above it or, failing that, from one of the four cardinals. A square just inside a doorway is bright on the door side only, and the same room is black at midnight. The roof above a square is its own `INDOORS` flag; the weather charge comes from the outside cache's stamp |
| Astronomy (sun position and light) | **ported and gated against 424 samples** | P2 | `cdda_core_types::astronomy` transcribes `sun_ra_declination`, `sidereal_time_at`, `sun_azimuth_altitude` and `sun_light_at` (`src/calendar.cpp:123-421`) and is gated against `corpus/astronomy/sun.json`: **424 of 424** samples match on right ascension, declination, sidereal time, hour angle, altitude and the light level, with right ascension within a hundredth of an arc second and the light exact to `1e-3`. Two rules the data taught: the year length is **truncated to whole days** (`to_days<int>`) before it becomes an angle per day, and the timezone is **whole turns** (`angle_to_time`), so seventy-one degrees west is 17054 turns and not 17054.4 — the three tenths of a second that difference is worth showed up as a hundredth of an arc minute of right ascension (round 571). **Not ported, declared:** `sunlight_angle`, `offset_to_sun_altitude`, `solar_noon_near`, the moon phase and eternal day/night overrides |
| Open-sky light | done | P2 | `open_sky_lit_level` joins the sun's curve to the level thresholds: at altitude -15 the curve gives 1.53 which is under `AMBIENT_LOW` and reads dark, at -9 it gives 4.24 which is merely hard to see by, and full daylight is far past lit. Two ported halves, no new numbers |
| Sunlight curve | done | P2 | `sun_light_from_altitude` reproduces `sun_light_at` (`src/calendar.cpp:381-402`): piecewise, not a cosine - exponential across the three dawns, exponential to the horizon, linear to 60 degrees, flat at 125 above it. Gated on twelve values computed from the expression and on monotonicity over the whole range, because a wrong branch boundary shows up as a dip at dawn. The altitude comes from the **Astronomy** row above: the chain from latitude, day of year and hour is ported and gated on 424 samples of a year |
| Map light | **partial: the classifier, not the cast** | P2 | `light_at` reproduces `map::light_at` (`src/lightmap.cpp:674-697`) arm for arm: a source at or above `LIGHT_SOURCE_BRIGHT` is **BRIGHT** whatever the field says, then `LIGHT_AMBIENT_LIT` is **LIT**, then `LIGHT_AMBIENT_LOW` is **LOW**, else **DARK** - and it **never** answers `BRIGHT_ONLY` or `BLANK`, which belong to `apparent_light_at` where obstruction, range and memory come in. Six threshold cases plus a sweep asserting those two values never appear. The classifier is also checked against **700 measured squares** of `corpus/fields/light-session.json`: every square with its own source reads BRIGHT, no square reads bright-only, and every dark square's ambient is under the low threshold. **Not ported, declared:** the cast that produces the numbers the classifier reads - a per-turn lightmap over four levels - so the replay of the session is the next slice |
| Daylight quarters | done | P2 | `CARDINAL_QUADRANTS` reproduces `dir_quadrants` (`src/lightmap.cpp:384-404`): a cardinal lights the two quarters facing it, so a doorway lightens the floor inside it and not the back of the room. Daylight is merged by taking the brighter answer, so a lamp in one corner survives sunrise |
| Sight falloff | done | P2 | `sight_calc`, `sight_check`, `accumulate_transparency` and `update_light_quadrants` (`src/shadowcasting.h:92-116`). Light falls off as `1 / e^(transparency * distance)` - the Beer-Lambert law with the coefficients merged into a running average - only solid stops it, and two rays reaching one corner do not add because it is the same light however many ways it is computed |
| Four quadrants | done | P2 | `FourQuadrants` and `Quadrant` reproduce `src/shadowcasting.h:24-90`: four floats per tile indexed NE, SE, SW, NW in the reference's order, with `max`, `fill`, elementwise `multiplied` and `max_with`/`max_with_value`. A tile's light is as bright as its brightest quarter, which is what `lit_level` reads - a lamp around a corner still lights the floor you can see |
| Level tick | done | P2 | `Level::tick` is the one place per-turn work happens: it takes the clock and the rolls, does the field pass, and reports what it did. Light is deliberately not rebuilt there - a turn nobody looks at costs no light map, which is the reference's own lazy-cache behaviour. Pinned by replaying twenty-five turns on two identical levels and requiring the same answer every turn |
| Field processing | done | P2 | A turn ages every field on every fielded tile and takes them off when they die, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing. The visit order is the reference's: x outer, y inner (`src/map_field.cpp:168-170`), which is what makes the turn independent of the order fields were added - pinned by a test that builds two levels in opposite orders and compares the shared RNG stream afterwards, not just the intensities | A turn ages every field on every fielded tile and takes off the ones that died, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing; a tile that loses one field keeps the rest and moves its drawn type to what is left. What is not ported yet is the processors themselves - fire spreading, smoke drifting - which need the map to react to a field rather than merely to age it |
| Field layers | done | P2 | `FieldLayer` reproduces `field::add_field` (`src/field.cpp:158-182`): one entry per type per tile, merging sums intensities (clamped to the type's maximum), a dead entry revives at the *new* strength rather than the one it died with, the null type is refused, and the drawn type is the highest priority with ties going to the later arrival. Adding a type that is already there returns false - the difference between "this tile caught fire" and "the fire got stronger" |
| Field decay | done | P2 | `FieldEntry` reproduces `src/field.cpp:58-113`. Three rules that read like bugs and are not: intensity never falls below one, death is a separate flag, and a decay resets the age *and* the decay time so the next half-life is drawn afresh. The decay time itself is `now - age + half_life` where the age has already been ticked, so a field with a half-life of five decays on the fourth tick after it starts, not the fifth. Fire's own branch (`dice(2, age)` against the half-life) is ported but only its shape is tested: the exact draw belongs to the shared RNG stream and will be gated by a replay |
| Cached light | done | P2 | `Level::cached_visibility` mirrors `map::build_map_cache`: a level's light is rebuilt when the level is dirty, not once per query, so an unchanged scene costs one build however often it is read. The cache is keyed by where the light is as well as by whether the world moved - same world, different viewer, different answer - and any terrain write dirties it, which is the only way a stale answer can be avoided without tracking what each submap feeds |
| Radiation | **done (first slice)** | P2 | `Radiation` is the reference's per-square total (`submap.h:69,166-176`) and `process_extra_radiation` is `field_processor_extra_radiation` (`src/map_field.cpp:664-673`): the guard is on the **maximum**, so a level that radiates nothing returns before it draws - a die that is not thrown is a world that does not move. `fd_nuke_gas` is the only field in the data that radiates, at 0-1, 0-2 and 0-3 for its three higher intensities. `map::adjust_radiation` adds and the store does not clamp, because the reference's array is a plain `int`. **Not ported, declared:** nothing reads the value yet - a character's dose needs a character - and the store is not saved |
| Wandering field | **done** | P2 | `process_wandering_field` reproduces `field_processor_wandering_field` (`src/map_field.cpp:675-690`) and the reach is **a square, not a circle**: `points_in_radius` is a `tripoint_range` over `center ± (r, r, 0)` (`src/map_iterator.h:153-160`), and the circle is the separate `points_in_radius_circ`. A vent at intensity two fills nine squares; at intensity one it fills only its own, because the reach is `intensity - 1`. Three vents in the data wander - a gas vent into toxic gas, a smoke vent into smoke, the tindalos rift into its own gas. The rule that reads like a bug is transcribed: a square whose wandering field is **weaker** gains one level, and any other square - including one already as strong - goes to `add_field`, which **sums** the strengths (`src/field.cpp:180-186`) and lets the setter clamp to the type's level count. Five tests, one of them through the turn itself: a whole `process_field_turn` runs both processors in the reference's order (radiation before wandering, `src/map_field.cpp:2223-2238`) |
| Transparency memo | done | P2 | Whether a tile blocks light is worked out once per tile, not once per cast: two casts from nearby tiles pay only for the row and column the other never saw, and a write drops that one tile's entry rather than a whole window. The reference caches transparency per level; a per-tile memo is the same idea at a finer grain |
| Level coordinates | done | P2 | `Level` joins the three pieces: submaps own the tiles, the level owns the submaps, and a world coordinate is turned into a cell with **Euclidean** division - truncating division puts tile -1 in submap 0 and folds the world's north-west edge into its middle. A coordinate whose submap was never made reads as nothing and blocks light, so light does not travel through a world that does not exist. The origin does not light itself, and a wall at the edge of the world is still visible while what is behind it is not |
| Light levels | done | P2 | The thresholds of `src/lightmap.h:9-19`, `LIGHT_RANGE` (checked against a table the reference printed), and `map::light_at`'s order (`src/lightmap.cpp:674-696`): out of bounds, then a bright source in the tile, then daylight, then anything worth seeing by, then dark. `MAX_VIEW_DISTANCE` is 72, not the 60 the shadow-caster's radius uses - two constants that look alike and are not |
| Shadow casting | done (eight segments) | P2 | `cast_light` reproduces `oldCastLight` (`tests/shadowcasting_test.cpp:33-90`) driven by the eight segments of `src/shadowcasting.cpp:545-580`, and is gated on five grids the reference itself printed (`corpus/shadowcasting/`). Four segments leaves two quadrants dark; the origin does not light itself |
| Per-level caches | done (dirty submaps) | P2 | `MapCache` reproduces `src/map_cache.cpp`'s promise: a changed tile marks its submap stale, the next read rebuilds that submap once, and a clean read costs nothing. Twenty marks in one submap are one rebuild - a cache that rebuilt per mark would be correct and useless. Invariant I8 is what the tests check |
| Submap ownership | done (one store, ids not pointers) | P2 | `SubmapStore` owns every submap and hands out `SubmapId`, which is an index that survives the store's growth - the reference hands out raw pointers into its map buffer and the port does not, because a pointer is a lifetime the whole simulation would carry. An id that was never made returns nothing rather than a default: an ungenerated submap has no terrain, and the reference's null pointer says the same |
| Tile storage | done (uniform pages) | P2 | `TilePage` is the reference's `submap` storage (`src/submap.h:74-130`): no array until the first write that differs from the uniform value, one-way - writing the uniform value back does not collapse it, because `ensure_nonuniform` does not. Cells are stored row by row as `ter[y][x]`, so a walk sees the reference's order |
| Line walk | done (2D walk) | P2 | `line_to` reproduces the reference's Bresenham step for step, including the starting error term that decides a tie; 14,400 target/offset pairs land on their target (`tests/line_test.cpp:416`), and the degenerate line yields one cell because the reference's walk enters its body once. Not ported: `bresenham` over 3D bubble points, `squares_closer_to_test` |
| Sound | **partial: distance, reach, falloff and the turn's queue** | P2 | `sound_distance` and `heard` transcribed from `src/sounds.cpp:253-275` and the pre-filter at `:484-489`: distance is measured in steps where a diagonal is one, and a noise carries `volume * 2` squares - volume ten reaches nineteen and not twenty-one, with the edge exclusive because the reference's test is strictly greater. Now also the turn's queue: `SoundCategory` in the reference's thirteen-category order with `is_provocative` exactly as `src/sounds.cpp:287-313` writes it - **four** categories provoke (speech, electronic speech, alert, order) and a fight does not, which is worth stating plainly because it is the opposite of what a reader expects - and `SoundQueue`, whose guard is `volume < 0` rather than `<= 0`, so a silent noise is still a noise and a negative one records nothing at all. Four tests. **Now ported:** clustering (`cluster_sounds`, `src/sounds.cpp:365-431`) - a list of at most **ten** noises is never clustered, because `num_seed_clusters` is `max( min( n, 10 ), log( n ) )` and every seeded cluster is a single noise, so small lists come out exactly as they went in; a longer list is seeded by **drawing** from the dice and removing the chosen noise by swapping the last into its place, which makes both the draw order and the swap part of the world. A cluster's position is a volume-weighted average, its volume is the **maximum** rather than the sum - a monster hears the loudest thing in it - and it provokes if **any** noise in it did. Five tests, one of which corrected my own arithmetic in a fixture I had just written. **Now ported:** the vertical attenuation (`sound_distance_3d`, `src/sounds.cpp:253-269`). The horizontal part is the ordinary step distance; the vertical part is scaled five times, and below ground the first level down adds four **before** that scaling while each further level adds twenty more - so one level up costs 5, one level into a cellar costs 25, two levels 30 and three 135. The reference's own comment calls these "5x" and "100x", which is not what its arithmetic does; the port follows the arithmetic, because that is what a monster's hearing is built from. Five tests. **Now ported:** the weather and the horde signal. `heard_volume` subtracts the weather's `sound_attn` **without a guard**, so a quiet noise in a storm is negative - clamping at zero would report a different number to anything that shows the player what they heard. `horde_signal` (`src/sounds.cpp:432-458`) turns that reduced volume into an overmap signal: underground it divides by **twice** the depth, a volume of **sixty or less sends nothing**, and past that the signal is the volume over `SEEX`, floored at eight and capped at twenty-six, rounded **up**. Four tests, two of which corrected my own arithmetic in the same round - I wrote that sixty after the weather sends a signal when the gate is strictly greater, and I wrote the depth as positive so the underground division never ran. **Not ported:** weather as a system (the port takes the attenuation as a number), the rest of the player-facing list, horde signalling, and the player-facing list that draws markers |
| Scent | **done: the diffuser, the marks and the turn** | P2 | `cdda_core_types::scent::ScentWindow` transcribes `scent_map::update` and `scent_map::decay` (`src/scent_map.cpp:60-285`) - a square keeps most of its scent and hands a little to its eight neighbours, `NO_SCENT` terrain blocks the exchange and empties its own square, `REDUCE_SCENT` takes part at a fifth of the weight - and `sim::advance_scent` joins it to the turn the way `do_turn.cpp:642-658` does: the character deposits, the window diffuses over the character's radius, the grid is loaded into the window first so scent stays where it was left. Pinned by six arithmetic tests, a terrain-marks gate and a recorded session. **Not ported, declared:** scent types, and the pause after standing still |
| Map memory | **ported and gated: the tile, its four writes and the prefix clear** | P2 | `cdda_core_types::memory::Memorized` is the reference's `memorized_tile` (`src/map_memory.h:22-54`) field for field — terrain **and** decoration, each with its subtile and rotation, plus the glyph — and `MapMemory` carries the four writes (`set_terrain`, `set_decoration`, `set_symbol`, `clear_decoration`), each preserving the rest of the tile the way the reference does by reading it out and writing it back (`src/map_memory.cpp:218-276`). Gated against `corpus/memory/tiles.json`, a scripted sequence with every branch of the clear in it: a decoration cleared by its own prefix takes its symbol with it, one a prefix does not match keeps everything, and the **empty prefix matches every id**, which is how a region is forgotten at once. Five squares compared, and the gate was mutation-checked by removing the symbol clear. `remember_seen` still writes only terrain and light — the subtile, rotation and decoration need the drawing data the port does not have yet. **Not ported, declared:** drawing a remembered square dimmed (there is no frame path for it), and `prepare_region`'s cache rectangle, which is an optimisation over the store rather than a rule about it (round 582) |
| Vision (`seen_cache`) | **ported and gated: the cross-level cast, the engine's own level, and the rule joining sight to light** | P2 | Three gates, and they are three different claims. `zlight::build_seen_cache` over the reference's own recorded inputs reproduces its `seen_cache` on **all three levels** of `corpus/vision/start-location.json` — the transparency and floor bits the reference read are recorded beside the answer, so a disagreement cannot hide in a loader. The engine's own `Level::seen_grid_of`, given the same transparency map, reproduces the **character's level** (289 squares) — a cast that is right and a level that hands it the wrong table are two different failures. And `vision::shown_at` joins sight with light the way the draw path does (`src/game.cpp`): a square is shown when the cast reaches it **and** something lights it, with `apparent_light_at` applying the brightest-quadrant rule. Seven tests across three files, both port-side gates mutation-checked (an empty transparency map fails 163 of 289 squares). Drawing a remembered square dimmed is **ported** as of round 585: the frame draws a square whose level is `DARK`, `BLANK` or `MEMORIZED` from the memory, and memorizes what it draws. **Not ported, declared:** the reference's camera cache, which is the avatar's unimpaired range rather than the map's, and the `BRIGHT_ONLY` glow (`#` in the interface), which needs the screen layer (round 585) |
| One turn of field processing | **done (assembly); bubble and terrain decide** | P2 | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt (`src/map_field.cpp:160-460`). Two things the world decides rather than the prototype: a field outside the **reality bubble** is untouched - not even aging, which is what the reference means by leaving it unloaded - and the **map** supplies water, sky and passability, so the prototype says how much faster a field ages under water while the terrain says whether it is under water. The bubble is 132 squares on a side, radius 66, matching `MAPSIZE` x `SEEX` in `src/map_scale.h`. Seven tests, including a field outside the bubble left exactly as it was with **nothing drawn for it**, and the edge of the bubble counting while one square past does not | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt - the order `map::process_fields_in_submap` uses (`src/map_field.cpp:160-460`). The **map** now supplies water, sky and passability through `CellFacts`, and those override the prototype's own idea of them: the prototype says how much faster a field ages under water, the terrain says whether it is under water. A field that dies is removed before the later steps; the first version only checked that the entity existed and a test caught it. Five tests, including the same field over water and over dry ground. Per-field data still comes from the caller, so the prototype stays in the content crate | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt - the order `map::process_fields_in_submap` uses (`src/map_field.cpp:160-460`). A field that dies is removed **before** the later steps, which is what the reference's `!is_alive()` check does; the first version of this only checked that the entity existed and a test caught it. Four tests: an ordinary field just ages, the walk is deterministic (no dice for ordinary fields), one spread attempt per turn rather than one per neighbour, and a dead field is never asked to spread. Per-field data comes from the caller, so the prototype stays in the content crate |
| Items in a fire (the loop) | **done** | P2 | The loop from `field_processor_fd_fire` (`src/map_field.cpp:980-1031`): a fire finishes at most `intensity * 2` items a turn and stops once it has; each item's contribution comes from `simulate_burn` and its bookkeeping from `burn_item`; anything finished leaves the map and the fire is handed the summed smoke and fuel to round. Ground items gained a `burnt` counter for it. Six tests: a small item consumed, the two-per-intensity cap, a large item surviving but remembering, **no plan means no dice**, a stack losing charges instead of vanishing, and a fireproof item untouched by all of it. `will_explode_in_fire` **is** honoured: items that go off are removed **before** anything burns (`src/map_field.cpp:982-993`), spill what they held, and are counted as `detonated` rather than `consumed` - and a test proves nothing burned and **no dice were drawn** even when the burn plan would have destroyed it. **Not ported, declared:** the explosion itself (blast damage and its own content). Burn products **are** ported: `burn_products` follows `map::create_burnproducts` (`src/map_field.cpp:116-143`), spawning `floor(efficiency * burned / product_weight * portion / portions_total)` of each product of each material, never a copy of the burning item itself. The material dump carries the products now - **64 of 193** materials have them, wood leaving ash at 0.018 - and both the corpus count and the arithmetic are tested. The contents of a destroyed container **are** handled now - they spill onto the square, with the reference's irremovable-item rule still missing |
| Item burning bookkeeping | **done** | P2 | `roll_remainder` (`src/rng.cpp`) - whole values draw nothing, fractions round up with the probability of the fraction - and the charge/accumulation half of `item::burn` (`src/item.cpp:12609-12643`): a stack loses `roll_remainder(burn * 250ml * stack_size / (3 * volume))` charges and a stack with no volume loses them all outright; a solid accumulates `roll_remainder(burn)` and is gone at `volume / 250ml * 3`. Six tests, two of which pin that nothing is drawn when it should not be. **Not ported here:** the corpse branch (needs monster `burn_into` data) and `heat_up` |
| Item burning (`simulate_burn`) | **done** | P2 | How much of an item a fire eats in a turn, transcribed from `src/item.cpp:12543-12592`: a contained fire burns at intensity 3 regardless; an immune material stops the whole item; a material with a `volume_per_turn` scales by that volume against the item's; a liquid giving the fire under 200 **subtracts** `rng(400-1200 per litre)` because wet things smother small fires; several materials are averaged by their declared portions; no materials at all burns at a flat 1; and a stack multiplies by its number of stacks. Nine tests, including that a solid item draws **no dice at all** (stream compared) - the liquid branch is the only one that rolls |
| Material burn data | **done, with a declared gap** | P2 | `--dump-materials` now carries per-intensity `{immune, volume_per_turn, fuel, smoke, burn}` (`src/fire.h:45-56`), and the loader reads `burn_data` from the data files, including volume strings like `"200 ml"`. What the data says: **109 of 193** materials describe burning, wood fuels 1/2/3 across the intensities, and **exactly one material is fireproof at every intensity** (`nomex`). **Declared gap:** the other 84 materials get defaults filled in by the reference at load time, which the port does not reproduce, so a material with no `burn_data` has no burn entry here rather than a zeroed one |
| Monster spawn gate | **done, with packs declared** | P2 | The field dump now carries per-intensity `spawn_chance`, `spawn_count`, `spawn_radius` and `spawn_group`, and the data says **only two levels in the whole corpus spawn anything**: `fd_tindalos_rift` at 15-in-100 and `fd_fatigue` at 600-in-100. What is ported is the gate (`src/map_field.cpp:763-787`): positive chance and count, one `one_in(chance)`, then the whole count is asked for at once - pinned by a test that three-at-a-time equals three singles over 400 seeds, not three separate rolls. **Now ported:** turning a group into creatures - `spawn_from_field` tests the count and chance **before any dice**, rolls one `one_in(chance)`, then asks the group until the count is used up, over the 401-group table. Four tests: three monsters for a count of three, about one trigger in a hundred at a chance of a hundred over 2000 fields, **no dice at all** for a level at zero, and nothing for a group the data does not define. **Still not ported, declared:** the reference asks for a **batch** and passes the count in by reference, so a pack entry consumes several points and spawns several monsters at once; packs are not modelled, so a batch here is always one. Placing the monsters on random passable squares within the radius needs a map, and is the next piece |
| Toxic and nuke gas processors | **done** | P2 | Toxic gas does damage equal to its intensity and takes that many moves, drawing nothing, and a creature that does not breathe takes nothing (`src/map_field.cpp:1966-1971`). Nuke gas draws a move penalty and damage per intensity, then a plant takes a second move penalty and has its damage multiplied (`src/map_field.cpp:1972-1990`); the draw order - penalty, damage, plant penalty - is pinned against a second transcription over 3 intensities x 2 breath states x 2 body kinds x 20 seeds. Six tests, including a 30-seed comparison showing plants come off worse, and two that apply an effect to a creature in the world (hurt and slowed, and killed) |
| Creatures on the map | **done (foundation)** | P2 | A creature is an entity with a square, a kind and hit points (`src/monster.h`); damage comes off the hit points and a creature at zero or below leaves the world, which is the death check a field processor needs (`src/monster.cpp`). Six tests: where it stands, damage, lethal damage, **negative damage is not a heal**, spawn order on a shared square, and creatures and items coexisting on one square without either query seeing the other. The monster **prototype** loader already existed (1,154 types from `--dump-monsters`) - this round was the runtime side. **Not here yet:** speed, moves, armour, behaviour, and the part of `monster_spawn` that picks a species |
| Underwater dissipation and self-upgrade processors | **done** | P2 | Two small ones, both now data-driven: a field under swimmable terrain ages by its `underwater_age_speedup` with no dice (`src/map_field.cpp:516-523`), and a field grows one intensity on `calendar::once_every(upgrade_duration)` when `one_in(upgrade_chance)` fires, with both values positive (`src/map_field.cpp:503-514`). Five tests, three of which pin the *absence* of a draw: off the period, chance zero, and dry land. The data note carried over: only `fd_fatigue` has an upgrade path at all |
| Gas spread processor | **done** | P2 | `field_processor_spread_gas` (`src/map_field.cpp:579-585`) is a one-line gate over `map::spread_gas`, which the port already had - so this round was about the piece that was missing: a gas **outdoors** ages by the field's `outdoor_age_speedup` *before* the spread dice and **before** the intensity bail-out, so a thin gas outside still dissipates faster than inside. That ordering was wrong in the first attempt (the intensity check came first) and a test caught it. `process_spread_gas` refuses a field whose spread chance is zero without drawing anything. Nine tests in the file: the existing spread behaviour, outdoor versus indoor aging, and both processor gates |
| Field processor data | **done** | P2 | `--dump-fields` now carries what the remaining processors read: per-field `underwater_age_speedup`, `outdoor_age_speedup`, `percent_spread`, and per-intensity `upgrade_chance` and `upgrade_duration`. Findings from the data: smoke spreads **10%** a turn, and **exactly one** field in the whole corpus can upgrade itself (`fd_fatigue`) - which is worth knowing before writing an upgrade processor that would almost never run. Re-dumped and the loader extended with one test over all 145 fields |
| Push-items field processor | **done** | P2 | Only rocks count, and only rocks that have lain there at least one turn; a rock is thrown to a random neighbouring square that holds a push-items field of its own, losing its age on landing, and a square with no such neighbour keeps its rocks (`src/map_field.cpp:790-836`). Five tests: an aged rock thrown and its age reset, a fresh rock left alone **without drawing dice** (stream compared), non-rocks untouched, nowhere to throw, and two rocks thrown one after another with their charges intact. Ground items gained an **age** for this processor - it is the first one that cares how long something has been lying there. **Not ported:** the reference also damages whatever creature stands where a rock lands; there are no creatures yet |
| Fire: the whole turn | **done (assembly)** | P2 | One turn now runs the slices in the reference's order (`src/map_field.cpp:945-1425`): eat the ground, then age (gained time spent first, otherwise a starving fire ages ten seconds per intensity), then either the flashpoint `one_in(3)` for a strong fire **or** the growth check - never both - then spreading, then smoke and hot air. Three tests: a 20-seed x 3-intensity comparison against a manual composition of the same steps (this proves the **order and wiring**, not the steps, which have their own gates), a fire on plain ground gaining life, and a fire on bare ground starving by twenty seconds at intensity 2. **Not in:** items burning, vehicles, fall-through to the level below, webs, pits, creature effects |
| Fire: wind reach | **done (seventh slice)** | P2 | Which neighbours the wind lets a fire reach, transcribed exactly from `src/map_field.cpp:1136-1152`. **Both my readings of the condition were wrong before the tests corrected them:** it is not "the wind shields most neighbours" (the first guess) and not "the shield is vacuous" (the second) - the `differs` test is `x != b.x && y != b.y`, so a neighbour that shares *either* coordinate with a blocker counts as not differing, and only a neighbour sharing coordinates with **all three** blockers reaches the `x_in_y` and can be stopped. The test proves the draw set with the stream compared per neighbour: exactly the three blockers draw, and a blocked neighbour gets through sometimes over 40 seeds |
| Fire: eating terrain | **done (sixth slice)** | P2 | `FLAMMABLE` ground feeds the fire `5 - intensity` turns and always burns; `FLAMMABLE_HARD` needs `one_in(3)` and gives `4 - intensity`; `FLAMMABLE_ASH` always burns and leaves 10-1000 ash when the ground is wrecked. Every burning kind adds two smoke plus one per five wind power, and a fire above intensity 1 wrecks the terrain on `one_in(200 - intensity * 50)` (`src/map_field.cpp:1051-1090`). Gated against a second transcription over 3 intensities x 3 wind powers x 8 flag combinations x 12 seeds. **The subtlety:** a square that is only `FLAMMABLE_HARD` and fails its `one_in(3)` burns nothing at all - it does not fall back to a cheaper burn |
| Fire: smoke and hot air | **done (fifth slice)** | P2 | Smoke needs three gates in the reference's order - the terrain not suppressing it, `rng(0, 100 - windpower) <= consumed`, and `rng(3, 35)` under intensity times ten - and hot air needs fewer than five adjacent fires plus a non-zero `rng(0, 4 - adjacent)` (`src/map_field.cpp:1400-1425`). Both dice orders are pinned against a second transcription. **A finding worth keeping:** with nothing consumed, the first gate passes one time in 101, so a weak fire over bare ground smokes about once in 500 turns - the test asserts zero smoke in 80 turns because that is the truth, not because the port is broken. **Simplification:** the reference prefers the square above, and the port has no levels yet, so smoke always takes the fallback at the fire's own square |
| Fire: flashpoint | **done (fourth slice)** | P2 | A fire above intensity 1 draws `one_in(3)`; if it fires, the fire hands five minutes of its own life to each neighbour worth feeding - one that is no stronger, or older, i.e. with less time left - visiting the eight neighbours from a random start and stopping the moment its own age is no longer negative (`src/map_field.cpp:1164-1215`). A fed neighbour under intensity 2 grows to 2 and always gains five minutes. The pass also returns the adjacent-fire count the growth ceiling needs, as the reference counts both in one loop. Four tests: the decision table, a small fire drawing nothing (stream compared), a 60-seed search that finds a feed and checks both sides of the trade, and a stronger younger neighbour left alone |
| Fire: growth | **done (third slice)** | P2 | The ceiling and the cost, transcribed from `src/map_field.cpp:1240-1262`: under -500 minutes of fuel age a fire may reach 3 outright; otherwise neighbours decide (three allow 2, seven allow 3) and more than -50 minutes of fuel age gives 2 to a lone fire. Growing is one step at a time and costs ten minutes of age per point of the size it reaches. The two thresholds **overlap**, so at exactly -500 minutes the answer is 2 rather than 1 - which the first version of the test got wrong, and the reference's ordering is what decides it |
| Fire: spreading | **done (second slice)** | P2 | The spread decision is transcribed draw for draw from `src/map_field.cpp:1290-1345`: `power = intensity + one_in(5)`, then `rng(1,100)` against 25 per point of intensity above one, then the fuel checks in flag order - `FLAMMABLE` and `FLAMMABLE_ASH` on `one_in(2)`, `FLAMMABLE_HARD` on `one_in(5)` and only at power 3+, then `one_in(5)` for items - with a burning square skipped before any dice. A new fire starts at intensity 1 and two minutes old, and the source spends a minute of its own life per fire, which is why a fire that spreads also burns out sooner. Gated against a **second transcription** over 3 intensities x 20 fuel combinations x 12 seeds, plus a stream test proving a burning neighbour draws nothing, and 60-seed tests requiring both outcomes. **Not yet:** wind direction, shelter, webs, pits, and the growth-to-raging path |
| Fire: burnout over bare ground | **done (first slice)** | P2 | Fire with nothing to eat gains ten seconds of age per point of intensity, so a strong fire over bare ground dies sooner than a weak one - the reference's own comment is "drain more power from big fires, so that they stop raging over nothing" (`src/map_field.cpp:1128-1132`). The caller says whether the tile has fuel, because "fuel" here is terrain and furniture data. Four tests, including that a fire which has already been removed is left alone rather than despawned twice. **What is not ported:** consuming items, consuming terrain, spreading to neighbours, wind and smoke - those come in the next slices |
| Terrain prototypes and flammability | **done** | P2 | **1,769** terrains from `--dump-terrain`, of which **643** are flammable. Flammability is derived, not stored: the reference checks three flags (`src/mapdata.h:634-637`) and so does the port, because a stored copy can disagree with the flags it came from. One test exists purely to stop a plausible shortcut: `t_lava` carries `USABLE_FIRE` and does **not** burn - a port that grepped for "FIRE" would set lava alight |
| Items on the ground | **done** | P2 | A tile's items are entities with a square, a type and charges, handed back in the order they were dropped (`src/map.cpp`, `map_stack`). Five tests: insertion order, an empty square, a square never returns a neighbour's items, charges survive, and taking one out of the middle leaves the rest in order. **Why the order is load-bearing:** a fire that burns a pile consumes it front to back, so the order is part of what a replay has to reproduce - which is why it is pinned before fire is ported, not after |
| Shock vent field processor | **done** | P2 | A vent stronger than 1 draws `one_in(5)` and thins; a spent one refills to 3 and walks 3 to 6 bolts of 4 to 12 steps, laying electricity of strength 2 or 3 and turning after each step on two `one_in(4)` draws (`src/map_field.cpp:838-876`). The dice order *is* the behaviour here, so the test is a **second transcription of the same C++** compared against the port over 20 seeds: same vent intensity, same lit squares, same strengths. A separate test runs 40 seeds on a strong vent and requires both outcomes - thinned and stayed - so a stuck branch cannot pass |
| Acid vent field processor | **done, with a declared divergence** | P2 | A vent stronger than 1 thins by one and resets its age once at least a minute old, drawing nothing; a vent at 1 refills to 3 (`src/map_field.cpp:877-901`). **Divergence:** the reference's second half walks a radius of 5 for acid fields at *intensity zero* and strengthens them - a branch that is dead in the reference too, because `set_intensity` floors a field at 1, so no zero-intensity field can exist. The port does not reproduce dead code, and a test asserts that a vent makes no acid of its own |
| Extinguisher field processor | **done** | P2 | An extinguisher cloud fights fire on its own square, both losing the smaller of the two strengths (`src/map_field.cpp:559-569`): a weaker cloud is spent and leaves the fire burning lower, a stronger one puts the fire out and survives smaller, equal strengths remove both, and a cloud with no fire keeps its strength. No dice are drawn, so nothing shifts a replay. Five transcribed tests; the ids are the loader's (`fd_fire` 14, `fd_extinguisher` 16) and checked by `field_ids.rs` |
| Field ids in the sim | **done** | P2 | The sim's field constants are the loader's ids, checked against the reference's dump by `crates/sim/tests/sim_suite/field_ids.rs` (four tests: the null id is never assigned, the six hardcoded fields exist, ids are unique, and each constant equals the registry's answer). Replacing the 900-range placeholders broke two tests that had assumed their *ordering* - one filtered `FLAME_BURST.0 ..= FIRE_VENT.0`, which becomes an empty range once the pair is (26, 28) - which is exactly the kind of latent assumption a placeholder hides |
| Temperature | **ported and gated: the three terms, and the terms a square below street level ignores** | P2 | `sim::temperature` transcribes `weather_manager::get_temperature` (`src/weather.cpp:956-982`), `get_heat_radiation` (`src/game.cpp:1755-1795`) and `get_convection_temperature` (`src/game.cpp:1835-1855`). Gated against `corpus/temperature/squares.json` — a corridor of floor with lava, hot spring water and a cold gas on it, nine squares, each recorded with its three terms and its total. The order the three are summed in is the reference's own (into a single-precision delta, then onto the temperature), because adding them one at a time rounds differently. Two loader gaps closed for it: terrain and furniture now carry `heat_radiation` (prototype dump version three; two prototypes in the whole data radiate) and field intensities carry `convection` (field dump version five; eleven fields have a non-zero one and the cold-air families are negative). **Not ported, declared:** the per-square cache (`temperature_cache`, `:963-980`) — this is the rule, not the store; the trap flag `CONVECTS_TEMPERATURE` (`:1838-1843`), which needs traps placed on the map rather than loaded; the creature's own warmth; and the `line_to`-based permeability branch for a square that has a creature on it (round 581) |
| Weather | **done: the generator, the condition loop, the sun and the manager** | P2 | `weather::System::tick` computes the point, decides the sky and books the next change in one call and reproduces the reference's recorded ten-day day from the recorded dice alone; `Climate` is a resource and `tick_climate` runs a turn of it (rounds 511-533). The manager's own path is gated on 240 hours with fourteen absorption changes. **Not ported, declared:** the falling-snow tile layer and the lightning strikes |
| Zone manager | **ported and gated: the four queries and their order** | P2 | `zones::ZoneManager` from `zone_manager` (`src/clzones.cpp:1112-1396`): zones are inclusive rectangles of a type, a faction, an `invert` flag and an `enabled` flag, and the four queries read the list from **both ends** — `zones_at` and `zone_at` walk it forwards (the oldest zone wins), `bottom_zone` and `loot_zone_at` walk it backwards (the newest wins), and "loot" is the **prefix of the type's id**, not a flag. `has` and `has_near` ask the **cached point set** (`cache_data`, `src/clzones.cpp:892-917`), which skips a switched-off zone — so the same square answers "yes" to "what am I standing in" and "no" to "is there a food zone here". Gated against `corpus/zones/probe.json`: 240 squares, 2,640 answers, seven overlapping zones plus one of another faction. Two rules the fixture pins: `has_inside` **ignores `invert`** (the rectangle is the rectangle; the inversion belongs to the reader) and **ignores `enabled`** (a switched-off zone still contains its squares). **Not ported, declared:** zone options, personal and vehicle zones, the point-set cache's staleness and `get_point_set_loot` (rounds 573-574) |
| Explosions | **ported and gated: the blast geometry, 812 squares and 824 pops** | P2 | `sim::blast::blast` transcribes `do_blast` (`src/explosion.cpp:185-310`): the wavefront, the `power * factor^distance` falloff, the shaped-charge bonus that multiplies the force by up to three when a square has few open neighbours, and the ten-neighbour order. Gated against `corpus/explosions/blast.json` — three blasts in a room sealed on **three levels**, compared on the **pop order** (every square the queue pops, in order, with its distance and force) and on every bash (812 squares, the force each was hit with). Both are exact. The queue itself is a transcription of **libc++**'s heap (`__sift_up`, `__floyd_sift_down`), gated separately on `corpus/explosions/heap.json`: the reference's comparator compares the distance and nothing else (`src/cata_utility.h:35-41`), so the order of a tie is a standard-library internal — libstdc++ picks the right child, libc++ the left, and the oracle is built with clang. **Not ported, declared:** creatures and items in the blast, fragments and shrapnel, the fire it starts, the `NATURAL_UNDERGROUND` roof it puts back, and the drawing (round 578-579) |
| Bashing | **ported and gated: terrain and furniture, 97 hits** | P2 | `sim::bash::bash` from `map::bash_ter_furn` (`src/map.cpp:3932-4245`), gated against `corpus/bash/hits.json` — eight fixtures, 97 hits, each compared on what the square became, the damage it kept, `did_bash`, `success` and `bashed_solid`. The oracle needed a **hook**: `bash` draws its roll from the global rng, so the dump hands the roll in and the port takes it as a parameter. Three rules the hits pin: furniture **hides** the terrain underneath (a chair on a concrete floor is a chair); a failed hit past `str_min` leaves at least one point of damage, which the *next* hit reads as strength; and `success` is **not** "something broke" — a `destroy` hit breaks the square and reports success false. **Not ported, declared:** items, fields and vehicles on the square, sounds and messages, `explosive`, tents, spores, signage, `furn_is_supported` (no data has `str_min_blocked`) and the **open-air path** — six of the 97 rows, where a destroyed floor becomes `t_open_air` and the reference regenerates a roof from the terrain below (round 576) |
| Terrain roofs | **ported and gated: all 1,109 terrains** | P1 | `MapData.roof`, from `optional( jo, was_loaded, "roof", roof, NULL_ID )` (`src/mapdata.cpp:1218`), compared prototype by prototype in the 1,769-row map-data gate. The field has two spellings of "none" and only one of them is none: the reference's null terrain id is **`t_null`**, so 534 terrains carry `t_null` and mean "no roof", while every piece of furniture prints an empty string because it has no field at all. It is what `map::get_roof` answers with, and therefore what a destroyed floor regenerates into — `t_rock_floor`'s roof is `t_rock_roof` (round 577) |
| Bash data | **ported and gated: every terrain and furniture block** | P2 | `mapdata::BashInfo` from `map_common_bash_info` and its two subclasses (`src/mapdata.h:40-84`, `src/mapdata.cpp:376-430`), loaded for all 1,570 blocks the dda set defines — 958 terrains and 612 pieces of furniture — and gated against `corpus/bash/info.json` field by field. Three rules the dump settled that a transcription would have guessed wrong: `ter_set_bashed_from_above` defaults to `ter_set` **only when the block is fresh**, so a child that renames `ter_set` keeps its parent's above-result (533 blocks turn on it); furniture's `furn_set` defaults to the real id `f_null`, not to nothing; and a block that names no items gets the real group `EMPTY_GROUP`. **Not ported, declared:** inline `items` arrays, which the reference turns into anonymous groups with ids no port can reproduce; `explosive`, `restricts_hauling`, `tent_centers`, `hit_field`/`destroyed_field` and the `sound`/`sound_fail` texts (round 575) |
| Zone types | **ported and gated** | P2 | `content::zones::load_paths` loads the 64 zone types the dda set defines, from the JSON tag `LOOT_ZONE` (`src/init.cpp:370`) — not `zone_type`, which is the mistake that returns an empty registry with no error. Gated against `corpus/zones/all.json` on id, name, description, display field, `can_be_personal` and `hidden`: 64 of 64, including the 42 a character may own and the one hidden type. **Not ported, declared:** the zone manager's options, `has_zone_type_for`, the personal and vehicle zones, and the area caches (rounds 572-574) |
| Falling and explosions | **explosions: done and gated; falling: in the turn, items and fields done, furniture recorded and not ported** | M2 | Explosions: see the Explosions row. **Falling:** `sim::falling` transcribes `map::process_falling` (`src/map.cpp:3018-3035`) and the two halves of `drop_everything` (`:2693-2705`) that do not need creatures or a vehicle — items, which fall to the **lowest** floor under them and not one level down, and cosmetic fields, which fall exactly **one** level (`:2977-2994`). Gated against `corpus/falling/fall.json`: five columns that lost their support, one of which has a floor of its own and must not move at all, one that falls three levels, one that carries smoke, and one item column that falls one. A liquid field with `accelerated_decay` that falls onto a hole is **destroyed**, not held in mid-air, because `map::add_field` refuses to place a non-gaseous field on a square with no floor (`:6531-6534`). Four tests. The field loader gained `accelerated_decay` and `phase` for all 145 fields (dump version four), gated on the eighteen fields that fall and the five phases the data uses. **Not ported, declared:** furniture falling, which bashes the square it lands on and so raises dust — `corpus/falling/fall.json` records the bench landing two levels down with `f_bench` and `fd_dust` intensity two, which the fall gate asserts as a number rather than a note; creatures falling (`gravity_check`); the impact damage an item takes, which needs item damage and resist that the port does not have; and the vehicle. The item falls are also assumed to land on an empty square, because the reference appends to the stack below and merging is not modelled. **Now in the turn:** `run_environment_turn` runs `process_falling_turn` in the reference's own position in it — after the scent and before the fields (`src/do_turn.cpp:642-655`) — driven by a `SupportLost` queue that is taken whole and emptied, the way `process_falling` moves `support_cache_dirty` out and clears it, so a square is asked once. Three tests: an item above a hole falls and the queue empties, the next turn moves nothing, and a cosmetic field falls with the item. Removing the step from the turn fails all three (round 583) |
| Vision through fields | **ported and gated** | P2 | The frame's transparency table folds the same three layers the reference does (`calc_transp`, `lightmap.cpp:140-167`): terrain, furniture, and the fields — which **multiply**, so smoke at its second intensity is ten times as absorbing as air and smoke at its third intensity blinds the line. Two gates pin it (`frame_smoke`): a square behind smoke is seen less than the same square without it, and behind dense smoke not at all. This closed the last red gate the M2 exit criterion had left open (round 570) |

## P3 — Items

| System | Status | Gate | Notes |
|---|---|---|---|
| Eating | **partial, declared** | P3 | `eat` and `MealTally`: **one serving spends `charges_to_use` charges** - one for food, so the rule only shows on a soup that costs five - and the nutrition gained is the type's per serving. Seven tests: a bite costs one charge, a serving of soup costs five, the last serving empties the square, **a partial serving is not a serving and spends nothing**, a rock is not food, an empty square, and a tally that adds up over a meal. **Not ported:** hunger and thirst themselves, the eating time and its interruption, the container a drink comes from, and the rot that food does while it is carried |
| Eating and drinking: the data | **partial, declared** | P3 | The item dump carries `nutr`, `quench` and `charges_to_use` now, and `nutrition_from_dump` reads them into a table: **1,315 of 9,912** items carry nutrition, meat is **46 nutr and one charge a bite**, water quenches without feeding. The nutr value is the reference's `get_default_nutr()` - kilocalories over its own `kcal_per_nutr` - and is taken from the dump rather than recomputed, because that constant is internal to the reference and reproducing it would be inventing a number. **Not ported:** the eating action itself - hunger, thirst, the charge spending, and the container a drink comes in |
| Crafting activity | **partial, declared** | P3 | `start_craft`, `craft_turn_on`, `cancel_craft` model the activity, transcribed from `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`): the unfinished craft is **an item that holds its parts from the moment they are taken**, which is why cancelling loses nothing - the reference only removes an empty componentless recipe item. An unfinished craft carries a sentinel kind until it becomes its result. Four tests: the parts go into the unfinished item, working the turns produces the result, **cancelling keeps the parts inside**, and an interrupted craft keeps its progress. Writing them found a modelling bug: `items_at` was returning things **inside** containers as if they lay on the square, so a craft holding its own parts looked like it had left them there - contents are now excluded, which is true of every container, not just crafts. **Not ported:** the reference's `item_counter` percentage encoding, practice ticks, assistants, and the checks that abort a craft when the recipe or the workbench changes |
| Crafting time | **partial, declared** | P3 | `batch_time_moves` from `recipe::batch_time` (`src/recipe.cpp:108-149`), transcribed part by part because every part is load-bearing: a speed of zero means "cannot craft here" and reports the time as if at full speed rather than dividing by zero; a recipe that does not batch takes the whole time per unit; one that does follows a **scaled logistic curve**, which is why twenty units cost far less than twenty times one and the saving flattens at `batch_rscale`; one assistant gives three quarters and two or more give three fifths; and **no arrangement beats a single unit**, which is a floor in the reference and a test here. Six tests, including the asymptote and the floor. The work-turn loop is ported as `CraftingWork` and `craft_turn`: a turn's moves become progress scaled by `base_total / current_total`, so a slow crafter accrues less per turn rather than needing a different total, progress never overshoots, and a place that cannot be crafted in **cancels** the craft instead of crawling. Five tests, including finishing twice in a row staying finished rather than starting over. **Not ported:** proficiencies and their time maluses, the light and workplace speed factors themselves, practice ticks, and the interruption rules |
| Skills | **partial** | P3 | The recipe dump now carries `skill` and `difficulty`, and `Recipe::skilled_enough` follows the difficulty check in `Character::can_craft`: the crafter's level in the recipe's skill must reach its difficulty, and **knowing the recipe is a separate question** because the reference keeps them apart - a character can be skilled enough for a recipe it has never learned. Six tests, and the corpus gates pin the reference's own numbers: all 6,291 recipes name a skill, **2,960 are fabrication**, and **exactly 1,396 are difficulty zero** - the set a fresh crafter with the recipes known can make. **Not ported:** skill progression and practice, the borrow-a-recipe-from-another-crafter rules, and `can_train` exemptions |
| Crafting | **partial, declared** | P3 | `craft_on_square` from the component handling in `Character::craft` (`src/craft.cpp`, `consume_components`): **availability is checked before anything is taken** - the reference's `has_comps` only asks - alternatives are tried in the order the recipe lists them, and a failure leaves the square exactly as it was. Five tests, one of which found a real bug: an alternative that came up short was consumed and then abandoned when the next alternative filled the group, so a two-alternative recipe could eat something it never used. The fix was to do what the reference does - check, then consume - rather than consume and put back. **Not ported:** crafting time and progress, skill and difficulty checks, the knowledge check, tools that wear out, and the failure products |
| Recipe requirements | **partial, declared** | P3 | `Recipe::components_available` from `requirement_data::has_comps` (`src/requirements.cpp:895-940`): each group is a set of **alternatives** - one of them in sufficient number satisfies it, and every group must be satisfied - with quantities scaled by the batch. Eight tests, two of them over the whole corpus: **all 6,291 recipes are satisfiable with an unlimited stock** (a rule that wrongly rejects a group shows up for every recipe at once) and **none is satisfiable from an empty stock**. Also tested: alternatives are a choice not a sum, a missing group fails the recipe, and a batch of zero is treated as one rather than being a free recipe. Tools and qualities are ported too: `tools_available` follows `tool_comp::has` (`src/requirements.cpp:952-990`) - a count of `-1` is **not minus one charge** but "held and not spent", which is why the encoding is resolved into `ToolNeed` rather than read as a number, and a tool that spends charges needs `count * batch` of them - and `qualities_available` follows `quality_requirement::has` (`:932-940`), where an item at **or above** the level counts and the batch changes nothing because qualities are not spent. `can_make` asks all three, and the corpus gate is that **every one of the 6,291 recipes is makeable holding everything**. **Not ported:** the volume restriction and `check_enough_materials` - the reference's rule that one pile cannot satisfy two overlapping groups, which needs an inventory rather than a lookup |
| Consuming items | **partial, declared** | P3 | `consume_items` from `map::use_amount_square` and `item::use_charges` (`src/map.cpp:5877-5897`, `src/item.cpp:13135-13180`): items are taken **in the order they lie there** - spawn order, so part of the deterministic contract - and the caller learns both how much it got and whether that was everything asked for, because crafting needs to put back what it took when it fails. Seven tests: a partial take, taking exactly what is there, **asking for more than exists (three taken, request unsatisfied, square empty)**, other things untouched, an empty square, several piles taken in order, and asking for nothing. **The filter** is ported (round 589): `consume_items_with` asks it **before** comparing the type, because the reference's `visit_items` hands each item to the filter first (`src/item.cpp:13145-13149`), so a pile the filter rejects is skipped and the walk carries on — pinning that order matters, and the test shows a predicate being asked about a pile of another kind entirely. **And the liquid gate** (round 590): `consume_items_held` takes the square's `TFLAG_LIQUIDCONT` and the item types' phases, because `item_stack::use_charges` only offers an item to the per-item rule when it is **not a liquid** or the square holds liquid (`src/item_stack.cpp:146-160`) — a puddle on a plain floor is scenery, and the gate is asked **before** the filter, so a predicate never sees it. Both mutation-checked. **And contents** (round 591): the walk is the reference's `visit_items` — **pre-order**, an item offered before the things inside it, a rejected item skipped while its contents are still walked (`src/item.cpp:13140-13165`) — so a canteen on the floor is a source of its water, and an emptied pile is removed from its container while the container stays. The liquid gate is asked about the **top-level** pile only, which is why a canteen is usable where a puddle is not. Correcting this changed one craft test honestly: the parts a craft took are inside the unfinished craft, so asking the square for them finds them. **And the other half of the pair** (round 592): `map::use_amount` removes **items**, not charges — `quantity--` per item whatever it holds — so asking for one of a pile of twenty takes all twenty, where the charges path leaves nineteen. Callers need both: a recipe's `count` is items, a liquid is charges. The walk differs too, contents **first and bottom-up** (`contained_list.emplace_front`, `src/item.cpp:12877-12888`) with the container last, and consuming a container takes what is inside it. That last rule exposed a real bug elsewhere: despawning a parent left its contents in the world with an `Inside` pointing at nothing, so `remove_item_with_contents` now walks children first and both paths use it. Mutation-checked: reversing the contents order fails, and so does taking charges instead of items. **Not ported:** vehicle cargo and the infinite liquid sources a map can hold |
| Item weight | **partial: the base rule, contents, `NO_DROP`, the integral weight and the charge gate** | P3 | `item_weight` from `item::weight` (src/item.cpp:7229-7300`): the prototype's weight, times the charges **only for a charge-counted type**, plus everything inside a container, counted all the way down. Ten tests: a plain item, a stack, contents, nested containers, a destroyed item weighing nothing, a zero-charge stack counting as one object, charges that saturate rather than wrap, a `NO_DROP` container weighing nothing contents included (round 556), `item_weight_integral` reading the type's other number (round 557), and **the whole dataset replayed against the reference's own answers** (round 559): `corpus/items/measures.json` records `item::weight`, `weight(integral)`, `item::volume` and `volume(integral)` for all 9,912 types, and 9,846 of them agree to the gram. Weights are counted in **milligrams** because twenty-four charge-counted types weigh a fraction of a gram a charge, and charges multiply only under `count_by_charges()` - a torch with twenty charges weighs one torch. **Not ported, declared:** corpses (18 types differ), guns and magazines with mods or ammo in them (44), the `null` type, crafted items weighing their components, `REDUCED_WEIGHT` (no type carries it) and a relative-`weight` override |
| Faults | **ported and gated completely: all 54** | P2 | `cdda_content::faults` is `fault::load` (`src/fault.cpp:226-260`) and `class fault` (`src/fault.h:76-121`), and `adjust` applies a fault's modifiers the way the reference does — `( value + add ) * multiply`, per entry in **data order**, so two entries for one damage type compose. The gate compares **every field of every fault** (54) against `corpus/items/faults.json` (`--dump-faults`): name, description, fault type, item prefix and suffix, price modifier, degradation modifier, `affected_by_degradation`, both encumbrance numbers, blocked faults and both modifier lists. Three rules came out of the gate rather than the reading: **a fault's `name` is a translation object** (`{ "str": "Blown fuse" }`) while its `description` is a plain string, so the reader must take both; **`copy-from` inherits every scalar the child omits** (`fault_staff_short_broken` takes `degradation_mod: 50` and `affected_by_degradation: true` from `fault_handle_broken`); and **the two modifier lists are appended, not replaced** — the child's own entry comes **after** the parent's three, which is six of the 54 faults and a mutation of exactly that line fails three of them. **Which faults a type can catch is its own gate**: `corpus/items/item-faults.json` (`--dump-item-faults`) records the **resolved** weighted list for all **915** types that can acquire a fault, and the resolution is a finalisation pass (`item_factory.cpp:919-936`) rather than the type's own text: a direct entry adds its fault at its weight (100 when unstated, truncated from a float), and a `fault_group` entry either **replaces** the weight of each fault in the group with `( weight + weight_add ) * weight_mult` or adds each at one `weight_override` — replacing keeps the fault where it already stood while adding appends, so the order a type states its entries in decides the result. Mutating that replace into an append fails **14** of the 915 types. **Declared:** the `fault_fix` family (repairs, their requirements, skills and time), `fault_group` and its weighted lists, `random_of_type`'s die, and `item::tname`'s use of the prefix and suffix — the prefix is loaded and gated, but the displayed name is the UI's |
| Wearing an effect down | **ported and gated: 24 runs × 8 turns, and the three guards** | P3 | `ActiveEffect::decay` reproduces `effect::decay` (`src/effect.cpp:1042-1068`) over eight profiles worn for eight turns from three starting positions: no tick, a tick every turn, a tick every third turn, one that may reach zero, positive steps, and a duration-sized one. The rule does two things in one pass, and the guards are what make it worth recording: the intensity steps only while the effect is **short of its ceiling** (`get_max_duration() > get_duration()` — an effect at full length does not lose intensity yet; dropping that guard fails 6 of 192 turns), the tick divides the **absolute turn** rather than the effect's age (shifting its phase fails 7 of 192), a **positive** step is skipped when another effect blocks this one while a negative one always applies (letting it through fails the unit test), and the duration always falls by one unless the effect is paused. A type that lets its intensity reach zero is taken off on that turn and then **stops** stepping — the `intensity > 0` guard means it sits at zero for the rest of its duration, which the recording shows. **Declared:** the duration-sized recompute is *not* reachable in this grid — eight turns is too short for a ten-minute factor to cross a boundary — so that branch is pinned two ways instead: the `--dump-effect-merge` recording for the recompute itself, and a unit test for the fall (606 turns is two rungs, 600 is one). Also declared: `rem_ids`/`rem_bps` become one boolean here, `process_effects`' iteration and on-removal hooks, and the actual block map (`effect_is_blocked`), which enters as a flag |
| A per-turn effect on a body | **ported and gated: 252 cases, 99 of which move something** | P3 | `ActiveEffect::apply_to_body` reproduces the hunger and thirst branches of `Character::process_one_effect` (`src/character.cpp:10989-11006`) on a real avatar: three hunger levels, two thirst levels, arrival and non-arrival, three turns, seven effect shapes. Four rules hide in three lines. The amount comes from `get_amount` on the turn the effect **arrives** and from `get_mod` on every turn after — dropping that distinction fails. The chance is **skipped entirely** for a brand-new effect, so an effect acts at once whatever its tick says. The clamp answers a **movement**, so it is added rather than assigned — assigning it fails. And the two statistics are not symmetric: hunger caps at `MAX_HUNGER` and never floors, thirst floors at `-100`. **Declared:** only two of the reference's twenty-odd branches are assembled (the rest need fields the body does not carry yet: sleepiness, radiation, wetness, pain, stamina, focus); the roll back at `get_mod`'s stated maximum is pinned by a unit test on dice the test writes itself rather than by the grid, because **the oracle's dice plateau** — after two draws from a fresh seed it answers the same value however it is reseeded (95, 84, 10 / 71, 97, 10 / 46, 10, 10 from seeds 12345-12347, recorded by `--dump-rng-probe`) — so a recorded roll would be the plateau rather than a die. That defect is open and is recorded in `docs/11-oracle-harness.md` |
| An effect's average modifier | **ported and gated, 4,160 cases** | P3 | `effect::get_avg_mod` (`src/effect.cpp:1284-1296`) is `get_mod` without the dice, and it has a gate that reads wrongly from instinct: the mean is returned only when the stated maximum, **read as an integer**, is not zero — so a maximum of half a point states no range and the low comes back alone. The mean truncates toward zero. The corpus's 4,160 cases cover both branches and both resistance levels; the fractional maximum is pinned by a type assembled for it, because the shipped data never states one |
| When an effect kills | **ported and gated, 1,392 replays** | P3 | `effect::kill_roll` (`src/effect.cpp:1634-1645`): a list of `( numerator, denominator )` pairs indexed by **intensity** through `cap_to_size` (`src/effect.cpp:1107-1116`), which subtracts one and caps at the **last** entry — so intensity three of a two-entry list reads the second, and an intensity of zero underflows to the last entry rather than the first. An effect with no list never kills and draws no die. The data holds one such effect (`star_vampire_blood_drink_feral`, one in twenty); the recording replays all 1,392 rolls, and the cap itself is asserted on a list assembled for it |
| Stdlib dice | **ported and gated: 98 wrapper draws and the engine stream match the oracle's library** | P0 | The standard leaves `uniform_int_distribution`, `generate_canonical` and `normal_distribution` implementation-defined, and the two libraries disagree on **64 of 98 draws** (`corpus/rng/wrappers-libcxx.csv` against `wrappers-gcc.csv`, same program, same seeds). The port had been holding libc++'s numbers — the macOS library — while every recording it replays comes from a **gcc** oracle. All three are now libstdc++'s. `uniform_int` takes libstdc++'s branches: downscale (`scaling = urngrange / uerange`, reject, divide), upscale (two draws, wrapped, retried) and plain. `generate_canonical` is GCC 16's `__generate_canonical_any`, which is **not** the standard's prose: it builds an integer from two draws, divides it **with truncation** by `R^2 / 2^53 = 511`, and only then converts — the classic form gives `0.1315377…` where the reference gives `0.1317951…` for seed 1, wrong on the fourth digit of every real drawn. `normal_distribution` is the polar method answering the **second** value of the pair and keeping the first. All 98 wrapper rows and the raw engine stream match. **Declared:** the platforms disagree by construction — the port reproduces the Linux/gcc oracle, not a macOS build of the original, because CDDA itself promises no cross-platform dice; `docs/03-decisions.md` records the choice |

| Bounding a modifier | **ported and gated: 875 cases, all three outcomes** | P3 | `cdda_sim::effects::bound_mod_to_vals` reproduces `bound_mod_to_vals` (`src/cata_utility.cpp:199-208`), the rule every "the effect moves your hunger by three, but not past its stated ceiling" in `process_one_effect` goes through. Two of its rules read backwards from a clamp written from instinct, and the grid separates both. A ceiling or a floor of **zero means no limit at all**, not "clamp to zero" — treating it as a clamp fails 55 of the 875 cases. And the answer is the **movement**, not the new value, so the caller adds it back; the floor is then tested against a movement the ceiling may already have shortened, which is a trap of two cases (`value -5, movement 60, ceiling 5, floor 20`) that only shows when the ceiling comes first. **Declared:** the callers — which of the twenty-odd per-turn branches read it, and with which ceiling and floor (`get_max_val`/`get_min_val`, which come from the modifier table) — are not assembled into a per-turn pass yet |
| Whether an effect fires | **ported and gated: 14,760 cases on the reference's own dice** | P3 | `ActiveEffect::activated` reproduces `effect::activated` (`src/effect.cpp:1360-1394`) — the chance arithmetic plus the two things it does not have: the **clock** and the **dice**. The clock only lets the thing happen on turns the tick divides (`( when - turn_zero ) % tick == 0`), and a tick of zero means every turn while a negative tick means never. The dice are one uniform float compared against `mod / top`, or `top * mod / bot` when a bottom is stated. The recording holds **the die the reference rolled for each case** and the port replays it, so a disagreement is a rule that differs rather than two draws that happened to land differently; and each case also records whether the reference **took** a die at all, so a rule that rolls when it should not is caught even when the answer coincides. 1,797 of the 14,760 cases fire and 9,124 roll. Two things this recording caught: the port's dice-consumption assertion and the oracle's own flag were both needed — the first version of the harness detected "took a die" backwards (it compared the wrong draw, because `rng_float` pulls more than one engine value) and said 5,636 where the truth is 9,124. **Declared:** the reduced path (as with the percentage, `resists_effect` cannot be faked on a synthetic effect), `get_percentage`'s caller-side clamping inside `process_one_effect`, and the `mod` multipliers real callers pass, which here are one and two |
| Effect chances | **ported and gated: 112 types, 2,152 percentages, and 158 of them negative** | P3 | `ActiveEffect::percentage` reproduces `effect::get_percentage` (`src/effect.cpp:1324-1356`) — the arithmetic behind every "you vomit", "you cough" and "the pain rises" in the game. Four rules stack and each has a shape a reading gets wrong. A **value of zero with no stated chance answers zero**, not "always"; but a *missing* chance with a **non-zero** value divides a hundred by a top of zero and answers an **infinity**, which the reference does too and which the gate pins rather than papers over. A stated bottom switches the shape from `100 / top` to `100 * top / bottom`, so a bottom of zero and a top of zero mean opposite things. A **tick of zero means one** turn — most effects fire every turn and the member exists for the ones that do not — a tick above one divides, and a tick below zero is a separate refusal. And the intensity used is the **raw** one, unlike the amount readers, which use the effective intensity. The measured shape of the grid is asserted rather than assumed: 233 zeroes, 27 exactly one hundred, 1,734 between, and **158 negative**, because a chance or a bottom can be stated below zero and nothing clamps the answer — the first draft of this gate claimed "some above one hundred" and the recording said there are none. **Declared:** the reduced path is the same formula over the level-one cells, which `--dump-effect-mods` pins, and cannot be reached from a synthetic effect through `resists_effect`, so the recording holds the effect's own path only; `effect::activated`, which is this percentage plus the dice (`x_in_y`) and the tick's clock, is not ported yet |
| Effect modifiers | **ported and gated: 180 types, 1,081 cells, 4 intensities each** | P3 | The `base_mods`/`scaling_mods` tables behind every effect that has one — the data `process_effects` reads to change a character's speed, pain, hunger, thirst, healing and a dozen other things while an effect is worn. The shape is three-deep and the recording pins all of it: an **effect name** (`"PAIN"`, `"HUNGER"`, `"SPEED"`, …), an **action** (`pain_min`, `pain_max`, `pain_chance`, `pain_tick`, …) and a **reduction level** which is the index in the JSON array, so `pain_min: [ 1, 0 ]` is one at the base level and zero when the effect is reduced. Each cell holds a base and a scaling, read as `base + scaling * (intensity - 1)` (`get_mod_value`, `src/effect.cpp:563-582`). The thirty-one names and their 139 keys are transcribed from the reference's thirty-one `extract_effect` calls (`src/effect.cpp:333-560`) in the same order, so the two can be diffed side by side. Two mistakes the recording caught inside the first hour: the data writes whole numbers without a decimal point (`str_mod: [ -1 ]`), and collapsing the reduction level — the plausible reading, since the array *looks* like an intensity table — loses a quarter of the cells on `bile_irritant` and fails 1,185 of 1,081 value comparisons. **Declared:** what a `process_one_effect` pass *does* with these numbers (damage, the chance and tick rolls, the messages), `vitamins`/`limb_score_mods`/`enchantments` which are their own tables, and the `reduced` variant selection, which enters as a level |
| Wearing an effect | **ported and gated: 30 runs × 3 applications, and the one division that does not truncate** | P3 | `cdda_sim::effects::ActiveEffect` reproduces both halves of `Creature::add_effect` (`src/creature.cpp:1782-1856`) over six effect profiles applied three times in five sequences: the creation path (duration capped by the ceiling, intensity capped by the ceiling, a duration-based type deriving its intensity instead) and the merge path (duration grown by `dur_add_perc` percent of the new one, then intensity jumped to the one asked for, or stepped by `int_add_val`, or left alone, in that order of preference). The **one division that does not truncate** is `ceil( duration / int_dur_factor )`: `time_duration / time_duration` answers a `double` (`src/calendar.h:317-319`), so 21 minutes against a ten-minute factor is intensity **3**, where truncation would say 2 — the grid has a sequence of 7, 13 and 1 minutes precisely to part them, and dropping the ceil fails 4 of 90 applications. Beside it, `dur * dur_add_perc / 100` **is** integer arithmetic: three turns at fifty percent add one, not two, and a wrong divisor fails 28 of 72. Intensity cannot reach zero unless the type sets `int_decay_remove`, so a decaying effect sits at one rather than vanishing. **Declared:** body parts (every application here lands on the torso), `deferred` applications and the effect processing that follows an add (`process_one_effect`), immunity (`is_immune_effect`, `immune_bp_flags`, `CANNOT_GAIN_EFFECTS`), the block check against effects already worn, `on_effect_int_change` observers, and decay itself (`effect::decay`) |
| Effect types | **ported and gated: 451 loaded types × 21 fields, plus the 113 minted ones named** | P3 | `cdda_content::effects` loads the 451 effect types the core data states, field for field against the reference's own registry: intensity ceiling, duration ceiling, the four decay settings, the eight boolean switches, the rating, the flags and the four resist/remove/block lists. The family is unlike every other loader here: it is **not** a `generic_factory`, so there is no `copy-from` to merge — each object is read flat with a default for every member it omits — and that is also why the prototype digest could not see it, since the digest walks the `generic_factory` registry and effects are a plain `std::map` (`src/effect.cpp:58`). The harness now appends them by hand, so `prototype-digest.json` has **91 registries** where it had 90. Four defaults were wrong on the first run and the recording said so: `max_intensity` is **1**, `dur_add_perc` is **100**, `int_decay_step` is **-1** and `show_intensity` is **true** when the data is silent. One member is not data at all: **`impairs_movement`** is a lookup in a hardcoded set of ten trapping effects (`src/effect.cpp:1497-1509`), so a JSON member cannot set it — the port keeps the same set and mutation-checks it (removing it fails 9 types). **Declared:** the 113 `mabuff:` effects the reference **mints** one per martial-art buff (`register_ma_buff_effect`) and the named gap is exactly that count, the text (names, descriptions, memorial logs, messages) which is interface rather than rule, `base_mods`/`scaling_mods`' deep per-intensity modifier map, `vitamins`/`limb_score_mods`/`enchantments`/kill chances, and `max_effective_intensity` |
| The hunger effect | **ported and gated: 126 character states, and the ladder closes `update_stomach`** | P3 | `cdda_sim::body::choose_hunger_effect` reproduces the tail of `Character::update_stomach` (`src/character_body.cpp:1019-1099`) and the two volume tests it runs on (`stomach_contents::would_be_engorged_with` / `would_be_full_with`, `src/stomach.cpp:373-385`), over seven fillings of a 250 mL stomach × three ages of the last bite × six body-fat levels. Two things the recording settled and reading had got wrong. **The fullness ratio is an integer division**: `units::volume / units::volume` answers the underlying `int64` (`src/units.h:253-257`), so every threshold — five sixths, three quarters, eleven twentieths — collapses to "at or over capacity", and a float ratio makes the gate fail. Because of that truncation **`hunger_full` can never be chosen** (engorged is tested first and fires at the same ratio) and **`hunger_starving` can never be chosen** either (the reference tests `emaciated` 1.0 *after* `underweight` 2.0), so two of the nine rungs are dead code — kept in the reference's order so the port cannot quietly "fix" them, and asserted unreachable by the gate. The two ladders are also genuinely different: a body whose last bite is over ten minutes old is read in eighths and halves of capacity and bottoms out on body fat; one that has *just* eaten is read in quarters, cools to `blank` at full calories, and bottoms out on whether it is overweight. The body fat itself (`get_bmi_fat`, `:6356-6360`) is now computed in the port from stored calories, height and lean size, and `KCAL_PER_KG`'s 3,500 kcal/lb is one constant. The effects it installs are real now: the worn hunger effect carries a **duration** that runs (`HungerEffect` is an `ActiveEffect`, 24 hours, intensity one), it is **not** refreshed while the character already has the one it should, and when the day ends it is taken away and installed again on the next update. **Declared:** body parts (every application here lands where the effect registry does not distinguish), `process_effects`' modifier application, and `is_hibernating`, which the sleeping path would need |
| The metabolic rates | **ported and gated: 72 character states, each function compared separately** | P3 | `cdda_sim::needs` reproduces the four functions everything else in the food clock is measured against — `get_starvation` (`src/character.cpp:4657-4670`), `metabolic_rate` (`:770-790`), `base_bmr` (`:6514-6527`) and `get_bmr` (`:6538-6542`) — plus `multi_lerp` (`src/cata_utility.cpp:267-288`), which they are all curves over. The recording calls **each function separately** as well as recording the state it read, so a disagreement says which of the four is wrong instead of only that the rates came out different. 72 states: six hunger figures from engorged to past famished, three calorie stores from full to half, four activity levels. Three rules the gate settled: **`set_hunger` caps hunger at 300**, so a grid case that asked for 600 is a character at 300 and a metabolic multiplier of exactly 1 — reading the requested figure straight through was the gate's first failure; the activity level is **rounded up** to the next whole step (flooring it fails 4 states, which is the honest count for a grid this small); and `base_bmr`'s terms each **truncate** — `6.25 * height` assigned to an `int`, and the product truncated on the way out. The bodyweight is recorded **per case** because starving a character changes it, which moved `base_bmr` from 1,738 to 1,631 across the grid. **Declared:** the sleeping path (`calc_sleep_recovery_rate`: halves both rates and reads every body part's temperature), the `ROOTS2`/`ROOTS3`/`CHLOROMORPH` halving, `TRANSPIRATION`'s weather term, the enchantment modifiers, and `get_speed`/`bodyweight`/`height` themselves, which are recorded inputs |
| What the body does with a meal | **ported and gated: 1,686 comestibles × 6 character states, every number the rule moves** | P3 | `cdda_sim::body` reproduces `Character::update_stomach` (`src/character_body.cpp:935-1019`) by calling the **real rule**, not a paraphrase of it: `--dump-update-stomach` runs `update_stomach` on a fresh pair of compartments holding one bite of each comestible, in six profiles — three at a 1,500 mL capacity and three at the **250 mL a character actually has** (`max_volume` defaults to zero and `capacity()` floors it at a quarter of a litre) — fed and just bitten, fed and bitten forty minutes ago, engorged and thirsty, and famished. **The order is the rule**: both compartments digest; water that left the stomach becomes thirst at five millilitres a point; a **drug** vitamin skips the guts; what the stomach released goes into the guts; only what the **guts** release becomes stored calories and vitamins, which is why a meal eaten now does not feed you now; then the body burns its basal rate and gets hungry. The fullness ladder is **two different ladders** — a stomach that has held food for over ten minutes reports fullness as `full`, `1/2` and `1/8` of capacity, while one that was just filled reports `3/4` and `1/2` and cools a hunger of zero — and the settled one ends in the "there's no food except what you have stored in fat" branch at 300, which the `water` traces reach. **Only one vitamin in the content can be held positive at all**: `iron`'s ceiling is 3,600 units, `calcium` and `vitC` state a ceiling of **zero**, and the other fifty are markers — so 72 of the 10,116 traces store anything, and that is the honest count. The two dice (`roll_remainder( rates.hunger * five_mins )` and thirst) are the rule's only randomness: the harness seeds the engine, records the rolls, seeds it again, and the rule then rolls the same numbers, so the trace holds both the roll and the effect it had. **Declared:** the hunger *effect* ladder that follows (`hunger_engorged` / `hungry` / `near_starving`), `has_calorie_deficit()` and the BMI categories it reads, the activity-history bookkeeping, and the vitamin `decays_into` cascade on a negative change. **Oracle note:** `character.h` gained a `get_stored_calories()` getter in the harness patch, because the food clock adds **sub-kilocalorie** burns and the displayed figure would hide them |
| The stomach emptying itself | **ported and gated: 1,686 comestibles × 2 compartments × 6 steps, every number** | P3 | `cdda_sim::digest` reproduces `stomach_contents::digest` (`src/stomach.cpp:400-437`) and `get_digest_rates` (`:440-476`), and the recording is a real trace: a bite of each comestible put into a 1,500 mL stomach and a 1,500 mL set of guts, then six half-hour digests of each at 2,500 kcal and a hunger of one. **The two compartments are different machines wearing the same class.** The stomach passes material on at a sixth of its **capacity** per half hour with a flat 5,000 thousandths-of-a-calorie minimum; the guts absorb at 5% of the metabolism per half hour with a minimum of `floor( kcal / 24 · hunger · 1000 )`. Water moves in **five-minute** steps at 250 mL, so the caller passes both counts, and a tick with no half hour in it moves water alone. Three branches are counted by the gate rather than assumed: **354** foods carry negative water and have it taken back out of the stomach (`min( water, rate )` passes the negative through), **719** steps released exactly the flat minimum, and the **12** foods carrying a drug vitamin skip the trickle — the stomach dumps a drug whole so a dose is a dose. **Declared:** the `STOMACH_SIZE_MULTIPLIER` enchantment, the tapeworm and `activate_consume_eocs` before ingestion, and everything downstream of digestion (`update_stomach`'s hunger, thirst, stored kcal, vitamin counts and the guts-feeds-the-body ordering) — that is the next slice |
| What a bite puts in the stomach | **ported and gated: 1,686 comestibles, every field of the summary** | P3 | `cdda_sim::food_summary` reproduces the four calculations between an item and the stomach (`src/consumption.cpp:1689-1714`), and `stomach::ingest` (`src/stomach.cpp:392-398`) adds the result unchanged, so this recording *is* the stomach after one bite: **the calories a body actually gets** (`:223-258` — a quarter off `RAW` uncooked food, half off `CARNIVORE_OK` food for a carnivore, and rot sliding linearly to nothing at halfway-rotten); **the vitamins**, converted from the data's **RDA percentages** to internal units on each vitamin's own `rate` (`src/vitamin.cpp:122-129`: `24h · percent / 100 / rate`, 0.96 units per percent for the three vitamins, and **only** for the three — 50 of the 53 types in the content are `counter`, `drug` or `toxin` and are already in units); **the water**, `quench · 5 mL` in both directions, which is how a dry food makes you thirstier; and **the room it takes up**, which is neither its volume nor its weight — the dry bulk is crunched to its dry weight when it is less dense than water (1 g = 1 mL) and then multiplied by calories per gram, **square-rooted above three**. Three input rules the gate had to get right before any of that compared: the port had item **weight in milligrams but documented as grams** (now documented honestly, and the gate divides by a thousand exactly as `units::to_gram` truncates); a charge-counted item's volume is its type's **divided between `stack_size` and rounded up**, where the stack size falls back to the **comestible's `charges`** and not the item-level `count` — 219 drinks were a quarter too big until `effective_stack_size` followed `charges_default()` (`src/itype.cpp:139-149`); and an item flagged **`NO_DROP` weighs nothing at all**, which is acetylene's 40 g of data becoming a zero-gram item. **Declared:** item components (`made_of_any_food_components`, `BYPRODUCT`, `NUTRIENT_OVERRIDE`), the carnivore vitamin blacklist (the port has the flag, not the list), enchantment `KCAL` and `VITAMIN_ABSORB_MOD` modifiers, mutation `vitamin_absorb_multi`, the tapeworm halving, and `activate_consume_eocs` — none of them reachable from the content the port has loaded |
| The enjoyment of a bite: `Character::fun_for` | **ported and gated: 37,092 recorded pairs — 1,686 comestibles × 22 eater states — plus 12 transcribed cases** | P3 | `cdda_sim::fun_for::fun_for(&Food, &Eater) -> (i32, i32)` is the reference's `Character::fun_for` (`src/consumption.cpp:483`) as a **pure function**, which is what makes it gateable before the systems that will feed it exist. The upstream suite `tests/food_fun_for_test.cpp` is transcribed case for case — sick, rotten, cold, melted, species food, gourmand, monotony, taste blocker — plus the three branches upstream never reaches: `HEMOVORE_FUN`, the bloodfeeder's half-interest in ordinary food, and `bio_faulty_grossfood`. The order of the blocks is the reference's and it matters: the **ceiling** (`fun * 3` or `fun * 6`) is computed *before* the temperature, the species flags, the blood rules and the gourmand, so a species food's ceiling keeps the unhappy number the bite used to have, and the gourmand's ceiling is the triple of the *penalised* fun. Two more things only a careful reading gets right: the taste blocker compares the eater's power against the food's **stated** fun rather than the bite they would have had, and needs **strictly more** than it, so a rotten garlic still costs exactly its own 15 kJ; and the monotony loop stops at zero — the penalty can never make food taste bad — unless the food carries `NEGATIVE_MONOTONY_OK`. A separate test loads the corpus and checks the six properties the transcriptions assume (garlic below -1, cola `EATEN_COLD`, ice cream `MELTS`, cat food `FELINE`, dog food `LUPINE`, toast-em worth eating), so the cases cannot pass while testing nothing. `--dump-fun-for` now records the exact pair in each of 22 eater states, and that recording settled three things the inequalities could not. **A food's taste is the item's, not the slot's**: `item::get_comestible_fun` (`src/item.cpp:8041-8056`) adds every flag's **`taste_mod`** — one flag in the whole content has one, `BAD_TASTE` at −5, which is why 22 mutant foods read −10 in the data and taste like −15 — and floors a **`MUSHY`** item at `min(-5, fun)`. **Rot is not a number you can set**: `set_relative_rot(1.5)` on a food that does not spoil leaves it at **0**, so the recording carries `rot_set` per food rather than assuming the value asked for. And the sweeps agree **exactly** on all 37,092 pairs, including the interactions nobody would write a case for (a cold cola's ceiling staying at 15 while its bite doubles, a taste blocker still charging a rotten garlic only its stated 15). `json_flag` gained a loader for `taste_mod` — the first time the port has needed a flag to carry a number rather than a decision. **Declared:** the `ignore_already_ate` argument (nothing needs it yet) and `make_component_hash`, which the port compares as a plain number because the component system is not ported. Mutation check: dropping the `taste_mod` sum fails exactly the 22 `BAD_TASTE` foods |
| The comestible slot, the rest of it: vitamins, addictions, contamination, the material list | **ported and gated: 1,686 comestibles × 25 fields, plus 193 materials** | P3 | `Comestible` now carries `vitamins`, `addictions`, `contamination`, `freezing_point`, `primary_material` and `rot_spawn`, and the gate compares 25 fields per type. Four rules only the recording could settle. **A vitamin's amount may be a mass**, read on the vitamin's *own* scale: `vitamin_units::mass` counts **micrograms** (`src/stomach.h:22-35`) and `units_from_mass` is `mass / weight_per_unit` (`src/vitamin.cpp:136-144`), so `["vitC", "20 mg"]` is **21 units** on that vitamin's 938 μg scale, and the item mass reader — which counts milligrams and has no micrograms at all — cannot read it. **`addiction_type` is a bare string 113 times**, and a bare name takes the item's own `addiction_potential`, which the reference reads two lines earlier (`src/item_factory.cpp:3432-3435`). **A comestible that states no vitamins derives them from its materials** — `max(healthy,1) * 10`, each material contributing `vitamin * scale / materials.len()` **ceiled separately** (`:762-776`) — which is why 169 prototypes carry a 53-entry map and 12 of them carry real numbers. And **a stated `primary_material` replaces the material list outright** (`:463-467`), which is why `mead` - materials `["alcohol", "honey"]`, primary material `alcohol` - has no honey vitamins: eleven prototypes are like that. Two ordering facts the gate caught and the port had wrong at first: the derivation runs **per prototype on the raw inherited slot** (a child derives from its own materials, not from its parent's derived numbers - `blood_mi-go` was the case), and `extend.vitamins` adds to the inherited map (`protein_shake`). `--dump-material-vitamins` was added as its own recording because a disagreement about a food has to be chased to the material data before it can be called a derivation bug. **`contamination` is a probability map** — the reference's weighted reader takes a **1.0 default** (`:3427-3428`), so `water`'s `0.1` for `highly_contaminated_food` is one in ten, not one-tenth of something; nine prototypes carry a disease. **`rot_spawn` reads from the defaults, not from the parent**: a prototype that states it gets every member read with `was_loaded = false` (`src/item.cpp:16200-16206`), so the egg abstract stating only `"chance": 0` carries **no monster** and its children each restate theirs — 133 prototypes spawn something when they rot. **`monotony_penalty` is the one member whose default is computed from the material list**: it starts at `-1` and becomes **0 when any material is `junk`**, else 2 (`src/item_factory.cpp:495-497`) — 1,589 prototypes take the 2 and the rest state their own. **Declared:** `specific_heat_liquid`, `specific_heat_solid` and `latent_heat`, which the same finalisation averages over the material **portions** (`mat_total`, `:476-492`) and which nothing yet reads, `consumption_effect_on_conditions` (inline effect-on-conditions, **zero** uses in the core content — the recording pins the count rather than a comparison), `rot_spawn`'s `chance` and `amount`, which the reference leaves **indeterminate** for the prototypes that state no `rot_spawn` at all so the recording cannot read them safely, `numeric_bound_reader` errors, and the allergy **flags** `set_allergy_flags` adds, which are flags rather than vitamins |
| The comestible slot: 15 members, and the reader's own arithmetic | **ported and gated: 1,686 comestibles, 15 fields each, `nutr` derived** | P3 | `ItemType::comestible` is `Option<cdda_content::comestible::Comestible>`, read from the item object the way `islot_comestible::deserialize` reads it (`src/item_factory.cpp:3405-3441`) and gated on `--dump-comestibles`. Three things in it are not in the JSON at all. **The slot merges on `copy-from`** — `load_slot` reuses the parent's object and reads into it (`:2859-2868`) — so a child that states only `calories` keeps its parent's `quench` and `spoils_in`, which is the opposite of a stated `flags` array. **`proportional` and `relative` apply to every one of these numbers**, in that order and *instead of* the stated value (`src/generic_factory.h:1251-1259`), and `proportional` multiplies in `double` with the compound assignment truncating toward zero: `offal.parasites` is meat's 32 halved to **16**, `hide_bag.quench` is raw leather's -2 scaled by 6 to **-12**, `buttered_toast.fun` is toast's 2 plus **4**. **`calories` is multiplied by 1,000 at finalisation** (`:460`), and `nutr` is derived — `kcal / ( 2500 / 288 )`, an `int` divided by a `float` (`src/itype.h:231-239`) — so the port keeps kilocalories and multiplies in an accessor; doing the multiply during the load instead multiplied inherited values twice and the gate named all 54 types. **Declared:** `vitamins`, `addiction_type`, `contamination`, `rot_spawn`, `consumption_effect_on_conditions`, `primary_material`, `freezing_point` and `specific_heat`\* (the recording carries vitamins and addictions, and they are the next round), the `numeric_bound_reader` errors on `charges`/`parasites` (no prototype trips them, so the bound is documented rather than enforced), and the behaviour of eating, which is `MealTally`'s own gate |
| Examine actions: the name, and the four actors with data | **ported and gated: 1,769 tiles, 74 bare names, 80 object tiles** | P3 | `MapData::examine` holds a `cdda_content::examine::ExamineAction`: `Named` for the **73 hardcoded functions** the content spells as bare strings (`src/iexamine.cpp:7741`), and an actor for the **4 that carry data** — `appliance_convert` (38), `cardreader` (21), `effect_on_condition` (20), `mortar` (1, kept by name only). The defaults are the behaviour: a card reader's `consume_card`, `allow_hacking`, `despawn_monsters` and `query` are all **true** and its `radius` is **3** (`src/iexamine_actors.cpp:241-261`), which a fixture pins because a corpus of tiles that state every member would hide them. Two facts the recording settled: the reference's **default function is `iexamine::none`**, so 1,208 of the 1,769 tiles are `none` by default and **no prototype in the content writes `"examine_action": "none"` at all** — the dump cannot tell a statement from a default, and neither can the port. And **no prototype states an examine action over a parent that has one**, so replace-vs-keep is unobservable in the corpus and is pinned by a fixture instead (`optional` overwrites, the same rule `use_action` taught in round 629). **Declared:** the examine actors' *behaviour* (all of it needs a character standing on the square), `mortar`'s dialogue conditions, and the card reader's own `mapgen_id`/radius exclusivity, which the port keeps as data and leaves to the caller |
| Whether a `transform` action may be used at all | **ported and gated: 459 transforms × 3 standings = 1,377 verdicts, qualities message included** | P3 | `cdda_sim::transform::can_use` is `iuse_transform::can_use` (`src/iuse_actor.cpp:428-475`): the conditions are asked in a **fixed order** and the first failure is the answer, so the variant matters as much as the text — an item that is neither worn nor wielded reports the **wearing** rule even when it also has no charges. The refusals carry the reference's own sentences, which is how the port knows its `need_charges_msg` default (`"The %s is empty!"`) is only one of six in this content (`"Your internal reservoir is empty."`, `"The %s's batteries are dead."`). The gate compares the **template**, because the recorded sentence has `item::tname` formatted into it and that is a later P3 piece: the recorded text must begin with what precedes the placeholder and end with what follows it, handling both spellings (`%1$s` from the three literals, `%s` from an actor's own message). The `qualities_needed` rule is ported too, message and all: it asks the character *and* everything within one tile (`has_quality` over an inventory formed from the map), and then names each unmet quality as `"<name> <level>"` joined by an **Oxford comma** — so the three quality-bearing actors' nine verdicts are compared **whole** rather than as templates, because that sentence carries no item name. **Declared:** the worn-mutation conflict (`438-451`, needs the mutation layer), the broader rule that a character's own qualities and a one-tile inventory are two separate sources (the gate supplies one predicate for both, and only the "nothing available" side is exercised), and the **order** between `need_worn` and `need_wielding`, which this content cannot distinguish because no actor states both — that order is pinned by a transcribed unit test beside the code instead |
| What a `transform` action **does** to its item | **ported and gated: 459 transforms, item-level half, 4 declared container shapes** | P3 | `cdda_sim::transform::apply` is `iuse_transform::do_transform`'s item half over `item::convert` (`src/iuse_actor.cpp:347-419`, `src/item.cpp:660-693`), gated on a recording that builds each of the 459 items, hands it to the reference's own `do_transform`, and compares type, charges, count-by-charges, active, countdown and item counter. The surprises are all in `active`, which is a **union of three unrelated conditions**: the actor's own flag, the target **having a temperature** — `is_comestible() && !NO_TEMP`, which is why a liquid result switches the item on and 11 transforms differ without it — and a timer existing at all, which covers `target_timer` and the target type's own `countdown_interval` (read from `"countdown_interval"`, 0 for every transform target in this content, so it is **declared untested** rather than claimed). Charges survive unless the charge-per-count-ness changes, in which case `charges_default()` replaces them, and the actor's `ammo_qty` overwrites afterwards. **Declared:** the 4 transforms whose result goes **inside** a container (pocket insertion plus sealing), `target_group` results (none in this content), `random_ammo_qty` (none), and the corpse half of `has_temperature` (the port has no corpse type and no transform produces one) |
| Ammunition effects, and the registry that does not police them | **ported and gated: 736 rounds' effects, 82 defined effects, 4 documented ids that are really flags** | P2 | A round's `"effects"` is a container like any other: **cleared when stated**, `extend`ed and `delete`d otherwise (`auto_flags_reader`, `src/item_factory.cpp:2990`). Both halves were missing, which cost **238 of 736** types — every round that inherits its effects and extends them (`bp_10mm_fmj` had one effect instead of four). The reader also asked the *object* whether it declares `"subtypes": ["AMMO"]` rather than the **item**, so a child that inherits its ammo slot lost the whole block; the content happens not to contain such a child, so that half is pinned by a fixture instead of the corpus. The temptation to refuse effects the registry lacks is **wrong**: `ammo_effect_str_id` is a plain string that nothing validates at load (`src/ammo_effect.h:16-32`), and the content proves it by listing four **flag** names among ammunition effects (`ALLOWS_BODY_BLOCK`, `HURT_WHEN_WIELDED`, `STREAM_GAS_INSECTICIDAL` on `rock`, plus `CASELESS_ROUNDS` from the 0.I migration file). A stricter port would not load the content at all, so the gate pins the slack. **Declared:** what an effect *does* (its burst radius, its fields, the projectile path) — only the ids are ported |
| Use actions: the map, its order, and what a type lends itself | **ported and gated: 2,653 types' action lists and 1,054 charge scales** | P3 | `cdda_content` keeps `use_actions` as a `BTreeMap` keyed by action name, because the reference keeps `use_methods` in a `std::map` (`src/itype.h`) — and **three behaviours follow from that alone**: the actions come back **sorted by name** rather than in data order, a repeated name **collapses** (the first wins, `emplace`), and a `repair_item` actor is registered under its **`item_action_type`**, so a repairkit answers to `repair_metal` (`src/item_factory.cpp:1858-1865`). Inheritance is **replace, not merge**: the container overload of `optional` **clears** the member when it is present and only then inserts, falling through to the separate `extend`/`delete` objects when it is absent (`src/generic_factory.h:1168-1192`) — an earlier merge disagreed on **31 of the 2,653** types, and the same mistake had already put four types' actors on the wrong side of the split. The other half is what a type **lends itself** at finalisation (`src/item_factory.cpp:333-377`, `808-810`): a quality at or above a usage's level lends that usage's actions **at a charge scale of zero** (336 types), `charged_qualities` lend them without touching the scale, a mod slot lends `GUNMOD_ATTACH`/`TOOLMOD_ATTACH`, a gun lends `detach_gunmods`/`modify_gunmods`, and anything that can plug in gets `link_up` at zero whether or not its actor says so. `ammo_scale` is a map whose **zero is a real zero**, not "unset" — every scale in this content is zero, which is exactly the case a port that blurred the two would never notice. **Declared:** the reference's `usage_from_string` table (the port accepts a usage's action id without asking whether the reference has a function for it), `delete`'s container forms beyond `use_action`, and every actor's `use` |
| Use actions: the four shapes of an entry | **ported and gated** | P3 | `read_use_actions` is `use_function_reader_helper` (`src/item_factory.cpp:1877-1903`). An entry is a **string** (a legacy id), an **object** (an actor with a `"type"`), or an **array** — `[ "CROWBAR", 0 ]`, which names the action and overrides its `ammo_scale` (`1894-1899`) — and the member itself may be a single entry or an array of them. The array shape is the one that gets lost: a reader that accepts only strings and objects **drops those entries without an error**, and dropping the branch leaves `large_repairkit` with three actions instead of five (`GUN_REPAIR`, `repair_item`, `link_up`) — which is what the test reports. **Declared:** the `ammo_scale` override itself, which the port keeps as a bare name until an actor that consumes it is ported |
| Use actions: `deploy_furn` and `place_trap` | **ported and gated completely: 39 and 28** | P3 | `cdda_content::use_actions::{DeployFurn, PlaceTrap, TrapData}` are `class deploy_furn_actor` (`src/iuse_actor.h:404-421`) and `class place_trap_actor` (`992-1030`) with their loaders (`src/iuse_actor.cpp:1229-1232`, `4166-4187`), gated per item type against `corpus/items/deploy-furn.json` and `corpus/items/place-traps.json`. `deploy_furn` is **one mandatory member** — the reference calls `get_string` rather than `read`, so an actor without `furn_type` is a content error and the port returns one instead of an empty id. `place_trap` has the shape worth pinning: **two halves at two levels**. `unburied_data.load( obj )` reads the **actor's own object**, while `buried_data` reads a nested `"bury"` object **only when `bury_question` is non-empty** — so the same four keys (`trap`, `done_message`, `practice`, `moves`) mean different things at the two levels, and dropping the question check leaves the two buryable traps (`beartrap`, `landmine`) with the unburied trap and a cost of 100 instead of `tr_beartrap_buried` at 275 moves. Both halves `assign`, so a missing member keeps its own default: `tr_null`, an empty message, no practice, **100 moves**. **Declared:** both actors' `use` — the placement checks (solid neighbour, neighbouring terrain, underwater, under the player) and the deploy itself |
| Use actions: the `holster` actor | **ported and gated completely: all 76, with its one behavioural rule** | P3 | `cdda_content::use_actions::Holster` is `class holster_actor` (`src/iuse_actor.h:790-812`) and its loader (`src/iuse_actor.cpp:2905-2909`), gated per item type against `corpus/items/holsters.json` (`--dump-holsters`). The actor carries **no storage data at all** — the pocket stores, which is why 45 of the 76 state their messages and the rest leave them empty — so the data gate is two translations, and the part worth a gate is the behaviour: `can_holster` (`src/iuse_actor.cpp:2911-2914`) is `!obj.active && holster.can_contain( obj ).success()`. That adds exactly one rule to the pockets — **an active item cannot go into a holster even when it fits**, so a lit torch stays in hand — and the test covers all four combinations of active and fits, with dropping the `!active` failing the one that says so. **Declared:** the item's `active` flag as a component (the rule takes it as an argument until there is an active-item concept), and `holster_actor::store`'s move-and-charge bookkeeping |
| Use actions: the `firestarter` actor | **ported and gated completely: all 42** | P3 | `cdda_content::use_actions::Firestarter` is `class firestarter_actor` (`src/iuse_actor.h:481-511`) and its loader (`src/iuse_actor.cpp:1460-1465`), gated per item type against `corpus/items/firestarters.json` (`--dump-firestarters`). Three fields and one **derived default**, which is the whole reason this actor is worth a gate: `moves_slow` falls back to **`moves * 10`, computed from the fast cost after that has been read** — so a firestarter that states only `"moves": 50` has a slow cost of **500**, not the member's 1,000. Falling back to the member default instead fails **7 of the 42** (`electric_lighter` 25→250, `lighter` 50→500) and the one-line rule test as well. The corpus exercises both branches — 25-odd derive, 13 state nothing at all (which lands on 100/1,000 by luck, so the test with a fast cost that is *not* 100 is the one that matters), and several state a slow cost that is **less** than ten times the fast one (`matches`: 40/1,000), which a derived-only reader would get wrong. `need_sunlight` marks the five magnifying glasses. **Declared:** the actor's `use`, which picks a fuel type and resolves the fire |
| Use actions: the `link_up` actor | **ported and gated completely: all 108** | P3 | `cdda_content::use_actions::LinkUp` is `class link_up_actor` (`src/iuse_actor.h:1160-1182`) and its loader (`src/iuse_actor.cpp:4917-4925`), gated per item type against `corpus/items/link-ups.json` (`--dump-link-ups`). **The defaults are the behaviour**, because 106 of the 108 cables never state them: `efficiency` is **0.85**, so a cable nobody described still loses a sixth of the power it carries, and `move_cost` is **5**, so attaching one still costs a moment; `cable_length` **-1** means "as long as the item's own limit", not "no cable"; and `targets` defaults to **`{ no_link, vehicle_port }`** — not empty — so a cable that names none may still be plugged into a vehicle port. Shrinking that default to one state fails **101 of the 108**. `charge_rate` is a power written in the game's units (`"1 kW"`, `"35 W"`, `"0.5 kW"`), read by [`parse_watts`] and gated by a second test on each suffix. `efficiency` is a **`float`** in the reference and stays `f32` here, compared bit-exactly (0.85f32 is 0.850000024 printed, which is what the dump shows), and `targets` is compared as a **set** because the reference orders by the enum's value and the port by name. **Declared:** the actor's `use` (the whole cable dance), `menu_text`'s default, and the vehicle-side link states |
| Use actions: the `consume_drug` actor | **ported and gated completely: all 108** | P3 | `cdda_content::use_actions::ConsumeDrug` is `class consume_drug_iuse` (`src/iuse_actor.h:247-283`) and its loader (`src/iuse_actor.cpp:849-871`), gated per item type against `corpus/items/consume-drugs.json` (`--dump-consume-drugs`). The actor is a **list of consequences** rather than one action, so almost every field is a collection: effects with a duration and a body part, fields produced in the world, stat adjustments, damage over time, and the item left behind (`null` by default, which is the reference's `NULL_ID` **whose name is the string "null"**). Three rules the gate pinned: `vitamins` are a **`std::map`, sorted by id**, not a list in file order — reading them into a list disagreed on **30 of the 108** drugs, and the reference `emplace`s so a duplicate id keeps the **first**; a vitamin entry `[ id, lo ]` is a range of one, because `hi` falls back to `lo` (`src/iuse_actor.cpp:866-870`); and `moves` defaults to **100**, a whole turn, rather than zero. An effect's `duration` is read as a **string** through the game's duration grammar or as a whole number of **turns**, both landing in seconds (`src/iuse_actor.cpp:839-844`). A second test keeps the dispatch honest: `consume_drug` must never appear as an unported name, or every drug's consequences would vanish while the drug gate still passed. **Declared:** `bp` is `bp_null` in every entry of this corpus and no effect is `permanent`, and `damage_over_time` is empty everywhere — all three branches are read but unexercised, so their counts are **asserted at zero** rather than assumed; and the actor's `use` |
| Use actions: the `transform` actor | **ported: the actor's data for all 459 actions, 431 `transform` and 28 `delayed_transform`, the latter with its own two fields, plus the actor's own message** | P3 | `cdda_content::use_actions` is `class use_function`/`class iuse_actor` (`src/iuse.h:279-300`) and `class iuse_transform` (`src/iuse_actor.h:44-110`) with its loader (`src/iuse_actor.cpp:216-250`), gated per **item type** against `corpus/items/transforms.json` (`--dump-transforms`) — so a transform that landed on the wrong type is a disagreement, not something "found somewhere in the content" hides. `delayed_transform` is **not a different shape**: the reference loads it with the same loader and adds a `not_ready_msg`, so one reader covers both. Rules the gate pins: `sealed` defaults to **true** (an actor that says nothing means a sealed container — flipping that default fails all 459), `cost` and `target_charges` default to **-1** (meaning "the item's own"), `variant_type` to `<any>`, and `target_timer` is read in the game's duration grammar as seconds. **The whole loader is read, field by field** — 22 members: the message (`"msg"` → `msg_transform`), the target and its group, the container and its sealing, the charges and ammunition, the timer, the conditions (`need_worn`, `need_wielding`, `need_empty`, `need_charges`, `need_fire`), `moves`, `menu_text`, and `qualities_needed` (which 3 transforms state as `SCREW_FINE: 1`). Three clamps and two default sentences come with it (`src/iuse_actor.cpp:246-268`): a **negative `moves` is a content error** while `need_fire` and `need_charges` are **lifted to zero**, so a data error becomes a satisfied requirement rather than an impossible one; and the actor supplies the sentences themselves — **`"The %s is empty!"`** and **`"You need a source of fire!"`** — when the data says nothing, which 391 and 458 of the 459 respectively do. Emptying that default fails both the corpus gate and the unit test. `cost` is **not** a field here, because `iuse_transform` never reads one — `iuse_actor::cost` stays at the member's `-1` for every transform in the content, and reading a `"cost"` the reference ignores would be a rule of the port's own invention. **`delayed_transform` adds two fields and derives**, which round 626 closed: `delayed_transform_iuse::load` calls the parent's loader and then takes `not_ready_msg` and `transform_age` **in turns** (`src/iuse_actor.cpp:992-997`), so treating it as a plain `transform` — as the port did for a round — loses the age a ferment waits for and the message that says why it cannot be used yet. All 28 have both, the gate asserts the two counts agree, and swapping the branch so the age is read for plain transforms instead fails all 28. A `use_action` entry the port has not ported yet is kept as [`UseAction::Other`] by its actor type or legacy id, so a type's list is never silently short — 137 legacy string ids like `"ALCOHOL"` are `iuse_function_wrapper`s whose type **is** that string (`src/item_factory.cpp:1807-1818`). **Declared:** the other 29 actor types (423 `transform` entries are the largest, then `consume_drug` 107, `link_up` 102, `effect_on_conditions` 65, `holster` 63), `target_group` and `rand_target_charges` (the corpus exercises neither, so both are read but unproven), and every actor's `use`, including the transformation itself |
| Fault fixes | **ported and gated completely: all 40** | P2 | `cdda_content::faults::FaultFix` is `class fault_fix` (`src/fault.h:50-73`) and its loader, and the gate compares **every field of every fix** against `corpus/items/fault-fixes.json` (`--dump-fault-fixes`): name, success message, time, `mod_damage`, `mod_degradation`, the faults removed and added, and the skill requirements. A fix **removes faults and may add others** — a repair that turns a broken handle into a cracked one is one fix, not two — and it moves the item's damage and degradation by fixed amounts: a grind takes 1,000 damage off and puts **5 degradation on**, which is what makes repeated repairs a losing game. **A fix's time is written in the game's duration grammar** (`"4 h"`, `"30 m"`) and the reference reads it with a JSON duration reader whose unit is a **turn**, so the port's `parse_duration` answer is already in seconds. **20** of the 40 need a skill and **3** add a fault, so neither branch is a corner. **Declared:** a fix's `requirements` (an item-component tree, a family of its own), its `time_save_profs`/`time_save_flags`, the `set_variables` and `adjust_variables_multiply` it applies to the item, and the repair *activity* that uses a fix |
| Faults on an item | **ported: setting, the blocked-fault cascade, forcing, removal** | P2 | `cdda_sim::ItemFaults` is `item::faults` (`src/item.h`), and `set_fault` is `item::set_fault` (`src/item.cpp:7828-7845`) — three steps in a fixed order, and the middle one is the easy one to miss: the new fault's own `block_faults` are **removed before** it is inserted, because a thing cannot have a chipped blade when the blade is gone. So a chipping staff that cracks becomes cracked and not chipped, and breaking takes both away; mutating the removal out leaves the item with both faults. The check runs **first**, so a refused fault changes nothing — not even the removal — which the test pins by trying to chip an already-broken item; a **forced** fault skips the check and arrives beside its blocker rather than replacing it. `remove_fault` is the one `erase` (`src/item.cpp:7869-7872`). **Declared:** `remove_single_fault_of_type`'s first-match-only erase (`7874-7882`), `set_fault`'s message, and `item::tname`'s use of the name prefix |
| Fault selection at damage | **ported: the weighted pick, its blocked set and its single-entry shortcut** | P2 | `cdda_sim::pick_fault` is `item::set_random_fault_of_type` (`src/item.cpp:7848-7865`), `can_have_fault` is `item::can_have_fault` (`src/item.cpp:7809-7826`), and the pick is `weighted_int_list::pick` (`src/weighted_list.h:120-129`, `247-267`). Four rules, each pinned by a test: the die is **`rng_bits()`**, a raw 32-bit draw, **not** an `rng( lo, hi )` pair — so the test replays `bits` lines rather than `int` ones; the raw value is reduced **modulo the total weight** and then walked from the front, which is why 155 of 155 total lands back on the first fault; a **blocked or already-present** fault is off the list, and the block relation is one-directional (a broken handle blocks chipping, not the reverse); and a list of **one** entry returns it **without touching a die at all** (`if( objects.size() == 1 ) return 0;`), which the test proves by replaying an empty recording — likewise an empty list draws nothing. Dropping the `+ 1` from the modulo fails the pick with `fault_handle_chipping` where the reference says `fault_handle_cracked`. **Declared:** `set_fault` itself (adding the fault to the item's list and the displayed-name prefix), the `force` path's `random_entry`, and the `weight`/`message` reporting |
| Item damage, repair floor and degradation | **ported: the type half gated over 9,912 types, the instance half transcribed** | P2 | `ItemType::damage_max`/`degrade_increments`/`damage_level` are `itype::damage_max` (`src/itype.h:1572-1574`), the finalisation in `item_factory.cpp:4115-4126`, and `itype::damage_level` (`src/itype.cpp:117-126`); `cdda_sim::ItemDamage` is `item::damage_`/`degradation_` with `item::set_damage` (`src/item.cpp:967-970`), `item::set_degradation` (`972-979`), the three-way branch of `item::mod_damage` (`9440-9468`) and `get_degrade_amount` (`9428-9439`). **Two of the type rules are not data**: the maximum is the constant 4,000 (or 0 for the 1,261 charge-counted types), and the degradation increments are 0 for all but **230** types because the loader discards the stated `degradation_multiplier` unless the type is a vehicle part. The instance rules are integer arithmetic with three traps, each pinned by its own test and mutation: **the destruction test comes before the damage is applied** (`( damage_ + qty ) > max_damage()`), so reaching the maximum exactly survives and damage never passes it; **`set_damage`'s clamp has the degradation as its lower bound**, so a repair cannot undo degradation; and **`factor( 0 )` is `-1`, not 0**, which makes the first hit degrade an item twice as fast as the second (setting it to 0 fails the test with 80 where the reference says 160). A charge-counted pile takes damage as **charges** (`mod_damage_charges`), not health. **Gated on 79,296 measurements** (`corpus/items/damage.json`, six probe values per type); dividing before multiplying fails 25,953 of them and dropping the vehicle-part gate fails 9,682. `rand_degradation` is `rng( 0, damage() ) * 50.0f / degrade_increments()`, **truncated** by the float-to-int conversion rather than rounded (rounding fails 6 where the reference says 5), and a type that does not degrade **rolls nothing at all** — the reference returns before it touches a die, which the test proves with an empty recording. `degradation_symbol` is the five-step bar, `""` for a type that cannot degrade because the reference checks the type **last**, and a zero slice divides by one rather than by zero. **Declared:** the faults (`set_random_fault_of_type`, `degradation()`'s fault modifiers, and the melee/gun/armour adjustments), `repairable_levels`, `UNBREAKABLE`'s energy-shield variant, and the per-item `item::process_internal` tick — which does **not** degrade anything: there is no per-turn degradation in the reference at all, a claim this port made in its own roadmap and then corrected |
| Inventory counts and spending | **ported: `amount_of`, `charges_of`, `has_amount`, `has_charges`, `use_amount`, `use_charges`** | P2 | `cdda_sim::amount_of` is `visitable::amount_of` (`src/visitable.cpp:964-978`), `charges_of` is `visitable::charges_of` (`src/visitable.cpp:819-875`), and the two spending walks are `use_amount_internal`/`use_charges_internal` — the counting half and the spending half of the same `visit_items`. The distinctions they keep, each pinned by a test: **`amount_of` counts items, never charges** (a pile of two hundred nails answers one) while `charges_of` sums charges; **`amount_of` descends into anything** while **`charges_of` descends only into a container** (`return e->is_container() ? NEXT : SKIP`, `src/visitable.cpp:853`) — so a battery forced into a corpse is counted and not spendable, which the mutation of that one gate fails; `ITEM_BROKEN` is skipped by both before the type is looked at; `limit` stops the walk, which is what makes "are there two?" safe; **`use_charges` visits an item before its contents** and **`use_amount` visits contents first, bottom up** and consumes the whole item — two functions that look alike and are not. `Inventory` is a set of entity handles rather than the reference's `item_location` scheme, the roadmap's P3 decision. **Declared:** the UPS and bionic-power branches (a player layer), `provisioned_pseudo_tools`, the binned-items cache, and `remove_items_with`, whose filter-and-count signature no caller needs yet |
| Unfinished craft volume and weight | **ported** | P2 | `item_volume` and `item_weight` branch on a craft in progress, as `item::volume` (`src/item.cpp:7522-7534`) and `item::weight` (`src/item.cpp:7240-7251`) do: the volume is the **sum of the parts** floored at one millilitre — a recipe of nothing but practices would otherwise put a zero-volume item in the world — and the weight is the same sum with **no** floor. The unfinished craft's own type says nothing, and in the reference it even carries the **result's** type, so its volume would be the finished thing's; the test feeds a type volume of 999,999 ml and the craft still answers 200. The floor is load-bearing: dropping it fails the empty-craft assertion. **Declared:** the cached volumes and weights (`craft_data_->cached_volume`/`cached_weight`), which are memoisation and not behaviour, and the components map's per-component accounting, which the port does not keep |
| Corpse volume | **ported and gated over every creature** | P2 | `cdda_sim::corpse_volume` is `item::corpse_volume` (`src/item.cpp:7455-7479`) reached from `item::volume` for anything that is a corpse (`src/item.cpp:7518-7520`): the **creature's** volume multiplied by four fractions in a fixed order — `QUARTERED` by a quarter, `FIELD_DRESS` **or** `FIELD_DRESS_FAILED` by three quarters, `GIBBED` by 0.85, `SKINNED` by 0.85 — truncating at **every** step because the arithmetic is on whole millilitres, then capped at `MAX_ITEM_VOLUME` (`src/game_constants.h:35-36`, one tile = 1,000 litres). **Gated on 8,078 measurements**: `corpus/creatures/corpse-volume.json` is `--dump-corpse-volume`, 1,154 creatures with the corpse's volume plain and with each flag and with all four; **seven** creatures are over the cap, which is the only reason a plain corpse ever differs from the creature's own volume. `CorpseOf` now carries the creature's volume, `reap_dead_monsters` takes the lookup that supplies it, and `ItemFlags` is a component so that one corpse can be gibbed while another is not. A mutation of one fraction by a thousandth fails four measurements. **Declared:** a corpse's `integral` volume (the reference does not branch on it for a corpse) and the `debugmsg` for a creature with no volume at all |
| Item length | **ported and gated over all 9,912 types** | P3 | `ItemType::longest_side` is `item::length` for everything but guns and corpses (`src/item.cpp:7343-7398`): a **liquid** has no length, a **soft** thing has none (and "made of nothing" counts as soft, because `std::all_of` over an empty list is true), an explicit `longest_side` in the data wins, and otherwise it is `default_length_from_volume` of the **effective** volume — the cube root **of the one-charge volume** for anything counted by charges, rounded on the *root* rather than the volume. `corpus/items/measures.json` records the reference's own answer for every type, and the port matches **9,911 of 9,912**. Four bugs came out of the comparison rather than out of reading: the stack size of a charge-counted item falls back to the type's `"count"` when the data states none (a battery is a stack of 100, so its 100 ml is 1 ml per charge); a material's `soft` flag is **inherited** and must not be overwritten by a child that says nothing; a **gas** is measured like anything else, and only a liquid has no length; and the reference's "cap the stack size at 200" branch never reaches the data (gunpowder states 4,540). **Declared:** guns (a gun's length is its own plus its mods) and `belt40mm`, whose recorded length is one of the 25 grenades its default construction puts inside it — a gap in the creation default, not in this rule |
| Item volume | **partial: the charge rule, contents and the integral volume** | P3 | `item_volume` from `item::volume` (`src/item.cpp:7511-7590`): a charge-counted item's volume is `volume * charges / stack_size` and the division **rounds up a millilitre**, while anything else is its own volume whatever its charge count; a container then adds what is inside it. The type's volume is the volume of a **whole stack** - a battery is 100 ml for 100 charges - so one charge is one millilitre. Six tests, including a type whose stacks do not divide evenly and `item_volume_integral` reading the type's other volume (round 560: **120 types differ, 97 of them zero on purpose**, and a deliberate zero survives the loader because it copies the ordinary volume only when the field is still unset). The dump carries `integral_volume`, `stack_size` and `count_by_charges`, and the whole dataset is replayed through `corpus/items/measures.json`. **Not ported, declared:** corpses, crafted items, the per-item `volume` override, gun mods and `collapsed_volume_delta` |
| Pocket rules | **ported: the accept-lists, the ammunition rule, the phase rules, the size limits and the refusal wording** | P2 | `cdda_sim::check` is `item_pocket::is_compatible` followed by the weight and the room of `_can_contain`, **in the reference's order**, because the order decides which message a payload hears (`src/item_pocket.cpp:1318-1600`): a thing that cannot be unwielded; the accept-lists (a named id **or** a named flag **or** a named material); an ammunition pocket, which takes ammunition and only its own calibres and then **stops**, which is why it never complains about a length; a liquid needing a watertight pocket and a gas an airtight one; the largest item measured **per charge**; the longest side, the shortest side and the smallest volume; then the weight, the pocket's whole capacity and the room left. **Gated on 4,752 real verdicts**: `corpus/items/accepts.json` is `--dump-accepts`, twelve containers against every twenty-fifth item type, with `can_contain`'s answer and message for each pocket — the port reproduces **every message character for character**, 4,696 of them the accept-list's. Four rules came out of the gate: **a soft thing is never too big** (`charges_per_volume_recursive` answers `INFINITE_CHARGES` for a soft item with nothing inside it, so a 500 ml pair of stockings goes into a 17 ml pocket and a rock does not); **"not enough space" is not "item is too big"** — the first is the room left, the second is a thing bigger than the pocket outright; **`is_ammo` is not "names a caliber"** (a magazine names the caliber it holds and is not ammunition); and a **gas** is measured like anything else. `insert_into` reports the reference's own wording for a container whose pockets all refused: the sorted union, `pockets unacceptable because a, b, or c` (`src/item_contents.cpp:802-822`) — which is why a branch in a backpack hears three limits in one sentence. A holster that already holds something answers `holster already contains an item`, and because the port knows which pocket each occupant went into it reproduces the reference's sentence **exactly** — `pockets unacceptable because holster already contains an item or item is too long` for a log offered to a backpack whose holster holds the 2x4. **Declared:** the liquid/gas **mixing** rules against what a pocket already holds; the ammo-capacity and remaining-weight rules (`count` against `remaining_ammo_capacity`, and charges-per-remaining-weight); the base material's tie-break; and the `pocket`/`pockets` singular wording for a container with one pocket |
| Pockets | **ported: the loader and all three compatibility limits** | P2 | `cdda_content::pockets::read_pockets` is `pocket_data::load` (`src/item_pocket.cpp:136-199`): what the pocket is for, whether a liquid or a gas goes in, whether it keeps its shape, the capacity, the weight it takes and the three limits. **Gated over the corpus**: `corpus/items/pockets.json` records every pocket a constructed item reports, and the port matches **all 2,528 pockets across 1,146 types**, in order. Three rules came out of the gate rather than the reading: the longest side defaults to `default_length_from_volume( capacity ) * sqrt( 2 )` — the diagonal of a cube of the pocket's capacity, so a 500 ml pocket takes 113 mm and not anything; a pocket with an **`ammo_restriction` skips every limit**, because `pocket_data::load` keeps them inside `if( ammo_restriction.empty() )`, which is why an ammunition pouch reports a 200,000,000 ml capacity and a longest side of zero; and a gun's installed mods **add pockets** — `pocket_mods` (`src/item_factory.cpp:3357`) applied by `item::update_modified_pockets` (`src/item.cpp:1362-1387`), which is how a Steyr Scout's stock gives the gun a 200 ml container that the gun's own data never mentions. Pockets are also **inherited** through `copy-from` and must not be wiped by a child that says nothing. **Declared:** allow-lists (`item_restriction`, `flag_restriction`, `material_restriction`), `holster`, `ablative`, `sealed_data`, `max_item_volume` with its per-charge volume check, and the `MAGAZINE`/`MAGAZINE_WELL` replacement ordering beyond a single well |
| Pockets | **partial: per-pocket volume and weight limits, watertightness, the phase rule, replayed from a recorded session** | P3 | `Container`/`Pocket`, `insert_into` and its refusals from `item::put_in` over `item_pocket::can_contain` and `item_contents::insert_item` (`src/item.cpp:1848`, `src/item_pocket.cpp:1396-1401,1560-1600,2194`): **a pocket is its own limit** — the payload is offered to each pocket in turn and the first that accepts it takes it — so a backpack whose four pockets hold 25,000 + 510 + 5,000 + 5,000 ml **refuses** a 30,000 ml branch; a liquid needs a watertight pocket and a gas an airtight one; **the weight is checked per item against the pocket’s own capacity** (a 115 kg manhole cover is refused by a pocket that takes 30 kg); and the port answers `TooBig { remaining_ml, wanted_ml }`, `TooHeavy { capacity_grams, wanted_grams }` or `WrongPhase { phase }`. **A recorded session replays it**: `corpus/items/pocket-session.json` puts three rocks, a plank, a log, a branch, a crystal, a manhole cover, a battery cell and a bottle of water into a backpack and records the container’s weight, volume and room after each step; the port reproduces the five accepted steps to the gram and the millilitre (`1290/2250, 1947/2500, 2604/2750, 5044/6880, 1033/2500`) and refuses the log, the branch, the two heavy items and the water. **Capacity, weight and watertightness come from the data** (`corpus/items/pockets.json`). **The length and shortness limits are ported** (round 601): `compatibility_refusal` is the reference's `is_compatible` trio — maximum length, minimum length, minimum volume, in that order — asked **before** the weight and the room, because `is_compatible` runs first inside `can_contain` (`src/item_pocket.cpp:1408-1432`); the three refusals are distinct (`TooLong`, `TooShort`, `TooSmall`) and a test pins that a long *and* heavy item is refused for being long. **The data is real**: the pocket corpus now records `max_contains_length`, `min_contains_length` and `min_contains_volume` beside the volume and weight — **2,502 of its pockets carry at least one** — and the loader test reads them. One thing is deliberately not invented: an item's length is `ItemFacts::length_mm`, and **zero means "not measured"**, because the prototype's `longest_side` is not threaded yet and a length derived from volume would refuse payloads the reference accepts (a real 50-litre item is not a 370 mm cube — the pocket-session corpus caught exactly that). **Not ported, declared:** the prototype's `longest_side` (so the limits are only asked when a caller states a length), allow-lists of what a pocket accepts, liquids as charges and the bottle half of the session, charge restacking, and the holster/ablative/ammo-restriction branches |
| Containers | **partial, declared; a burnt container spills, except what cannot come out** | P3 | `Container`, `insert_into`, `contents` and `take_out` model a container holding items by volume, with the checks `item::can_contain` makes first: it fits or it does not, and an item cannot be in two places at once. Contents are entities carrying `Inside(parent)`, so a container's contents are a query rather than a parallel list. Six tests now: what fits goes in, a full box refuses and reports the room left and the size wanted, taking something out frees the room, an item cannot be in two containers, a non-container takes nothing, and **a container that burns away spills what was inside it onto the square** - which closes the gap the burning row had been carrying. An item flagged `IRREMOVABLE` does **not** spill: the reference collects only contents without `NO_UNLOAD` into its spill list, so an irremovable item is erased with its container, and that is now a test rather than a guess (46 items in the corpus carry the flag). **Not ported:** pockets and their per-pocket weight and length limits, allow-lists of what a pocket accepts, liquid filling, | `Container`, `insert_into`, `contents` and `take_out` model a container holding items by volume, with the checks `item::can_contain` makes first: it fits or it does not, and an item cannot be in two places at once. Contents are entities carrying `Inside(parent)`, so a container's contents are a query rather than a parallel list - the same shape as the rest of the port. Five tests: what fits goes in, **a full box refuses and reports the room left and the size wanted**, taking something out frees the room, an item cannot be in two containers, and a non-container takes nothing. **Not ported:** pockets and their per-pocket weight and length limits, allow-lists of what a pocket accepts, liquid filling, and the contents of a destroyed container (still named as a gap in the burning row) |
| Things that go off in a fire | **partial, declared** | P3 | `ItemType::will_explode_in_fire` from `item::will_explode_in_fire` (`src/item.cpp:13314-13325`): the type's `explode_in_fire`, or ammunition that cooks off. The **dump came first**: `--dump-items` now carries `explode_in_fire`, `cookoff` and `special_cookoff`, and the reference says **41** items explode in a fire and **387** cook off. The port reads `explode_in_fire` and matches the 41 exactly, and derives the two cookoff fields the way `item_factory.cpp:512-522` does - ammunition whose effects include incendiary or cook-off cooks off, a special cook-off is its own effect, and **hydrocarbon or oil items never cook off** however their effects read. It matches the reference exactly: **387 cook off and 8 do it in their own way.** Ammunition states its effects as `effects`, not `ammo_effects` (`item_factory.cpp:2990`), and the derivation runs only for items that declare the AMMO subtype, which is where the reference derives it - getting either of those wrong gave 4 instead of 387 |
| Item flags that do something | **partial** | P3 | `ItemType::is_irremovable` from `item::is_irremovable` (`src/item.cpp:10356-10359`) - the `IRREMOVABLE` flag and nothing else - and it is wired into the burning loop rather than merely defined. One content test over the whole corpus: **exactly 46** items carry the flag, `efile_lore` among them, and a rock does not. This is the port's first behaviour read from item data rather than from a parameter |
| Item damage levels | **done** | P3 | `damage_level` from `itype::damage_level` (`src/itype.cpp:117-126`): an undamaged item is 0, a type counted in charges is **ruined by any damage at all** (charges have no partial damage, so a leaking bottle reads as ruined rather than scratched), and anything else climbs `1 + 4 * damage / damage_max`, saturating at 5. Five tests, including the saturation past the type's maximum and a type with no damage capacity. Ground items carry a `damage` now, and the stacking rule compares damage levels as the reference does - one of the gaps that row declared is closed, and the test that a dented item never stacks with a sound one is what proves it |
| How much fits | **done** | P3 | `charges_per_volume` from `item::charges_per_volume` (`src/item.cpp:1391-1420`): a charge-counted type fits `wanted * stack_size / volume` charges, anything else fits as many whole items as the space allows, and only a type with **no volume at all** reports `INT_MAX` - a wanted volume of zero holds nothing, because that is what the reference's arithmetic says. Four tests. The first version of this returned "unlimited" for zero space too and a test I wrote tried to justify it; the test was rewritten to match the reference rather than the code |
| Item stacking | **partial, declared** | P3 | `stacks_with` and `drop_and_merge` from `item::stacks_with` (`src/item.cpp:1693-1760`): two things of the same kind stack, charges are ignored for a type counted in charges - which is what makes a pile a pile - and must match exactly otherwise, so **two half-used batteries never merge**, and a burnt item never stacks with a fresh one. Six tests, including dropping onto a matching stack adding to it rather than making a second pile. **Not compared, declared:** relics, variants, contents, degradation, filth and activity (damage **is** compared now), because the port's ground items do not carry them yet; the reference also has `precise` and `check_cat` modes this rule does not model |
| Item group draw | **ported and gated: the reference's own dice, the collection cap, the holiday skip, the count that draws nothing and the recursion chain** | M3a | `item_group_draw::draw` now asks for the ranges the reference asks for: a **collection** one `rng( 0, 99 )` per entry and a skip when the answer is at least its chance (`src/item_group.cpp:837-841`), a **distribution** one `rng( 0, sum_prob - 1 )` for the whole group and then a walk subtracting each entry's raw weight and taking the first that takes the total below zero (`:844-856`). That is not cosmetic: a replay is a list of `(range, answer)` pairs, and the port was asking `rng( 1, 100 )` and `rng( 1, total )`, so it consumed none of the reference's dice - the new gate replays a recording and the range check fails loudly rather than drifting. Two loader rules came with it: an entry whose weight is not positive is **not in the group at all** (`add_entry`, `:813-815`) and a collection caps its chances at a hundred (`:818-820`). And the holiday rule: an event-based entry outside its holiday has an in-season chance of zero, is **passed over even when the die chose it**, and its weight still counts towards `sum_prob` - mutation-checked by removing the skip. **The count rule** (round 587): `cnt = ( first == second ) ? first : rng( first, second )` — a count that cannot vary draws **no die**, and the port was drawing one, which put it a die ahead of the reference for the rest of the session; a count of zero makes nothing rather than being floored to one. Both mutation-checked. **The recursion rule** (round 588): the reference keeps a `RecursionList` — the **set of groups the expansion is inside** — pushes a nested group before expanding it and pops it after, and treats a repeat as an error and ends the branch (`src/item_group.cpp:307-312`). The port had a depth limit of eight instead, which drew a cycle four times *and* silently dropped the items of a chain nested ten deep; both are now gated, and removing the set check overflows the stack rather than quietly agreeing. **Not ported, declared:** the `spawn_flags::maximized` and `use_spawn_rate` variants of the same function, `Item_modifier::modify` (damage, dirt, faults, charges, the container the item goes into), the `EVENT_SPAWNS` option assumed to be `items`, and `put_into_container`, which needs pockets |
| Container overflow | **ported and gated: spill, discard, and the `none` that loses too** | M3a | `fill_container` is `put_into_container` (`src/item_group.cpp:115-171`) without the leading shuffle: each freshly spawned item is offered to the container in turn and, when it does not fit, is spilled onto the ground, discarded, or — for `on_overflow: none` — **lost as well**, because the function erases every drawn item from the group's list and puts back only what reached its `excess` pile. That is the part the names hide, and it is the part the tests pin: `spill` leaves the leftovers on the square, `discard` and `none` leave nothing, and flipping the branch fails two tests. **The shuffle is ported too** (round 594): `fill_container` shuffles the run before offering it, with `std::shuffle` over the game's engine exactly as the reference does — not through `rng()`, so no die is recorded for it, and the port drives its own engine instead. Faithfulness here is not a matter of opinion: `shuffle` is libc++'s `__shuffle` plus the `uniform_int_distribution<ptrdiff_t>` it reaches (`uint64_t`, so the 64-bit independent-bits engine), and `corpus/rng/shuffle.json` holds **36 recorded permutations** — nine lengths by four seeds — that the port reproduces draw for draw, first try. A seed-dependent test then shows the shuffle is actually reached by the spawn path. **And the corpse rule** (round 595): when an item does not fit, `put_into_container` asks whether the **container** is a corpse and, if so, forces the item in whatever its pockets say (`force_insert_item`, `src/item_group.cpp:137-145`) — corpses are the one container the data cannot size. `Fill { overflow, container_is_corpse }` carries both options, `force_into` is the forced insert, and the fixture is deliberately paired: the same 50 ml box takes three 100 ml rocks as a corpse and none as an ordinary container, so the test is about the flag rather than the arithmetic. Mutation-checked. **Not ported, declared:** `add_automatic_whitelist`, `sealed`, and the container variant (`set_itype_variant`) |
| Contents, pockets and containment | **partial: per-pocket volume and weight, watertightness, the phase rule** | M3a | `sim::insert_into` walks the pockets in order and refuses with `TooBig`, `TooHeavy` or `WrongPhase`; replayed against `corpus/items/pocket-session.json`. **Not ported, declared:** the length and shortness limits and the pouch-of-holding rules |
| Item location and references | not started | M3a | Replaced by generational handles |
| Item groups and spawn tables | **partial: the tree and its draws** | M3a | `item_groups` loads the recursive tree and reproduces the reference's draws from its own dice, including the default weight. **Not ported, declared:** `collection` vs `distribution` at every depth beyond what the gates cover |
| Item prototypes | **ported and gated** | M3a | 9,912 types loaded from 21 prototype fields and gated against `corpus/prototypes/items.json`, with weight, volume, integral overrides, flags, qualities, subtypes and the gun facts pinned in their own gates |
| Inventory | **partial: `Kit`, the model the build check asks** | M3a | `requirements::Kit` answers `amount_of`, `charges_of` and `has_quality` and is what five thousand recorded answers stand on. **Not ported, declared:** the real `inventory` with its stacks, wielding, magazines and UPS |
| Crafting and recipes | **partial: the registry, the time and the progress** | M3a | The recipe registry and its requirement groups are gated (`Recipe::components_available`), and `batch_time_moves`, `CraftingWork` and `craft_turn` carry a craft forward. **Not ported, declared:** `deduped_requirement_data`, so craftability is answered against the simple requirements rather than the deduped ones |
| Requirements and qualities | **ported and gated** | M3a | 433 requirement groups (`requirements::load_paths`) and the 87 tool qualities with every item's level (`qualities::load_paths`), both gated; `RequirementData::can_make` and the pile rules answer buildability for seven recorded kits |
| Tool qualities | **the registry and every item's level, gated** | P3 | `qualities::load_paths` for the 87 quality ids and `items::ItemType::qualities` for the 1,091 item types that have one, gated against `corpus/qualities/all.json` (`--dump-qualities`): every id, every name and every level matches. Levels can be **negative** — `BUTCHER -90` on a saw means worse than bare hands, not a sentinel. Three implicit rules the data taught: a gun that is neither a gun mod nor a primitive ranged weapon gets `GUN` at level one **and** the quality named after its skill, but only when that name is a defined quality (so a launcher gets `GUN` and no `LAUNCHER`); gun data is written **flat** (`"skill"` beside `"ammo"`), not under a `gun` member; and qualities are a **container**, so `extend`/`delete` reach them (round 567) |
| Build check | **whether a construction can be built from a kit, gated** | P3 | `requirements::RequirementData::can_make` from `requirement_data::can_make_with_inventory` (`src/requirements.cpp:865-888`), with `Kit` standing in for the reference's `inventory` (`src/inventory.h:114`). Gated against `corpus/build/check.json` (`--dump-build-check`): **5,285 of 5,285** answers across **seven kits** — empty, tools only, materials only, tools that spend charges, a stocked one, one that only a charge count can refuse, and the two together. Rules the data taught: a positive tool count is **charges** and the kit must hold at least that many (the reference's `charges_of` is asked with a limit and comes back clamped, so its `==` means "at least"), a negative one is a tool to **hold**, an item counted by charge is asked for in charges and anything else in items, and `item( id, turn, n )` makes **one** item — a kit of forty planks is forty items (round 568). `check_enough_materials` is ported too (`src/requirements.cpp:1061-1146`): a component that is also an available tool needs `count + tc` of it, a component that itself provides a required quality needs `required + count` at that quality, a charge count becomes items through the type's `stack_size` before it is measured, and the components of one craft may not add up past `MAX_ITEM_VOLUME` — one thousand litres (round 569). **Not ported, declared:** the UPS accounting and the `is_crafting_component` filter |
| Constructions | **the registry and what each one takes, gated end to end** | P3 | `constructions::load_paths` from `construction::load` (`src/construction.cpp:2194-2260`) and `requirements::load_paths` from `requirement_data::load` (`src/requirements.cpp`), merged by `constructions::needs_of` from `finalize_construction` (`src/construction.cpp:2533-2537`). Gated against `corpus/constructions/all.json` (`--dump-constructions`): **755 of 756** constructions match on id, group, category, time, `pre_terrain`, `post_terrain`, skills and their **components, tools and qualities** — every alternative of every group, at the reference's counts. Rules the data taught: `pre_terrain` is a `std::set` in byte order, skills fall back to the legacy `skill`/`difficulty` pair, time is a duration string in moves, `"LIST"` marks a nested group which is scaled by its outer count with a floor of `-1` and inlined from its **first** group only, a name is only a group if it is flagged, a bare string is a tool of count `-1`, tools expand into their subtypes and components do not, and two quality groups that cover each other collapse into the newer one (rounds 565-566). **Not ported, declared:** `constr_veh`'s frame list (generated at finalisation, `src/construction.cpp:2516-2520`), the special functions, `pre_flags`/`post_flags`, byproducts and the blacklist |
| Use actions | not started | M3a | 195 functions |
| Examine actions | not started | M3a | 108 functions |
| Consumption, butchery, liquids | not started | M3a | |
| Auto-pickup | not started | M3a | |

## P4 — Creatures

| System | Status | Gate | Notes |
|---|---|---|---|
| Creature base state | not started | Combat replay (M3b) | |
| Character state | not started | M3b | The largest single object in the game |
| Monster state | not started | M3b | |
| NPC state | not started | M3b | |
| Creature tracking and lookup | not started | M3b | Four parallel indexes today |
| Effects | not started | M3b | |
| Body parts and anatomy | not started | M3b | |
| Mutations | not started | M3b | |
| Bionics | not started | M3b | |
| Morale, needs, wounds, disease | not started | M3b | |
| Melee, ranged, projectiles, dispersion | not started | M3b | |
| Martial arts | not started | M3b | |
| Weakpoints, harvest, death | not started | M3b | |

## P5 — Spells and enchantments

| System | Status | Gate | Notes |
|---|---|---|---|
| Spell types and casting | not started | Spell replay (M3c) | Five cost kinds; mana is one |
| Spell effects, shapes and targeting | not started | M3c | |
| Enchantments | not started | M3c | Feeds creature and item stats |
| Terrain and furniture transforms | not started | M3c | |
| Teleporter lists | not started | M3c | |
| Relic generation | not started | M3c | |

## P6 — Activities

| System | Status | Gate | Notes |
|---|---|---|---|
| Activity actors | **partial: butchery is one, and it runs over turns** | Activity replay (M3d) | `ButcherState` is an entity's activity: `start_butchery` begins it, `butchery_turn` takes one turn of it, and `butchery_moves_left` says how much is left - gated by `sim_suite/butchery_activity.rs`. **Not ported:** the actor framework itself (the 69 classes' common shape), which arrives when there are creatures to act |
| Activity handlers | not started | M3d | |
| Activity record and suspension | not started | M3d | Three concurrent slots today |
| Pickup and movement modes | not started | M3d | |
| Action pipeline | not started | M3d | |

## P7 — AI and scripting

| System | Status | Gate | Notes |
|---|---|---|---|
| Monster planning and movement | not started | Decision traces (M4a) | Highest divergence risk in the port |
| Behaviour trees | not started | M4a | Data over a pointer graph today |
| NPC decision making | not started | M4a | 28 actions, ~30 tuned scorers |
| Pathfinding | not started | M4a | Includes load-bearing tuning constants |
| Dialogue | not started | M4a | |
| Conditions and effect-on-condition | not started | M4a | Content is code in a mini-language |
| Missions | not started | M4a | |
| Factions runtime and trading | not started | M4a | |

## P8 — Vehicles

| System | Status | Gate | Notes |
|---|---|---|---|
| Vehicle identity and model | not started | Vehicle replay (M4b) | No identity exists today |
| Parts and derived indexes | not started | M4b | |
| Movement and collision | not started | M4b | Pointer-order ties today (B1) |
| Vehicle interaction and use | not started | M4b | |
| Turrets and vehicle groups | not started | M4b | |

## P9 — World generation

| System | Status | Gate | Notes |
|---|---|---|---|
| Overmap storage | not started | Seed corpus digest (M5) | |
| Overmap generation | not started | M5 | Order-dependent today |
| Map generation interpreter | not started | M5 | 845 files of DSL |
| Built-in map generators | not started | M5 | 12 functions |
| Regional settings | not started | M5 | |
| World factory and world options | not started | M5 | |
| Hordes | not started | M5 | |
| Cities and map extras | not started | M5 | |
| Basecamps and faction camps | not started | M5 | |

## P10 — Interface

| System | Status | Gate | Notes |
|---|---|---|---|
| Text layout and output | not started | Frame parity (M6) | |
| UI stack and window abstraction | not started | M6 | |
| Menus (254 sites) | not started | M6 | |
| HUD widgets and panels | not started | M6 | Specification is JSON |
| Input and keybindings | not started | M6 | 620 rebindable actions |
| Fonts and colour themes | not started | M6 | |
| Tiles renderer | not started | M6 | Pixel gate nightly |
| Terminal renderer | not started | M6 | |
| Sound and music | not started | M6 | Not part of the state digest |
| Individual screens | not started | M6 | Full list in ui-screen-inventory |

## P11 — Runtime, platform, tooling

| System | Status | Gate | Notes |
|---|---|---|---|
| Game object and schedule | not started | Perf and packaging (M7) | |
| Main entry and command line | not started | M7 | Harness flags must survive |
| Input-to-action handling | not started | M7 | |
| Game mode and UI state | not started | M7 | |
| Event bus | not started | M7 | |
| Debug menu and editors | not started | M7 | ~309 actions; handy, hazardous |
| Screenshots and capture | not started | M7 | |
| Crash handling and diagnostics | not started | M7 | |
| Platform shims | not started | M7 | macOS, Linux, Windows; Android and web deferred ([12-platforms.md](12-platforms.md)) |
| Packaging | not started | M7 | Three desktop pipelines; the reference ships nine artifacts across five |

## M0 evidence

The gate is written as "harness exists; no gameplay yet — RNG draw-for-draw
equality; JSON accept/reject parity". Each part, and where it is proven:

| Claim | Proof |
|---|---|
| The harness exists | Five modes: environment, state, prototype digest, prototype ids, replay. Replay reaches a byte-identical state twice from one recorded session |
| RNG draws are identical | The engine is compared draw for draw against the reference for every composite case in the corpus; the distributions are transcribed from this build's standard library and compared the same way |
| JSON acceptance is identical | All 6,220 shipped files: same accept/reject verdict and same structural digest as the reference; the writer is compared byte for byte on 127 of them |
| The cache changes nothing | All 6,220 files read identically cold and warm |

## The replay runs a square at a time, and stops at the first unknown draw (round 269)

Replaying the reference's whole stream - not one die per square, but every die in order - is stricter than the raffle test
and found two things at once:

* **A real port defect.** A biome layer can yield **furniture** (`f_region_forest_dense` sits in the shrubs layer beside the
  placeholders), and the reference treats that as *no terrain*: furniture is placed separately and the square still falls
  through to the ground raffle. The port was returning the furniture name as the square's terrain. The recorded chunk caught
  it on its thirty-first square.
* **The unknown draw, identified.** It is the reference's **item spawning**: a forest square that places a feature also
  rolls for its biome's item group, and building an item draws a full-range int (`item::item()` during item group spawning -
  found by having the draw print its own call stack). The port places no forest items at all, so the dice cannot line up
  past the first square that does. Thirty-one squares replay **exactly** before it, which is what makes this a missing
  feature rather than a broken rule.

The second is parked as an `#[ignore]`d spec whose reason now names the missing feature rather than an anonymous draw: a
failing test in the tree is worse than a labelled one, and this one says precisely where the port stops agreeing and why.

**Item spawning in forest chunks is the next piece of P9**, and it is now a named gap with a replay that will measure it.
The reference places items **once at the end of the chunk** (`place_items( item_group, item_group_chance, whole chunk )`,
`item_spawn_iterations` times), not per square - so the port needs that rule, and the desynchronisation it causes is a
consequence of not placing them at all rather than of a per-square roll.

How much work that is, measured from the recording: between one chunk's last square and the next chunk's first square the
reference spends **589 to 601 dice**, where the placement rule alone accounts for one roll for the count plus three per
attempt. So the bulk is not picking squares - it is **expanding the item group**, which constructs items and draws for each.
The port now has the placement rule (count, square, twenty-try refusal) and not the expansion; that is the next piece, and the
measurement says precisely how big it is.

Two instrumentation attempts failed and were removed rather than left behind: a phase marker written from
`mapgen_functions.cpp` never reached the recording (the hook compiled but produced no lines, so it was unverified
machinery), and a call-stack dump fired on every full-range draw in the process and made a run take minutes instead of
seconds. The oracle is back to what it was: the draw recording, the per-square draw index, and nothing else.

**Furniture in a forest chunk is now a declared gap:** the port detects it and routes the square to ground correctly, but does
not place the furniture itself.

## The reference's die, through the port's raffle: 576 of 576 (round 268)

Each square of the recorded chunk now says **which die of the whole run it spent** - a 3.25-million-draw recording, entered
at the right index - and **which ticket that die bought**. The port recomputes the six weights from the perimeter alone and
runs its own raffle on the reference's die.

Result: **576 of 576 squares buy the same ticket.** Not a share, not a shape, not an average: every square of a generated
chunk reaching the same decision the reference reached, from the reference's own randomness.

The chunk is deliberately the hardest kind to fake - eight dense-forest neighbours, so the empty ticket's weight is zero
everywhere and no square falls back to plain ground. All four directions are used (59, 58, 54 and 72 squares) and 333 squares
take their own biome's ticket, so the test exercises the whole pool rather than one branch of it.

That closes the loop the last twelve rounds opened: the weights are right (576/576), and the weights **plus the reference's
dice** produce the reference's decisions (576/576). What is still unproven is everything downstream of the ticket - which
terrain the chosen biome's layer then produces.

## The pool, square by square, against the reference's own numbers (round 267)

The reference now writes, for every square of a chunk, the **four neighbour weights it computed**, the self weight and the
empty ticket's weight. No dice are involved on either side: this is arithmetic, and it can be compared positionally.

Result: **576 of 576 squares** match, on a chunk whose eight neighbours are all `forest_thick`, so no direction is
short-circuited by a neighbour with no forest. The self weight is the constant 3 on every square, and the empty ticket's
weight - the one figure that ties all six together, so any sign error anywhere shows up in it - follows from the same four on
every square too.

**One convention difference, found by the data:** the reference scales each neighbour weight by the biome's
sparseness-adjacency factor (4 here), while the port returns the proximity fraction and scales at the call site. The two
agree once the factor is applied, but my first version of the test compared them unscaled and read the port as four times too
small everywhere.

## How a chunk's terrain is actually declared (round 484)

Counted over the 6,216 `object`s, by which of the three ways each one declares terrain:

| `rows` | `terrain` inline | `palettes` | Entries |
|---|---|---|---|
yes | no | yes | 2,411 |
yes | yes | yes | 1,701 |
no | no | no | 1,302 |
yes | yes | no | 627 |
yes | no | no | 150 |
no | no | yes | 17 |
no | yes | no | 7 |
no | yes | yes | 1 |

Four consequences, each a case a generator either handles or silently gets wrong:

* **4,889 draw a grid, 1,327 do not.** A generator that requires `rows` is wrong for a fifth of the data.
* **2,429 need a palette lookup** (2,411 + 17 + 1). Reading `object.terrain` alone draws nothing for them.
* **150 draw a grid with no terrain declared at all** - they come from `fill_ter` or the game's default, from neither map.
* **1,302 have neither rows nor terrain nor palettes** - placement-only entries, not empty chunks.

**Not written yet:** any of the resolution. These are the counts a resolver has to reproduce, and they are recorded here because they cost a measurement each and would otherwise be
re-derived - badly - from a reading of the schema, which has already been wrong twice in this file.

## A mapgen entry keeps its map under `object` (round 482)

The trap, measured over all 6,216 mapgen entries:

| Where | Entries |
|---|---|
`rows` at the top level | **0** |
a terrain palette at the top level | **0** |
`object` holding the map | **6,216** |

So a generator written against `entry["rows"]` - the obvious shape - would compile, run, and draw nothing. The map is under `object` in **every** entry, and `object` itself varies:
some carry only placement effects, some a rows grid, some nested mapgen ids. The shapes fall 2,410 with `object` alone, 1,838 with a nested id, 867 of those carrying a comment, and
735 with an update id.

**My expectation was wrong and the data corrected it twice**: I wrote 5,850 under `object` from the sample I had read, and the measured answer is all 6,216. That is the third time in this
project a guess about data I had just counted has lost to the count.

## The mapgen registry equals the reference's, in both directions (round 460)

The first gate of the mapgen family is in, and it is an equality rather than a count:

```text
keys written in the data  +  the 26 registered in code  -  the 4 that are not chunks  =  6,306  =  the reference's list
```

The test walks `data/json` the way a registry build would, parses every file, keeps `type: mapgen` entries and collects `om_terrain` however it is written - a string, an array, or
an array of arrays - then does the union and the subtraction and compares **both directions** with the dumped list. A chunk the port would miss and a chunk it would offer are named
separately, because they are different bugs: one is a region the player can walk into and the port cannot draw, the other is a region the port would invent.

**What this does not yet prove:** that any chunk is generated *correctly*. It proves the port knows which chunks exist, which is the precondition for generating one. The terrain of a
generated chunk is gated separately (`corpus/maps/`), and the generator itself is the next piece of work.

## The generator's keys are the JSON ones plus a handful made in code (round 442)

The reference's 6,306 mapgen keys were compared against the `om_terrain` keys written in the data itself:

| Set | Count |
|---|---|
reference's keys | 6,306 |
`om_terrain` keys in the JSON | 6,284 |
in the data but not the reference | 4 |
in the reference but not the data | 26 |

**The 26 are the interesting half**: `forest`, `forest_thick`, `forest_water`, `lab_subway_curved`, `lab_subway_end` and twenty-one more are registered in **code** rather than in
JSON, which is why a port that builds its key set by scanning files comes up short exactly where the built-in generators live. The four the other way are data entries the reference
does not register - obsolete or condition-gated ones - and they are the mirror trap: a port that trusted the files alone would generate chunks the game cannot.

**The four named**: `lab_1side`, `lab_4side`, `lab_finale_1level` are floorplan **selectors** rather than chunk ids - the lab generator picks among them - and `null` is the
abandoned plaza's placeholder. So it is not that the reference forgot them; they are not chunks. Both directions are now pinned by tests, the twenty-six by name and the four by
name, which is the only way an equality test earns the word.

So the rule for the port's registry is neither list alone but the reference's list, and the test that will hold it is an equality against the dump rather than a superset check.

## What the generated chunk actually contains (round 439)

The reference's thick forest chunk, read before anything tries to reproduce it:

| Fact | Value |
|---|---|
overmap terrain | `forest_thick` |
distinct terrains in the chunk | **27** |
most common | `t_grass` 222, `t_forestfloor` 173, `t_grass_dead` 54, `t_underbrush` 21, `t_dirt` 16 |
squares | 576 = 24 by 24 |

**Twenty-seven kinds in one chunk is the shape of the problem.** A generator that produces grass and floor and calls it a forest is wrong by twenty-five terrain ids, and the
count makes that visible without running anything. It also says the port's mapgen cannot be a single function: 27 kinds over 576 squares means weighted choice among a palette, and
the palette is what `--dump-mapgens` describes.

## The map dump works, and a generated chunk is now gated (round 434)

The hang was the guard, not the load. `dump_map` asked every one of the 24 by 24 squares for its terrain, and a square past the loaded chunk pulls in a submap and then waits for
overmap generation that a headless run never provides. The harness now asks **only for squares the map holds** (`here.inbounds`) and writes an empty id for the rest, and the dump
finishes in seconds with a full chunk: `oter` `forest_thick`, overmap square `[60, 49, 0]`, 24 rows of 24 ids.

Two tests gate it, and they are the first ones in the project that measure **generated** terrain rather than terrain handed over:

| Assertion | Why it matters |
|---|---|
24 rows of 24 squares, named overmap terrain | a lost row would read as a different chunk |
every id is in the terrain loader's set | a generator inventing an id is the first thing a port's own generation would get wrong |
more than one kind of terrain in the chunk | a dump that returned one flat terrain would pass the id check vacuously |

**What is still missing before the mapgen gate itself:** the port has no generator yet. This dump is the reference's answer to "what does this chunk look like", and comparing it needs
the port to produce a chunk of its own - mapgen, the connection logic and the overmap terrain lookup, which is P1's mapgen family rather than P2's environment work.

## The map dump did not finish headlessly (round 430, closed in 434)

`--dump-map` was run against the same loadout that finished the frame in seconds. It wrote 6,455 bytes and then made **no further progress for half an hour**, with the process
still alive, so it was stopped and the file removed rather than left growing or hanging.

So the map oracle is **not** available yet, and the reason is in the harness code rather than in the invocation: the frame dump reads the map the game already generated, while the
map dump asks for generation, and generation in a headless run reaches something that waits. **Where it hangs, read off the code rather than guessed** (`harness.cpp:1011-1036`): the dump opens the file, takes `get_map()`, and loops over the 24 by 24 squares
calling `here.ter(...)`. Opening the file and writing the header is what those 6,455 bytes were; the stall is inside the loop, where asking for a square's terrain pulls in a
submap, and a submap that is not there yet asks the overmap for an OMT that has not been generated - and headless generation waits for something that never comes.

**The one-line fix was tried and does not work.** `here.load( project_to<coords::sm>( omt ), false )` before the loop makes the dump produce **nothing at all** - the call itself
is what blocks, because it is generation that waits, not the read. The line was reverted and the binary rebuilt, and the weather dump still reproduces byte for byte.

**How far it got, from the bytes it wrote.** The dump wrote 6,455 bytes: a header of about a hundred and rows of twenty-four quoted terrain ids, roughly 480 bytes each, so
**thirteen rows were written and the fourteenth blocked**. Thirteen rows is past the first submap's twelve, which is why the read stalls there and why the frame dump next door
never sees it: the frame reads twenty squares around the player, all inside the loaded bubble, while the map dump walks the whole twenty-four row chunk and steps outside it.

So the next attempt has a shape: either clip the dump to the squares the map actually holds and say so in the file, or force the chunk's generation on a path that does not wait.
Clipping is honest and small; it also means the dump describes what exists rather than what was asked for.

So the map oracle needs a different approach rather than a tweak: generate the chunk in a path that does not wait (a dedicated harness that calls the mapgen directly on an
`overmapbuffer` entry), or read the terrain from a **saved** world that was generated interactively. Either is harness work with a real design question in it, and neither is a
blocker for the port, which has the frame gate and every loader gate standing.

**Nothing else is blocked by it.** The glyph gate stands on the frame dump and passes on 1,638 squares; the loader and behaviour gates are unaffected; and the weather corpus came
from the same binary.

## The reference frame's light column is nearly binary, so it is a weak oracle (round 428)

Before writing a light gate, the column was read. Over 1,681 squares the reference records **two** values:

| Light | Squares | Where |
|---|---|---|
100.582 | 1,487 | open ground **and** under trees alike |
4.000 | 194 | the squares with no daylight of their own |

**Trees do not shade in this dump.** Forest floor beside a birch and forest floor under one both read 100.582, so the column cannot catch a port that forgets to block daylight -
the two values are "daylight" and "something", and any coarse rule reproduces them. So the light gate is **not** worth writing against this fixture; what it needs is a dump
taken somewhere the light actually varies: a lamp indoors, a wall casting a shadow, or the same map at a chosen hour.

That is a small, useful negative result: it stops a gate from being written that would pass for the wrong reason. The frame's glyph column, by contrast, varies over 40 distinct
characters and is what the square-by-square gate uses.

## The port reproduces the reference's frame, square by square (round 426)

The two halves of the frame gate are joined, and it holds: given the reference's own terrain for each square, the port's draw path prints **the reference's glyph on all 1,638
squares where nothing is drawn on top**. The comparison walks the reference's frame, sets the port's level to the terrain the reference recorded, draws with `Frame::text` and
asserts each square's character against the reference's own glyph column - 1,638 assertions, not a hash and not a spot check.

| What is compared | Source | Result |
|---|---|---|
glyph per square | the reference's `--dump-frame` | 1,638 of 1,638 plain squares |
terrain id in the frame | the port's terrain loader | all 1,681 known |
the 43 overlaid squares | items, creatures, fields | excluded by name, not by tolerance |

**What this proves and what it does not.** It proves the loader's symbols, the level's terrain layer and the draw path agree with the reference on the picture a player would see.
It does not prove map generation, because the terrain in the test comes from the reference rather than from the port's mapgen - that is the next gate, and it needs the
`--dump-map` output. The light column of the frame is likewise not yet compared: the port's sun is a hand-set number in this test.

## What the frame's glyph column means (round 424)

Before the frame gate can compare anything, it has to know what the reference's glyph column holds. Measured over the 1,681 squares of the reference frame:

| Squares | Glyph is | Meaning |
|---|---|---|
1,638 | the terrain prototype's own symbol | nothing drawn on top |
43 | something else - `f`, `*`, `,`, `6`, `)` | an item, a creature or a field standing on the square |
0 | - | every terrain id in the frame is in the terrain dump |

**So the reference's glyph is the drawn glyph, not the terrain's.** The port can be compared square by square only where nothing is drawn on top, until items and creatures exist;
the other 43 squares are a named gap rather than a tolerance. The counts are pinned, because a loader that dropped symbols, or a draw rule that stopped covering something, would
move them.

## Fog dims a lamp by the absorption it adds (round 419)

The weather number now reaches the physics, and a test works out the expected light rather than asking the port for it: every square under open sky absorbs `sight_penalty`
times more (`lightmap.cpp:146-154`), and the cast attenuates by `e^( -transparency * distance )`. Five squares out, fog's extra `0.7` of one unit is
`exp( -(1.7 - 1.0) * TRANSPARENCY_OPEN_AIR * 5 )` of the clear-weather light, and the measured answer agrees within five percent of the clear value.

**A one-word slip inside a test.** The content-side weather test filtered the "real" types by the wrong id - it excluded `none`, which is not a type, instead of `null`, which is -
so the zero-penalty type survived its own exclusion and the "every real weather makes sight worse or equal" assertion failed. The lesson is small and worth keeping: an id used
twice in one test is a place where one of the two will be wrong, and the failure named the assertion rather than the id, which is why it took a second look.

## The weather resource reads the data (round 418)

The `Weather` resource was handed its penalty by hand; it now takes a loaded type. The one rule worth a line is the zero:

| Data | Meaning | Resource |
|---|---|---|
`sight_penalty >= 1` | the absorption multiplier | passed straight through |
`sight_penalty == 0` | nothing wrote one (`null`) | **clear air**, not a division by zero |

**The guard stayed and the conversion moved.** `Weather::with_sight_penalty` still panics below one, because a caller passing `0.5` has the direction backwards; the loader's
zero is not that kind of mistake, so `Weather::from_type` translates it before the guard can see it. A test pins both halves: fog arrives at 1.7, and the unwritten type arrives
equal to `Weather::clear()`.

## The weather loader, and three guesses the data corrected (round 417)

`weather_type` derives its numbers from a handful of flags rather than storing them (`weather_type.h:73-140`), which is why the port takes them from a dump: **20 types**, gated on
`--dump-weather`.

| Field | Meaning | Trap |
|---|---|---|
`sight_penalty` | the multiplier `calc_transp` applies - higher is more absorbing | fog is **1.7**; the `null` type carries **0**, which means "unwritten", not "better than air" |
`precip` | none / very light / light / heavy | drizzle is *light*, rain is *heavy*, snowing is *heavy* |
`rains` | rain or snow | flurries are light precipitation that is not rain |

**Three expectations were wrong and the data won.** This round wrote that snowing was light precipitation, that rain was light, and that no type carries a penalty below one. The
dump says snowing is heavy, rain is heavy and `null` carries zero. Every one of those was a guess about data I had just dumped and could have read first; the tests now assert
the measured values and say which way the guess went.

**A zero that means "unwritten" is a bug waiting to be written.** `Weather::with_sight_penalty` panics below one, so the loader has to map that zero to the identity rather than
hand it over - the test names the one type that carries it, so the mapping cannot be quietly forgotten.

## The port can draw a frame (round 390)

Three answers per square - what is there, how brightly it is lit, how clearly it is seen - existed separately. A frame asks them together, once per view:

```text
one sight cast -> one light map -> a walk over the squares
```

| Piece | Reference | Port |
|---|---|---|
the glyph | `map_data_common_t::sym` | `TerrainTable::symbol`, loaded from the dump |
the level | `lit_level` | `lit_level_with`, the wall-face rule included |
the sight | `seen_cache` | `SeenGrid` from the cast |
the frame | the draw loop | `frame::frame`, `Frame::text`, `Frame::levels` |

**`Frame::levels` is the string a semantic frame gate compares.** A picture of the world is hard to diff; a picture of what the character can *tell* about the world is not - one
character per square, `B L l * .` for bright, lit, low, a glow with nothing in it, and blank. The tests use it already: five rows, five columns, a `*` where smoke is in the way.

**The view now owns its rules.** `View` held `fn` pointers, which the sim could not build because its rules read loaded tables; `Box<dyn Fn>` is what a table-shaped rule needs, and
the tests that used to write `|code| code == 2` now box it. Small change, and the reason the frame could be written at all.

**Furniture is merged into the transparency by code rather than by square.** The reference multiplies two per-square numbers; a frame that draws one level only needs to know
whether a code can be seen through, and the more absorbing layer decides. That is a simplification with a name on it rather than an accident.

## The sim casts from the numbers it already has (round 389)

The frame needs one sight cast per view, and the sim has every square's transparency before it casts anything: terrain, furniture, the outside cache and the fields, all
multiplied the way `calc_transp` multiplies them. It could not hand that over, because the view stored a `fn` pointer and a table has no `fn` pointer.

| Entry point | Takes | Use |
|---|---|---|
`seen_grid_of` | the per-square transparency table | the sim, which owns the numbers |
`seen_grid` | a code-to-number closure | a test or a tool with nothing to look up |
`seen_cast` | the table | the one implementation both go through |

**One cast implementation, and it takes a table.** The closure entry point walks the cast's square once to turn codes into numbers and then delegates, so there is a single place
where attenuation happens - the shape that stops two casts from drifting, which is the bug this file has already paid for twice.

**A space is a glyph, and the test had to learn it.** The glyph test first asserted that no terrain reads blank; the loader answered 36, and all 36 were right - open air, ledges
and the inside of a wall are drawn as nothing. The count is pinned now instead, because a loader that dropped the field would read 1,109 blanks and the number would say so.

## Every square knows its glyph (round 388)

The frame the port will draw needs three things per square: what is there, how brightly it is lit, and how clearly it is seen. Two of the three existed. The glyph did not - the
terrain loader read the id, the move cost, the flags and the transparency, and threw the symbol away.

| Piece | Reference | Port |
|---|---|---|
the glyph | `map_data_common_t::sym` (`src/mapdata.h`) | `Terrain::symbol`, from the dump's byte array |
the encoding | a one-grapheme string | decoded from UTF-8, first character kept - `[46, 46, 46, 46]` is four dots for one `.` |
the gate | - | every one of the 1,109 terrain entries has one; a wall is `#`, a floor is `.` |

**A count that disagreed was the dump's shape, not a bug.** The first version of the test asserted 1,769 terrains and the loader returned 1,109. Both are right: the dump holds
1,769 entries of *both* kinds - terrain and furniture - and this loader reads the terrain ones, which is the same split the scent-flag test found two rounds before. The test now
pins the terrain count and says why the dump's total is not it, in the same words as its neighbour, because the same mistake was made twice.

## The sim gathers in two phases too (round 387)

Round 386 gave the level the reference's buffer; the sim still cast sources as it found them. It gathers in two phases now, in the reference's order:

| Phase | Reference | Port |
|---|---|---|
1. record | `add_light_source` for terrain, furniture and fields (`lightmap.cpp:528-541`) | `light_sources` returns the emitters as the buffered list |
2. apply | the buffer drained through `apply_light_source` (`:654-659`) | `apply_buffered_sources`, neighbour skips included |
3. placed | items and creatures cast directly | `LightSource` components, cast unbuffered |

**Every existing light test stayed green through the change**, which is the point: the two phases are the same answer computed in a different order, and the tests that pin the
numbers - a lamp's own square, a fire's three intensities, the smoke that dims what is behind it, the turn that rebuilds the map - are what proves the reordering did not move
anything.

**The new receipt is the drained buffer.** After a cast the level holds no buffered sources, because the next build starts from zero (`lightmap.cpp:463-464`). A test with a
lamp and a fire side by side checks three things at once: the lamp keeps its own light, the fire beside it is lit, and the room east of the pair keeps the lamp's brightness -
the skip rule must never darken a square, which is the failure mode a badly-mapped direction table would produce.

## Buffered sources, and rays a neighbour already casts (round 386)

The reference does not cast a light source when it reads it. It **records** it, walks the whole map, and only then casts (`lightmap.cpp:463-464`, `654-659`, `666-670`).
Recording first is what lets a square ask "is my neighbour already lit at least as brightly" - and if it is, the reference skips that direction entirely (`:1257-1290`):

```text
record every buffered emitter -> for each recorded square, cast only the directions
                                  whose neighbour is not at least as bright
```

| Piece | Reference | Port |
|---|---|---|
the buffer | `light_source_buffer`, `max` of two sources on a square | `Level::add_light_source`, `Level::buffered_sources` |
applying | loop over the buffer calling `apply_light_source` | `Level::apply_buffered_sources` |
the skip | `buffer[neighbour] < luminance` decides each of four directions | the same test, through `OCTANT_DIRECTIONS` |
unbuffered | items and creatures cast immediately and are not recorded | `Level::apply_light_source_with` |
draining | the buffer is zeroed each build | cleared when applied, pinned by a test |

**The observable consequence is a test.** Two buffered fires side by side give the *same* light east of the pair as the eastern fire alone - the western one skips its eastward
rays - while an unbuffered source beside them still casts in every direction. That is the difference between "the port casts everything" and "the port casts what the game casts",
and it is the kind of difference that never shows in a screenshot.

**A table of directions beats eight magic indices.** The port's transform table is the reference's eight octants in the reference's order, so a `Direction` per octant names the
mapping once and lets the skip rule be read as four sentences instead of a bitmask.

## The frame asks once (round 385)

Round 384 built the sight cast once; the rule functions still built one per square. They take the cast now, so a draw loop casts the sight once and reads forty-nine squares from it.

| Entry point | Takes | Use |
|---|---|---|
`lit_level_with`, `apparent_light_with` | a `SeenGrid` | the frame |
`lit_level_at_square`, `apparent_light_at_seen` | nothing | one question, a test, a tool |

**The wall's-face rule now reads its neighbours from the cast too.** The reference consults `seen_cache[neighbour]` (`lightmap.cpp:773-779`); the port asked `seen_at` and would
have built a whole cast to answer about one neighbour, which is the same bug one layer down. A neighbour outside the cast reads zero and is skipped, which is what the reference's
cache gives for a square it never saw.

**Eight parameters became a closure.** Passing the cast's neighbour test as an argument was the smallest honest way to keep one body for both entry points; the alternative was two
copies of the quadrant table, which is how the two answers would start to disagree.

## One cast, many readers (round 384)

Round 383 gave sight its fraction but asked the cast once per square. The reference builds a level's `seen_cache` **once** and every reader asks it, and a frame asks about
fifteen thousand squares.

| Shape | Reference | Port |
|---|---|---|
the cast | `build_seen_cache` fills the cache, rebuilt when the level is dirty | `Level::seen_grid(origin, radius, transparency_of) -> SeenGrid`, one build |
reading it | `map::seen` | `SeenGrid::at(square)`, outside the cast reads as not seen |
one square | the same cache, one key | `seen_through`, now a grid of one question |
the receipt | - | `Level::sight_casts()`, a count the test asserts is **one** for 144 squares |

**The arithmetic test is the one that matters.** A grid that answers a hundred times faster and slightly wrong is worse than the slow one, so the second test works out the
expected value from the reference's own formula - `exp( -OPEN_AIR * d )` along an open corridor - rather than asking the port. It agrees to within 0.02 over six squares.

**A wall ends the cast and the squares beyond it read zero**, which the third test pins separately, because "attenuates" and "stops" are different claims and a cast that only
attenuates would dim the room behind a wall instead of hiding it.

## Sight is a fraction, and that is how smoke glows (round 383)

Round 382 declared the gap: with visibility as a yes or a no, `BrightOnly` was unreachable through the world. It is a fraction now.

| Piece | Reference | Port |
|---|---|---|
the sight cast | `cast_seen_light` starts each ray at `VISIBILITY_FULL` and accumulates `sight_calc` (`lightmap.cpp:1039-1100`) | `Level::seen_through(origin, radius, transparency_of, square) -> f32` |
"seen" | `seen_cache > 0` | unchanged, `seen_at` |
"hazy" | `seen_cache <= LIGHT_TRANSPARENCY_SOLID + 0.1` (`lightmap.cpp:733`) | the same fraction, and `View` gains `transparency_of` |

**And the branch is reachable.** A skin of smoke between the character and a bright square makes that square a glow: the cast measures **0.078** there, under the tenth, so the level
is `BrightOnly` - drawn as light with nothing in it.

**A quirk of the reference reproduced rather than corrected.** The square *past* the smoke measures 0.43, brighter than the square behind it, because the accumulator averages
transparency across a whole octant row: one column of smoke is diluted by the open air beside it. That is the reference's arithmetic, not a port mistake, so the test looks at the
square immediately behind the smoke and the comment says why.

**The cache question answered the honest way.** The first draft cached the fractional cast keyed by origin and radius - and hit the same class of bug this file carries a scar
from, because a closure cannot be part of a cache key. `seen_through` builds its cast each time and says so; a caller drawing a frame will build one cast and read it, which is the
shape the reference has (`seen_cache` per level, rebuilt when dirty).

## One question, two callers (round 382)

The draw path needs two answers about a square - "is it drawn at all" and "at which level" - and a port that computes them separately will eventually have them disagree.
`lit_level_at_square` is now the one body: `shown_at` is `!= Blank` and nothing else, and a test walks three squares through both and asserts they agree.

**The round's real work was a bug the split exposed.** `apparent_light_at_seen` returned the light map's value for any square that was not an opaque wall - including squares
out of the cast, where the reference multiplies by a visibility of zero (`lightmap.cpp:795-798`). Round 377's arithmetic had that multiplication in `shown_at`; moving it into
the quadrant function dropped it. The test that caught it is the one written in 377 against exactly that mistake, which is the argument for writing the test where the rule
lives rather than where the call happens.

**`BrightOnly` is unreachable through the world, and the tests now say so.** `obstructed` is `vis <= LIGHT_TRANSPARENCY_SOLID + 0.1` on a **fractional** seen cache: a wall
you look at is seen clearly, a fogged square is not. With visibility as a bool the branch can only be reached by handing the classifier its inputs directly, so that is what
the unit test does, and a second test pins what the world currently answers for a wall in plain sight - `Bright`, not a glow. Declared in the roadmap as the next step of T6.

## The face of a wall counts only the light it faces (round 381)

`apparent_light_at` took the brightest of a square's four quarters whatever the square was. The reference has two branches (`lightmap.cpp:748-798`):

| Square | Reference | Port |
|---|---|---|
opaque and visible | walk the eight neighbours, keep the non-opaque => visible ones, count only the two quarters each faces | `apparent_light_at_seen`'s neighbour table |
anything else | `vis * lm.max()` | unchanged |

**Why the second branch exists at all.** "For a non-opaque tile light from all directions is equivalent" - a floor square's four quarters differ only by where the lamp
happens to stand, so the brightest is the honest answer. An **opaque** square is different: three of its quarters may face into the wall itself, and counting them lets light
from behind a wall brighten the face the player is looking at.

**The test that proves it uses the quarters the player cannot see.** The fixture is a six-square block of wall with the player standing east of it; the light map carries a
hundred in the north-west quarter and five in the two the player can see. The honest answer is five. The old code answered a hundred, and the test says so in its failure
message.

**One test measured nothing because its own fixture said nothing was opaque.** The first run answered a hundred even with the new rule in place: `view()` in the test file
declares `blocks_light` as "nothing blocks", so the square was not a wall to the function under test. Fixed in the fixture, not in the rule - which is the right direction, and
worth writing down because a test double that answers "no walls" makes every wall rule invisible.

## A turn ends with the light it made (round 380)

Round 379 taught the light map to read the world's own emitters. Nothing rebuilt it, so a fire that started this turn was invisible until something else asked. The turn now
ends with a clear and a fresh cast, which is where the reference puts it: `map::build_map_cache` runs in the draw path, **after** the turn's fields have been processed.

| Rule | Why it matters | Test |
|---|---|---|
clear, then cast | the cast merges by maximum, so a fire that went out would stay lit by the previous turn's answer | `a_square_whose_fire_is_gone_is_dark_at_the_end_of_the_next_turn` |
cast at the end | the frame a player sees is this turn's fire, not last turn's | `a_fire_burning_through_a_turn_is_lit_at_the_end_of_it` |

**The red gate failed in the shape it should.** The first run panicked on `Requested resource ... does not exist`: no `Light` at all, because nothing had ever built one during a
turn. That is a better failure than a wrong number - it says the piece is missing rather than that it is wrong.

**Two test doubles disagreeing is a signal, not noise.** The new suite's `HardFloor` first declared methods the `FireMap` trait does not have, because it was written from
memory of an older shape. The trait's six methods are what a fire actually asks the map; the port keeps them that way deliberately, so the double is where that shows.

## The world lights itself (round 379)

`map::generate_lightmap` walks every square and adds a light source for anything whose own data says it emits one (`lightmap.cpp:525-547`): terrain, furniture, each field on
it, plus the items and the creatures the port has none of yet. The port gathered only the `LightSource` components a caller had spawned, so a burning square lit nothing and a
lamp terrain was dark.

| Emitter | Reference | Port |
|---|---|---|
terrain | `terrain->light_emitted > 0` | `TerrainTable::light(code)`, from the dump's `light` |
furniture | `furniture->light_emitted > 0` | `TerrainTable::light(code)` on the furniture table |
fields | each field's level `light_emitted` | `FieldTable::light(id, intensity)` - `fd_fire` is 20, 60, 160 |
placed | `LightSource` | unchanged |

**A red gate caught a bug in the light cast itself.** The lamp test read zero, and the reason was not the emitter table: `apply_light_source` writes
`max( lit_level::LOW, luminance )` to the source's **own square** before any ray is cast (`lightmap.cpp:1229-1233`), and the port never did. A lamp lit the room and left
itself dark. The fix went into `Level::add_light_source_with`, where the reference has it.

**And it corrected a test that had pinned the wrong answer.** `a_dim_source_lights_nothing_at_all` asserted an empty light map for a luminance-one candle. The reference
writes one unit to that candle's own square and then returns without casting, so the honest test is "one square, nothing else" - the old one was pinning the port's bug.

## A turn starts with the smell (round 378)

The reference's turn begins with the character's scent and only then touches the fields (`do_turn.cpp:642-658`):

```text
scent.set( player square )  ->  scent.update( player square )  ->  floor caches  ->  process_fields
```

The port had the scent marks and the diffuser but nothing joined them to the turn. `advance_scent` does now, and `run_environment_turn` calls it **first**.

| Piece | Reference | Port |
|---|---|---|
the deposit | `scent.set( u.pos, u.scent )` | `advance_scent(world, center, deposited)` |
the spread | `scent.update( u.pos, m )` over the character's radius | `ScentWindow` round the same centre, already pinned by six tests |
what the world already holds | the window reads the map | the grid is loaded into the window before it diffuses, so scent stays in the world rather than following the character |
the marks | `NO_SCENT`/`REDUCE_SCENT` terrain | `ScentTable`, from the loaded flags |

**The window is the reference's, not mine.** A draft of this round diffused the whole level's grid and disagreed with the pinned arithmetic on its first two tests; the reason is
the one `scent_map` encodes: the window is the character's radius, and `squares_used` - and so every answer - depends on how many squares take part.

**A test written with a small source proves nothing.** The first version of the spread test used a hundred units and measured zero on both sides, because the reference's
weights work in tenths and the absorption term takes a tenth of a hundred entirely. The numbers in these tests are large enough to survive the arithmetic, which is what makes
"the wall holds it in" a claim rather than a coincidence.

## Visibility multiplies the light, and the square you stand on is always seen (round 377)

The port asked "is there any light at all" and called that sight. The reference asks `apparent_light >= vision_threshold`, where **`apparent_light` is the light map
multiplied by the square's visibility** (`lightmap.cpp:795-798`). Two things followed from putting that right:

| Rule | Reference | Port |
|---|---|---|
apparent light | `vis * lm.max()`, quadrant-selected on an opaque tile | `shown_at` multiplies the brightest quadrant by the cast's answer |
the threshold | `lit_level_at(...) != Blank` | the chain from round 376, asked once |
the character's own square | `seen_cache[origin] = VISIBILITY_FULL` before the cast (`lightmap.cpp:1057-1059`) | `Level::seen_at` answers true for the origin |

**Two tests caught the two halves.** Without the multiplication, a bright room behind a wall was "shown" - the light map has a value there even though the cast does not. And
without the origin rule, the square the character stands on was blank, because the port's cast does not light its own origin (it matches the reference's shadowcasting
fixture there, which is right for the *light* map and wrong for the *seen* map).

**`shown_at` had no tests at all.** It was written three rounds ago, referenced from the ledger, and never pinned - so this round is also the one that gave it tests, three of
them: shown above the threshold, not shown below it, and not shown behind a wall however bright.

**Eight arguments became one type.** The first version of the new signature took eight parameters and clippy refused it, which was the right answer: `View` is what the
reference reads off the avatar, and naming it says so.

## Bright, lit, low, and a glow with nothing in it (round 376)

The port could say whether a square was lit, not **how** it looked. `lit_level` (seven values) existed; the chain that picks one did not. It does now:

| Condition | Answer | Reference |
|---|---|---|
hazy, and brighter than the ground | `BrightOnly` | `lightmap.cpp:826-833` |
hazy otherwise | `Blank` | same |
above `SOURCE_BRIGHT`, or items on a square the character can see | `Bright` | `lightmap.cpp:836-838` |
above `AMBIENT_LIT` | `Lit` | `lightmap.cpp:839-841` |
at or above the vision threshold | `Low` | `lightmap.cpp:842-845` |
anything else | `Blank` | same |

**Two thresholds in the reference are the same number, so one branch is dead.** `LIGHT_SOURCE_BRIGHT` and `LIGHT_AMBIENT_LIT` are both ten, so the `> SOURCE_BRIGHT` test
above has already claimed every square the `> AMBIENT_LIT` test could find: `Lit` is unreachable. The port keeps the branch, in the reference's order, and a test pins the
equality and the unreachability - because the tempting "fix" is to reorder the chain into something that disagrees with the game.

**A pinned count caught a real question and answered it the other way.** The scent test asserted 245 terrain pieces carry `REDUCE_SCENT`; the loader said 225 and the loader
was right: 20 of those entries are **furniture**. The test now pins the terrain split and says why the dump's total is not the terrain's.

## A wall marks itself for scent (round 375)

The scent ledger row carried a declared gap: "reading the terrain flags off the map (the caller marks squares)". That is closed. `ScentTable` reads the two flags out of the
loaded terrain prototypes and `scent_grid` marks the level's squares from their own codes, so a wall flags itself.

| Piece | Reference | Port |
|---|---|---|
the flag | `TFLAG_NO_SCENT`, `TFLAG_REDUCE_SCENT` (`mapdata.h:181-193`) | `ScentTable::marks`, loaded per code |
the grid | `scent_map`'s array, marked by `update` | `Scent` resource, one cell per square, row by row with x fastest |
the fade | `scent_map::decay`, once a turn before the fields | `decay_scent` |
the arithmetic | `scent_map::update` | **not here**: `ScentWindow` in core-types, already pinned by six tests |

**The sim owns the marks; the arithmetic stays where it was transcribed.** My first draft put a `diffuse_scent` in the sim as well, and it disagreed with the pinned
`ScentWindow` on the first two tests - because a whole-level grid is not the character's window, so `squares_used` differs and the answers with it. The draft is deleted
rather than debugged: a second implementation of a pinned rule is a liability, not a port.

**The same code answering two ways is the test that matters.** A table that marks plain air as blocking changes the answer for the same terrain code, which is what "loaded
from the data" has to mean - otherwise the hard-coded rule has only moved from a match arm into the sim.

## A clear sky costs nothing, and fog costs 1.7 (round 374)

The weather was a bare `f32` at every call site: `cast_lights_with(world, 1.7)` says a number and hides which weather it is, and nothing refused a caller that passed `0.5`.

| Before | After |
|---|---|
`cast_lights_with(world, sight_penalty)` | `cast_lights_with(world)` |
`cast_sunlight_with(world, turns, sight_penalty)` | `cast_sunlight_with(world, turns)` |
the number, passed by hand | `Weather`, a resource, defaulting to a clear sky |

**The refusal is the point.** `Weather::with_sight_penalty` panics below one, because that direction is not a matter of taste: the multiplier raises a square's absorption, so
rain is 1.1, snow 1.3 and fog 1.7 in the data, and a value below one would make bad weather *brighten* the world. A test pins the panic rather than leaving it to a doc
comment.

**An absent resource means a clear sky.** Every existing light and sun test kept working without inserting anything, which is the right default for a world that has not
loaded weather yet - and it is why the change could be made at all without touching a hundred fixtures.

## One place the world keeps what a code means (round 373)

The light and the sun each took their tables as arguments, the sight query took them from a resource, and the fields were a closure in one place and a grid in another. Three
shapes for one fact. Now there is one: `Prototypes` (a terrain table and a furniture table) and `FieldTable` in the world, and one entry point each.

| Before | After |
|---|---|
`cast_lights(world, terrain_fn, field_fn)` | deleted |
`cast_lights_with_furniture(world, terrain_fn, furniture_fn, field_fn)` | deleted |
`cast_lights_with(world, &terrain, &furniture, penalty, field_fn)` | `cast_lights_with(world, penalty)` |
`cast_sunlight_with(world, turns, &terrain, &furniture, penalty)` | `cast_sunlight_with(world, turns, penalty)` |
`Map::terrain_transparency` | deleted: the last hard-coded light rule is gone |

**Deleting the hard-coded rule was the point.** `Map::terrain_transparency` said code zero was solid and every other code was air - a rule that was true of the corpus and
would have been wrong about any world that loaded different prototypes. Its two remaining users were tests, and both of them now say what they mean: one inserts a table
with code zero opaque, the other inserts the table the data gives.

**A test caught the deletion's edge.** With the rule gone, a terrain code the table has never heard of answers "open air" - which is right, and which silently unblocked a
wall in a fixture that had relied on the old default. The fixture now marks its wall opaque, which is what a wall is.

## Smoke you can see through and smoke you cannot (round 372)

Sight counted the terrain and the furniture and forgot the third thing the reference checks: the fields. `calc_transp` multiplies each field's `translucency` into a square's
value (`lightmap.cpp:158-165`), and `map::sees` picks between the two casts - with the fields and without - with a member pointer at the top.

| Piece | Reference | Port |
|---|---|---|
the number | `field_intensity_level::translucency`, one per intensity | `FieldLevel::translucency`, read from the dump |
the table | the field registry | `FieldTable`, a resource, indexed by type and intensity |
the choice | `is_transparent` / `is_transparent_wo_fields` | `sees` / `sees_through_fields` |

**The flag and the number are not the same question.** Smoke is marked opaque from intensity two, and its translucency there is **ten** - a heavy dimming, not a wall. Only
intensity three has zero, and that is what blinds. A port that read `transparent` and stopped there would make thick smoke visible-through, or make thin smoke solid; both are
wrong in a way you only notice when something is shooting at you.

**The oracle had to move first.** The field dump carried the `transparent` flag and not the translucency, so the loader had nothing to load. The dump now carries both at
version 2, `corpus/prototypes/fields.json` is regenerated from it, and the loader test reads the numbers back - 1, 10 and 0 for smoke, 0.7, 0.4 and 0.1 for fire. The
hard-coded table this replaced was written the same day and deleted in the same round.

## A wall is seen as a wall; what is behind it is not (round 371)

The port had `map::seen` - the shadow cast that answers "is this square in the field of view" - but not `map::sees`, the straight-line question "can A see B", which is
what aiming, line of fire and creature sight all ask. It is a different rule and it is now ported:

| Gate | Reference | Port |
|---|---|---|
range | `range >= 0 && range < rl_dist( F, T )` | same, with `rl_dist` as Chebyshev until the "circular distances" option exists |
bounds | `!inbounds( T )` | the caller's `in_bounds` closure, and a line that leaves the level is blocked where it leaves |
the walk | `bresenham`, stopping **before** testing the target | `line_to`, with the same early return |

**The last square is the whole point.** The reference exits the walk before testing the square it is looking at, with a comment saying so (`map.cpp:7345-7347`): a wall is
visible *as* a wall, and only what lies behind it is hidden. A port that tested the target like any other square would make walls invisible, which is a mistake you can only
see from the player's side of the screen - and the test pins it directly.

**The corner is a tie-break, not a policy.** Two walls meeting at a corner, with the sight line running exactly through the gap: the answer comes from which cell Bresenham
takes first, so the test pins the answer rather than the intent, and says why.

## One sun, not two (round 370)

The cascade replaced the stand-in last round, and this round removed the stand-in. `Level::daylight`, `light_in_from_outside` and the `FACING` table are gone, along with
the seven tests that pinned them, and `cast_sunlight` - the closure-shaped entry point everything older calls - now runs the same three rungs as the prototype-driven one.

| Claim the deleted tests made | Where it is pinned now |
|---|---|
an outdoor tile takes the sun in all four quarters | the cascade's "fully outside" rung, and the sim's noon test |
a doorway lights only the quarters facing it | the cascade's shaft tests, read in quarters |
daylight never dims a corner a lamp already lit | `Light::merge`, which takes the brighter of two answers |
where light enters from, quarter by quarter (the `FACING` table) | `CARDINAL_QUADRANTS`, private to the cascade and pinned by its own tests |

**Nothing was deleted before its claim was covered.** The `FACING` table said which two quarters face each direction - north lights NE and NW, east lights SE and NE, and so on -
and that is exactly what `CARDINAL_QUADRANTS` carries, so the table was a second copy of one fact rather than a fact of its own.

**The adapter test passes either way, and that is worth saying.** It was written to be a red gate and it was not one: the stand-in already produced the doorway shape, because
`light_in_from_outside` was transcribed correctly. What was wrong was not the shape but *where the light came from* - the room's own ceiling rather than the sky. So the honest
receipt for this round is the deletion, not the test.

## The sun arrives from above, and a doorway is a shape (round 369)

The sim's daylight was a hand-rolled stand-in: "a tile with open sky on any side takes the sun". It is now the transcribed cascade, walked over **three rungs** because a
one-level world still has three levels in it:

```mermaid
graph TD
    A[The empty sky: lit by the sun, ends nothing] --> B[The roof layer: its opaque and floored squares end the 'fully outside' regime]
    B --> C[The ground: the first level to do the per-square work]
```

| Question | Answered by | Why not the other thing |
|---|---|---|
is there a roof above this square? | the square's own `INDOORS` flag | the outside cache's **three-square stamp** would give every wall a phantom ceiling and seal every doorway - a test caught exactly that |
is this square charged for the weather? | the outside cache's stamp | that is what `build_outside_cache` computes, and it is deliberately generous at edges |
what does the square above deliver? | `AboveLayer`: transparency, floor, light | the roof belongs to the level above, not to the room |

**The claim had to be read in quarters.** `max()` hides the whole point: a square under a roof still has a bright quarter where the doorway is, so a test comparing maxima
cannot tell the cascade from a uniform fill. The test now asserts that the door-facing quarters are bright and that an interior square, whose four cardinals are all roofed,
gets nothing.

**And two of my fixtures were wrong before the code was.** A "far corner" of a room is still lit from the open side, and a single roofed square in open ground is lit from all
four cardinals - both are the rule working, not failing. Interior squares are what a test of "nothing spills in" needs.

## The roof that stops the sun belongs to the level above (round 368)

`build_sunlight_cache` reads three things for every square it tests - the transparency cache, the floor cache and the light map - and all three come from the **previous level**
(`lightmap.cpp:367-371`). Only the weather reads the level being lit. The port had bundled the wrong three together, which meant a roof could not stop the sun:

| Square of the level above | Effect on the square below |
|---|---|
opaque | no light: the ray never gets through |
has a floor | no light: that floor is a **ceiling** to the room underneath |
unlit | no light: there is nothing to come through |
otherwise | `clamp( light * OPEN_AIR / transparency, indoors, light )` |

The fix is a type: an `AboveLayer` carrying those three answers, so the pass cannot accidentally read the right question from the wrong level. A test pins both directions - a
floor above the square keeps it dark, and removing that floor lets the sun through - and it then caught two more of my own tests, written against the bundled version, which
had been asserting the room's own transparency decided the answer.

**"Fully inside" is a latch and not a flag to reset by hand.** The second half of that test first reused the same pass, and got the indoors value for a level that was open to the
sky, because the first call had latched `fully_inside`. The reference does exactly this: once nothing gets through, every level below is filled without a single square being
examined. The test now starts a fresh pass, which is the honest way to model "a different world", not a reset.

**And `step` now hands in an open sky.** A single-level world has nothing above it, and the reference's first iteration is an empty level of open air lit by the sun itself
(`lightmap.cpp:310-315`). Passing that layer in explicitly is what makes the top of the port's world behave like the reference's: the sun arrives from above, not from nowhere.

## Fog costs the square in the open and nothing to the one under a roof (round 367)

The sim has passed `outside: false` and `sight_penalty: 1.0` on every square since the light was first cast, which made weather free. The outside cache from round 360 is now
wired into the cast, and the order of the two is the reference's own: **the sky is decided first**, because the transparency rule charges the weather against it
(`build_map_cache`, `map.cpp:9556-9557`).

| Piece | Reference | Port |
|---|---|---|
the sky | `build_outside_cache`: an `INDOORS` flag on either layer covers a 3x3 block | `cast_lights_with` builds it from the level's two layers before measuring anything |
the charge | `value *= sight_penalty` on squares in the sky (`calc_transp`) | `sight_penalty` reaches `transparency_of` per square, so an indoor square pays nothing |
clear weather | `sight_penalty: 1.0` | multiplying by one, so a clear sky leaves every existing light test untouched |

**The multiplier is above one, and that is not a mistake.** `weather_type.json` gives rain `1.1`, snow `1.3` and fog `1.7`, all `>= 1`, because the value raises the square's
absorption rather than lowering a transmission - the same backwards-looking direction as a field's translucency. A test written with `0.5` "rain" measured nothing at all,
because a penalty below one would make bad weather *brighten* the world.

Three entry points now funnel into one: `cast_lights` (terrain only), `cast_lights_with_furniture` (both layers, clear sky) and `cast_lights_with` (both layers and the
weather). The first two are the older, narrower shapes and their callers own their own geometry; only the third builds a sky map.

## Two layers, one conjunction (round 366)

`calc_transp` asks one question of a square and it is an `&&`:

```text
get_ter().transparent && get_furn().transparent     (lightmap.cpp:146-149)
```

Either layer being opaque stops the ray, and it does not matter which. The sim could not express that, because `Level` had no furniture layer at all - the reference's
submap keeps furniture beside terrain, and the port kept only the terrain.

| Piece | Reference | Port |
|---|---|---|
the layer | `submap::get_furn`/`set_furn`, materialised on first write | `Level::furniture_at`/`set_furniture` over the submap's own furniture page |
"nothing here" | `f_null`, code zero | `Some(NULL_ID)` on a made square, `None` on one the world never made |
the rule | the conjunction above | `cast_lights_with_furniture` takes both codes and asks both tables |

**A made square and an unmade one are different answers.** A tile that exists with nothing on it holds the null furniture, not "no furniture layer"; a tile the world has
never made has no layer to hold anything. The test was written the other way round first and the code was right: the square that had just been given a floor already
answered `Some(0)`.

**The floor does not save the table.** A glass floor under an opaque table stops the light, which is the case the sim could not express while it passed
`furniture_transparent: true` unconditionally - a room lit straight through solid oak furniture.

## A window costs the light nothing and a wall costs it everything (round 365)

The sim's light path had one hard-coded answer left: terrain code zero was solid, every other code was open air. The per-terrain `transparent` bool is now loaded, so the
answer is data - and `sim::terrain_table::TerrainTable` is where the two meet.

| Rule | Value | Why it is not a fraction |
|---|---|---|
a transparent terrain | exactly `LIGHT_TRANSPARENCY_OPEN_AIR` | `calc_transp` does not interpolate; a window dims nothing |
an opaque terrain | `0.0` | the cast's `sight_check` fails, so the square is a wall to the light |
a code the table has never seen | open air | a table that is not told about a code must not black out the map |

**Codes are the caller's.** The level stores terrain as a number, and what that number means is decided by whatever built the level, so `TerrainTable::insert(code, transparent)` is the
whole seam. Nothing in the sim guesses that code 2 is `t_wall`.

**The test was checked for teeth.** Setting the opaque branch back to open air fails three of the five cases, including the one that matters - "the wall shades the far side:
8.25582 against 8.25582" - so the assertion is doing the work rather than agreeing with itself.

Still hard-coded: `Map::terrain_transparency`, kept for callers with no table. It is the last hard-coded answer in the light path and the ledger says so.

## Transparency is stored, not derived, and the data says so (round 364)

`map_data_common_t::transparent` (`mapdata.h:573`) is a **bool on the object**, mutated as the prototype loads. Three measurements, each of which contradicts a plausible
guess:

| Guess | Data | Rule |
|---|---|---|
"a new prototype is transparent" | 373 of 1,109 terrains are opaque, and the 12 with no flags are opaque too | an ordinary prototype starts **opaque** |
"null prototypes are opaque" | `t_null` and `f_null` are **transparent** | `null_terrain_t`/`null_furniture_t` set the bool themselves and nothing loads over them |
"the flags decide it" | the flags agree with the bool on every entry | true, but only because every prototype that wants light names a flag |

The loader's rule, transcribed from `map_data_common_t::set_flag`/`unset_flag`/`unset_flags` (`mapdata.cpp:876-912`, `:1112-1122`):

* `set_flag` makes it transparent for `TRANSPARENT` **or** `TRANSLUCENT` - a window passes light without passing detail, and the light cache only asks about light;
* a child that names its own `flags` array calls `unset_flags`, which clears the bool **before** the new list is applied - this is why twelve prototypes are opaque with
  an empty flag list, and why the bool cannot be recovered from the final flags;
* `delete` of `TRANSPARENT` clears the bool **even if `TRANSLUCENT` remains**, while `delete` of `TRANSLUCENT` leaves it alone. That asymmetry is the reference's own; a
  port that neatened it would disagree on a window that deletes a flag.

**The oracle had to grow to settle this.** The terrain dump carried name, symbol, colour, move cost, light, coverage and flags, and none of them can express "opaque with no
flags saying so". It now carries `transparent` (dump version 2), and the whole content set compares field by field: **0 mismatches across 1,769 prototypes**, after the port
was corrected twice by the data rather than by reasoning.

## One cast, and a multiplier that reads backwards (round 363)

The level's light path was the last place still guessing: it had its own cast that picked a quarter by comparing offsets, and its own transparency rule that kept the
**most opaque** field on a square. Both are gone. `Level::add_light_source_with` now asks the transcribed transparency rule and hands the grid to the transcribed octant
sweep, so there is one cast in the port and one answer to "what does this square cost".

| Old | New | Why the old one was wrong |
|---|---|---|
most opaque field wins | every field **multiplies** | two layers of half-smoke are a quarter of the light, not a half |
quarter chosen by comparing offsets | the sweep's own quadrant, with `default_` for transparent squares | a wall must light the side facing the lamp and **not** the side facing away |
`cast_light_values` | `cast_light_quadrants` | the old path had no span recursion, so a dim square did not shadow what was behind it |

### The measurement that found it

The corner test asserted that a floor tile north-west of a lamp lights its **south-east** quarter. Probing the transcribed sweep showed it lights its **north-east**
quarter - because the reference writes `quadrant::default_` for a square it can see through, and `default_` **is** `NE` (`shadowcasting.h:26-31`), with a comment saying
"for non-opaque tiles direction doesn't matter". Only an **opaque** square gets the octant's own quadrant, which is what makes a wall read as a wall. The old test was
pinning a guess; the new one pins the measured rule for both cases.

### And the sign of a field's translucency

The reference attenuates by `e^( transparency * distance )`, so a **higher** transparency is *more* absorbing. Field data confirms the direction: open air is `2.3/72`,
smoke at its second intensity is `10` (`field_type.json`), and `fd_fire` uses `0.7`, `0.4`, `0.1` - each *less* absorbing than open air, because a fire is meant to be seen.
Dense smoke's `translucency: 0` multiplies the square to zero, and `sight_check` requires `> 0`, so it is **solid**: the third intensity of smoke blocks sight outright.

A port that read `translucency` as an opacity in the intuitive direction would brighten everything behind a fire and blind nothing behind smoke. The sim's own test was
using `0.38` for smoke and had to be corrected against the data rather than against intuition.

## Five caches, one order, and a reason for each step (round 362)

`build_map_cache` (`map.cpp:9549-9569`) is the door every light question goes through, and its order is the behaviour:

```mermaid
graph TD
    A[Sky: is there open air above this square?] --> B[Transparency: charge the weather for the squares in the sky]
    B --> C[Floor: what is underfoot, and is the level sealed?]
    C --> D[Vehicles, in a separate pass over every level]
    D --> E[Vision: copy the finished transparency and apply its three exceptions]
```

| Step | Reads | Why it cannot move |
|---|---|---|
sky | terrain and furniture indoor flags | transparency charges the weather **against the sky it just computed** |
transparency | the sky, fields, terrain | floor's dirty flag feeds the seen cache, so it wants to be settled first |
floor | terrain flags and the level below's furniture | vehicles write the floor of the level **above**, so they cannot be inside this loop |
vehicles | parts, doors, decks | the reference says it: otherwise "such changes might be overwritten by main cache-building logic" |
vision | the transparency cache | it **starts as a copy**, so anything writing transparency after it is invisible to sight |

**Two counter-intuitive consequences, both pinned by tests:**

* **smoke dims sight.** The vision cache copies the *with-fields* transparency, so smoke is a sight cost. What smoke does not do is make a square count as
  *not visible* - that is the `without_fields` answer, which the seen cache reads and this join leaves alone.
* **an open car door is opaque to neither cache.** The opacity test is on the part, and an openable part with its door open cancels it, so light and sight both pass. A closed
  door blocks both, because vision copies the light cache rather than recomputing it.

## A floor is what stops the sun, and a greenhouse is the exception (round 361)

`build_floor_cache` (`map.cpp:9417-9471`) answers "is there something underfoot" per square, and the sunlight pass reads the answer **one level up**: a square receives daylight
only when the level above has no floor there.

```mermaid
graph TD
    A[Start with a floor everywhere] --> B{Does the terrain open this square?}
    B -->|no| C[Floor stays]
    B -->|yes| D{Is there a greenhouse roof on the level below?}
    D -->|yes| C
    D -->|no| E[No floor, and the level is no longer sealed]
```

| Rule | Detail | Consequence |
|---|---|---|
four flags open a square | `NO_FLOOR`, `NO_FLOOR_WATER`, `GOES_DOWN`, `TRANSPARENT_FLOOR` | four different ways to fall: nothing there, water there, a stair there, glass there |
`SUN_ROOF_ABOVE` **below** closes it | furniture on the level *under* this one | a greenhouse catches the sun on its own level instead of passing it down |
`no_floor_gaps` | true only when the whole level is sealed | one hole anywhere lets a caller skip nothing |

**The roof has to be read from the level below, not this one.** A roof above you is a ceiling; a roof *below* you is what cancels the hole, because the sunlight pass is reading
this cache while it lights the level underneath. Reading the same level's furniture would make every greenhouse seal the sky above itself instead of catching it - a test pins
each direction.

## Under open sky: one flag and a three-square stamp (round 360)

`build_outside_cache` (`map.cpp:9260-9314`) answers one question per square - is the sky above it - and the last three rounds all quietly trusted the answer:

```mermaid
graph TD
    A{Is the level below ground?} -->|yes| B[Everything is inside, one fill]
    A -->|no| C[Start with everything outside]
    C --> D[For each square flagged INDOORS]
    D --> E[Mark it and its eight neighbours inside]
    E --> F[That is the sky's map]
```

| Rule | Detail | Why it is not just a copy of the flag |
|---|---|---|
the flag | terrain **or** furniture carries `INDOORS` | an awning or a tent is furniture, and it shelters the square |
the stamp | a **3x3 block** of inside per indoor square | a doorway, a threshold and a wall's own square count as indoors without being listed |
below ground | a single fill of `false` | there is no sky down there to ask about |

**The padded cache is not an optimisation detail, it is part of the rule's shape.** The reference allocates the answer one square larger on each side, marks it all outside, and lets
each stamp write its full nine squares; the two that fall off the map at an edge are swallowed by the padding, so no bounds check is needed and an indoor tile in the corner
still marks its whole neighbourhood. A port that wrote through an unpadded index would panic or wrap - a test pins that the padding does not wrap a corner stamp onto the far
corner of the map.

**The stamp is why a building's light comes in at the windows.** Two squares from a wall is already outside again, so the sun reaches the ground beside a hut and not the
floor inside it, and the weather penalty lands on the right squares.

## Seeing is not lighting: three exceptions the vision cache makes (round 359)

`build_vision_transparency_cache` (`lightmap.cpp:207-286`) starts as a **copy** of the light transparency cache and then changes it in three ways. Each is somewhere that
seeing and lighting disagree, and each is load-bearing:

| Exception | Rule | What breaks without it |
|---|---|---|
translucent terrain | `TFLAG_TRANSLUCENT` tiles become **solid to the eye** while staying lit | you can read a room through frosted glass |
crouching behind cover | if the player is crouching, prone or running on four legs, every **adjacent** square with `coverage >= 30` becomes solid | hiding behind a low wall does nothing |
your own square | forced back to open air **last**, after both | standing on frosted glass blinds you to your own feet |

**The threshold is inclusive and the radius is one.** `coverage >= 30`, not `> 30`, and `points_in_radius( p, 1 )` means the eight neighbours - this is a check for a low wall
you are pressed against, not a sight line. A test pins twenty-nine as *not* cover.

**The reference's own TODO is the interesting part:** making an adjacent square solid changes vision for **everyone** on the level, monsters included, so a player crouching
behind a window also stops two monsters seeing each other. That is what a single shared cache means. The port keeps it and says so, because "fixing" it per-creature is a
different design, not a correction.

## One square's transparency, computed twice over the same facts (round 358)

`build_transparency_cache` (`lightmap.cpp:100-203`) asks a small rule about every square, and stores **two** answers because the game asks two different questions:

| Answer | Meaning | Question it serves |
|---|---|---|
`transparency` | light gets through, fields included | how much light reaches here |
`transparency_wo_fields` | the same with the fields removed | is there anything to see here at all |

The rule itself, in the reference's order:

1. **terrain and furniture must both be transparent** - a conjunction. An opaque table on a glass floor is opaque, and so is a transparent chair in a wall. Both answers are
   solid, and the check comes **before** the fields, so smoke cannot make a wall less opaque.
2. **weather** - a square under open sky is multiplied by the weather's `sight_penalty`. The reference's own FIXME admits that places inside vehicles are not marked as
   inside yet, so the rain is wrongly counted there; the port keeps the rule and the FIXME is quoted at the call site rather than quietly fixed.
3. **fields** - every field whose intensity level is not transparent multiplies the value by its `translucency`. It is a **multiplication, not the dimmest winning**: two
   layers of half-opaque smoke leave a quarter of the light.

The `wo_fields` answer is taken after step 2 and before step 3, which is exactly what makes it "the same square without its smoke". If it were taken before the weather, a
rainy square would be marked as more see-through than it is; if after the fields, the two answers would be the same and the cache would be pointless.

**And the reason the second answer exists is visibility, not light.** Smoke that made squares invisible would make a smoked room look like solid rock, so the "can I see
it" test uses the answer that ignores smoke while the light map uses the one that does not.

## The order of `generate_lightmap` is the behaviour (round 357)

`generate_lightmap( zlev )` (`lightmap.cpp:446-661`) builds a level's whole light map in seven steps, and every step depends on the one before it:

```mermaid
graph TD
    A[Zero the light map, the source cache and the buffer] --> B[Pour the sunlight in]
    B --> C[The player and NPCs light their own squares immediately]
    C --> D[Sweep every square: openings, items immediately, terrain, furniture and fields into the buffer]
    D --> E[Monsters and vehicles add their light]
    E --> F[Flush the buffer: every recorded square casts, x-then-y]
    F --> G[Apply field light overrides, which replace a square outright]
```

| Step | Why the order matters |
|---|---|
sunlight before sources | the pour *fills* the map, so it must come first; a source after it can only add |
items and characters immediately, terrain and furniture buffered | a held torch is one square's worth of light, an inferno is thousands - only the bulk is worth batching |
buffer flushed **after** every emitter is recorded | the direction gating can see a neighbour that was recorded later, which is the entire saving |
overrides **last** | `lm[x][y].fill( value )` replaces all four quarters, so anything after it would be undone |

**The buffer keeps its contents while it is being flushed.** The reference does not clear it: each `apply_light_source` in the flush reads the same buffer to decide which
directions to skip. A port that drained it first would compute the first source's cast as if it were alone.

**And the pour does not know about walls.** Sunlight is poured as a plain fill; blocking it is the sunlight pass's own per-square work, one level up. A level built in the
wrong order gets daylight through a wall - a test pins that the pour alone will happily light the far side, and that only the source pass respects the wall.

## Sunlight is not cast, it is poured (round 356)

`build_sunlight_cache` (`lightmap.cpp:306-437`) does not cast daylight, because daylight is parallel. It walks the z-levels **top to bottom** and each square takes its
light from the square directly above it, or failing that from one of the four cardinals - which is how light spills sideways under a ceiling. Two flags carry the state
between levels, and they are the whole optimisation:

| Regime | Condition | Cost |
|---|---|---|
nothing blocked the sun yet | `fully_outside` | one fill of the sky's value per level |
a hole appeared | neither flag | per-square quadrant work |
nothing gets through | `fully_inside` | one fill of the indoors value per level |

**Which level does the work is not obvious.** A partly roofed level is still *filled wholesale* - the reference fills first and inspects after - and it is the level
**below** it that does the per-square work, because that is the first level whose `above` is partly lit and which is neither all-outside nor all-inside. A test that
skipped straight to the middle regime computed nothing, which is how this was found.

| Rule | Value | What a port gets wrong |
|---|---|---|
indoors light | `LIGHT_AMBIENT_DIM * 0.8` **only** if the level is above ground **and** the sky is brighter than a bright lamp | using the dim value at night, or below ground, where there is nothing to be dim about |
own column first | the square above is checked before the cardinals and **breaks** the scan | smearing light sideways that should have come straight down |
a cardinal lights **two** quarters | north lights `NE` and `NW` | lighting all four, which removes the gradient beside a shaft |
the divisor | `clamp( above * OPEN_AIR / transparency, indoors, above )` | dropping the clamp, so thin air makes squares brighter than the sky |

## A light source in three steps, and an inversion the port reproduces on purpose (round 355)

`map::apply_light_source` (`lightmap.cpp:1217-1294`) is where every light in the world arrives - a lamp, a fire, lava, a held torch. Three steps, in this order:

| Step | Rule | What a port gets wrong |
|---|---|---|
the source's own square | lit to at least **1.0** in **all four quarters**; the raw luminance goes into `sm` | lighting only the quadrant facing the room, so a candle you stand beside is invisible at three corners |
quantisation | `<= 1` casts nothing; `<= 2` becomes **1.49** | casting 2 units, which decay below the lit threshold after one square of open air |
direction gating | a direction is skipped when a buffered source one square that way is **>= luminance** | casting all eight octants per fire, which is what a burning building does when the optimisation is missing |

### The inversion

`apply_light_source` reads the buffer at `y - 1` in its "north" test and then casts the two octants that cover `y + 1`; its "south" test reads `y + 1` and casts
towards `y - 1`. The x directions are not inverted. Either the axis is named for where the light comes from rather than where it goes, or the reference has a sign error
that has never shown up loudly, because the skip is a performance shortcut and the skipped cast is usually covered by the neighbour that caused it.

**The port reproduces it rather than correcting it**, and had to measure it to be sure: a scratch probe cast each transform on its own and listed the squares it
covered, which is how the pairing in the test was established rather than guessed. Two consequences are pinned: a bright source to the north stops the cast that goes
*south*, and a source on the top edge cannot cast downwards at all, because the buffer test that guards that direction is switched off at `y == 0`.

A correction here is a deliberate change with an oracle receipt behind it, not a tidy-up.

## The light cast itself, and a rounding rule that changes less than expected (round 354)

`castLightAll` (`lightmap.cpp:986-1012`) is one octant walk eight times, once per transform, so that between them the eight cover every square on the level. Each square
takes the **brightest** light reaching it, never the sum: two rays arriving at one square are two chances for it to be seen, not twice the brightness.

| Rule | Value | What a port gets wrong |
|---|---|---|
the source's own square | **not lit by the cast** - `apply_light_source` lights it before casting | lighting it here too makes every lamp twice as bright at its own square |
a square light passes through | all of it in the **default** quadrant | spreading it across four, which changes what a wall beside it looks like |
an opaque square | lit in **the quadrant the light came from** (`quadrant_from_x_y`) | lighting it evenly, which is why the mechanism exists at all |

**And `rl_dist` answers with an integer, so "circular distances" changes less than a player might expect.** The reference's `rl_dist` returns `int` and converts the
Euclidean `trig_dist` on the way out, so a one-square diagonal of `sqrt(2)` is **still one square**; only a long enough diagonal rounds up (3,4 is five against
Chebyshev's four). The first draft of this round's test asserted the imagined rule - that a diagonal would dim with circles on - and the compiled reference said
otherwise. `rl_dist` now returns `i32` exactly as the reference does, and `trig_dist` is kept beside it for the float value.

`quadrant_from_x_y` (`lightmap.cpp:874-879`) is worth writing down because it is not the obvious mapping: positive x with positive y is `NW`, positive x with
negative y is `SW`, and negative x gives `NE` and `SE`.

## Two distance metrics in one function, and it has always been that way (round 353)

`map::pl_sees( t, max_range )` (`lightmap.cpp:859-877`) is what the rest of the game asks instead of re-deriving visibility. It composes the range check, the apparent
light, the player's own vision threshold and the light source in the square - and adds nothing of its own except **the order**:

1. off the map is not seen;
2. past `max_range` is not seen, unless a camera is at the square (a negative `max_range` means no limit);
3. blocked is not seen, **however much light gets through**;
4. otherwise seen if the light reaches the player's threshold, **or** if a light source sits in the square.

**The two metrics are not the same metric.** The range check uses `square_dist` - Chebyshev, always. `apparent_light_at` uses `rl_dist`, which is Chebyshev only when the
"circular distances" option is off and Euclidean when it is on. So under that option a square three across and four up is *inside* a range of four by one metric and
*outside* it by the other, and the reference has always worked that way. The port takes the option as an argument rather than reading a global, so a caller cannot
forget it silently.

**And the threshold belongs to the player, not the square.** It is computed from the light where the player is standing, so a lit room and a dark one settle the answer
between them; the square's own light is compared against it, not used to make it reasonable.

**Obstruction is the one thing light cannot overcome here.** `apparent_light_at` has a `BRIGHT_ONLY` answer for light that gets through a blocked sight line, but `pl_sees`
says no - which is why the game draws a lit wall on the far side of a corner and still refuses to let you see what is behind it.

## How dark a square has to be before a character stops seeing it (round 352)

`Character::get_vision_threshold( light_level )` (`character.cpp:2724-2763`) is the number the last branch of `apparent_light_at` compares against: below it a square is
`BLANK`, at or above it the square is `LOW`. Two ideas meet in it.

* **Range** sets a starting threshold through the same attenuation light obeys (`threshold_for_range` = `LIGHT_AMBIENT_MINIMAL / e^( range * 2.3/72 ) - epsilon`), so
  "the dimmest thing I can see at r squares" is a coherent statement.
* **Dimming** makes the threshold *worse* as the light around the character gets brighter, from 1.0 at the minimal ambient light to 2.0 at full daylight. A lit room
  does not help you see into the dark beyond it.

| Detail | Reference | What a port gets wrong |
|---|---|---|
the cap | `min( LIGHT_AMBIENT_LOW, ... )` | forgetting it, so a character in daylight can see into pitch dark - and the cap sits **below** `LIGHT_AMBIENT_MINIMAL`, an oddity kept rather than tidied |
the range floor | `max( 1.0, range * limb_score )` | letting a character with no perception see nothing at all, including their own square |
night vision | a **branch**, not a bonus: the +10 bionic replaces the enchantment multiplier rather than stacking | adding both and giving 13 squares where the reference gives 10 |

**The cap is the value most characters actually use.** With ordinary eyes the product passes three-and-a-half almost at once, so the threshold is a constant in practice,
and the dimming only becomes observable with night vision widening the range. Both facts are pinned: the cap by two lights that give the same answer, and the dimming by a
bionic-eyed character at two lights that give different ones.

## The span list, and a duplication this round had to undo first (round 351)

The light cast does not walk rays. It sweeps a widening wedge and carries a list of **spans**, each one a frustum of that wedge that has met the same opacity so far. When
a square's opacity differs from the span's, `split_span` (`shadowcasting.cpp:69-159`) cuts the span into up to four pieces:

```text
+-------+ <- end major
|   D   |  rows not yet processed, old opacity, and they must not re-check this row
+---+---+ <- leading edge major
| B | C |  B: this row's processed part, old opacity
+---+---+  C: the rest of this row, new opacity
|   A   |  rows above, old opacity
+-------+ <- start major
```

| Piece | Exists when | Why |
|---|---|---|
`A`, `B` | the square just left was **transparent** | an opaque square's span stops at it; nothing behind it is lit through it |
`D` | the leading edge has not reached the span's end | the rows beyond the change keep the old opacity |

**And `slope` compares by cross-multiplication, not by its fields.** `a/b < c/d` is `a*d < c*b` (`shadowcasting.cpp:34-38`), which makes one in two equal to two in
four - so the port implements `PartialEq`, `Eq` and `Ord` by cross-multiplication as well. A port that derived `PartialEq` from the fields would call those two angles
different while `Ord` called them equal, and whatever container sorted them would misbehave in a way that only shows up at the far end of a sweep.

**The round also had to undo its own duplication.** Rounds 349 and 350 added `Quadrant`, `FourQuadrants`, `sight_calc`, `sight_check`, `accumulate_transparency` and
`update_light` to `light.rs` without noticing that `shadowcasting.rs` already had all six from an earlier pass. The duplicate set is gone; `shadowcasting.rs` owns the
sweep, `light.rs` owns what the player makes of a lit square, and `visible_light` now takes the one `FourQuadrants` that exists. Tests were repointed, not duplicated.

**Two reference details worth keeping:** `slope` stores its fields as `int_least8_t` and the reference notes that a radius of 120 "definitely will not fit" - the port
stores `i32`, exact for every radius the game uses, and treats the truncation as a limit of the reference's storage rather than a rule. And `split_span` answers with
the index of the span that carries on, because the pieces are inserted **before** it and every caller would otherwise use a stale index.

## The whole physics of the light cast is four small functions (round 350)

Everything in `shadowcasting.cpp` is deciding *which squares a ray passes through*; the arithmetic that decides *how much light is left* is four functions in
`shadowcasting.h:96-116`, and all four are now ported and pinned.

| Function | Rule | What a port gets wrong if it improvises |
|---|---|---|
`sight_calc` | `numerator / e^( transparency * distance )` - Beer-Lambert with the length factored out | making it return zero for an opaque square, which is `sight_check`'s job, loses a square of light |
`sight_check` | `transparency > 0`, strictly | `>= 0` lets a ray into a wall and the cast never converges |
`accumulate_transparency` | `(( d - 1 ) * cumulative + current ) / d` - the exact running mean | carrying the previous value into the first step, which lights the whole map from the origin's own square |
`update_light` | a maximum | overwriting instead of taking the brighter value, so a lamp loses to the first dim ray that reaches it |

**`TRANSPARENCY_OPEN_AIR` is `2.3 / 72` for a reason.** `e^-2.3` is about a tenth, and a bright lamp (`10.0`) seen across `MAX_VIEW_DISTANCE` of open air comes out at
`1.0026` - exactly at the lit threshold. That is not a coincidence to be preserved with a magic constant: it is how the constant was chosen, and the value is now a test.

**And the running mean is order-independent.** At each distance the accumulator is the arithmetic mean of every transparency the ray has met so far, so the same squares
met in another order give the same light. That is a property of the cast, not an accident of the loop, and a test walks a ray forwards and backwards to prove it.

## A wall is not lit uniformly, and its neighbours decide (round 349)

The light map stores **four values per square**, one per quadrant, because light does not arrive at a wall from one direction: it arrives from wherever the lamps are.
`apparent_light_helper` (`lightmap.cpp:725-799`) is the rule that reads them, and it has two cases:

* a square **light passes through** takes light from every direction equally, so its answer is `visibility x brightest quadrant`;
* a square **light cannot pass through** is only lit on the sides the player can see it from.

For the second case the helper asks each of the eight neighbours which quadrants it sits in:

| Neighbour | Quadrants it lights | Note |
|---|---|---|
cardinal (south, north, east, west) | **two** - south lights `SE` and `SW` | a lamp beside a wall lights the half it is on |
diagonal (SE, NE, SW, NW) | **one**, twice | the corner shared by two quadrants |

A neighbour only counts if it is inside the lightmap, light can pass through it, and somebody can see it - its distance against the player's unimpaired range, and its
seen and camera values. Then each counted neighbour raises its quadrants to the player's visibility of the wall, and the answer is the brightest product with the wall's
own four values.

**And the quadrant order is not clockwise.** `enum class quadrant { NE, SE, SW, NW }` (`shadowcasting.h:24-30`) is north-east, then east's two, then west's two. A port
that ordered them clockwise from north would be a rotation of the truth, and every product would pair the wrong sides - a lit wall would read as dark and a dark one as
lit. The order is pinned by a test rather than left to the reader.

## The order of the branches is the behaviour (round 348)

`map::apparent_light_at` (`lightmap.cpp:801-857`) turns the light map into a `lit_level`. The quadrant arithmetic that works out *how much* light reaches a square is
`apparent_light_helper`'s job; this is the decision tree above it, and it is ported with those answers as inputs.

```mermaid
graph TD
    A[clairvoyance reaches the square] -->|yes| Z[BRIGHT]
    A -->|no| B{beyond unimpaired range and no camera}
    B -->|yes| C{is it a light source and nothing blocks it?}
    C -->|yes| Y[BRIGHT_ONLY]
    C -->|no| X[BLANK]
    B -->|no| D{obstructed?}
    D -->|yes| E{light beats AMBIENT_LIT and the sky?}
    E -->|yes| Y
    E -->|no| X
    D -->|no| F{light above a source, or a source here?}
    F -->|yes| Z
    F -->|no| G{above AMBIENT_LIT?}
    G -->|yes| W[LIT]
    G -->|no| H{at or above the vision threshold?}
    H -->|yes| V[LOW]
    H -->|no| X
```

Three things the tree gets right that a re-derivation would not:

- **clairvoyance beats an opaque wall**, and beats it before distance is considered;
- **obstruction is not blindness.** Enough light through a blocked sight line makes a square `BRIGHT_ONLY` - but only if that light beats the ambient light outside, so
  the same square under a brighter sky is blank;
- **the two comparisons differ**: `> LIGHT_AMBIENT_LIT` for lit and `>= vision_threshold` for low. Exactly ten is *not* lit; exactly the threshold still counts. Both
  boundaries are pinned.

## Seven light levels, not four (round 347)

`lit_level` (`lightmap.h:55-63`) has seven values and the port had four. The three it had were the brightnesses; the ones it was missing - `BRIGHT_ONLY`, `BLANK`,
`MEMORIZED` - were excluded on the stated grounds that they are "not light levels but states of the map overlay". That was wrong, and `map::apparent_light_at`
(`lightmap.cpp:801-857`) is the proof: it **returns** `BRIGHT_ONLY` for a square whose light gets through without its detail, and `BLANK` for one that cannot be seen
at all. They are answers the light code produces, so the visibility rule has to read them.

`map::get_visibility` (`map.cpp:6808-6849`) is the join, and it is a five-arm switch over seven levels:

| Light level | Player is fine | Boomed |
|---|---|---|
`DARK` | `DARK` | `BOOMER_DARK` |
`BRIGHT_ONLY` | `LIT` - bright, and nothing more can be made out | `BOOMER` - a smear of colour |
`LOW`, `LIT`, `BRIGHT` | `CLEAR` - the level itself is not the answer, only that the square can be seen | `CLEAR` |
`BLANK`, `MEMORIZED` | `HIDDEN` | `BOOMER_DARK` |

Two things fall out of the table that are worth stating: the three middles are indistinguishable to this function, and a boomer's bile changes **every** answer it
touches - including turning a memorised square into a dark one.

## What marks a level stale, and the early-out that saves a light map (round 346)

`level_cache` (`src/level_cache.h:20-98`) holds a level's derived values; four functions mark it stale (`map.cpp:295-345`), and porting the marking is what makes the
rebuilds correct before any of the derived arrays exist. Three kinds of mark, and they are not interchangeable:

| Mark | Granularity | Why |
|---|---|---|
transparency | one bit per **submap** | transparency is computed a submap at a time, so a changed tile spoils only its own |
seen (the light map) | one flag per **level** | visibility is a property of the whole map, not of a square |
floor, outside | one flag each per level | both are read off terrain and furniture and both are cheap to recompute for a level |

**Submap bits are x-first.** The bit is `smp.x() * MAPSIZE + smp.y()` (`map.cpp:306`), the third place this project has found the same ordering - after the SoA tile
order and `get_nonant`. A port that indexes y-first gets a transposed grid of dirty bits, which is invisible until a rebuild is skipped.

**And one mark is conditional on visibility, which is the clever part.** `set_seen_cache_dirty( p )` (`map.cpp:313-325`) returns early twice: already dirty is
nothing to do, and a square whose **seen and camera values are both zero** cannot have been shadowing anything, so a change there cannot change what is seen. That
is what keeps a fire crossing an unexplored room from recomputing the whole light map.

**A non-field change also invalidates reachability** (`map.cpp:307-309`). Smoke moving does not change what a creature can walk to; a wall appearing does. The port
keeps that distinction because the reachability cache will be built on it.

## The bubble is a square, and its walk order is the digest's order (round 345)

The state dump fingerprints the world with `points_in_radius( center, 60 )` (`harness.cpp:1141`), and porting that window is what makes the fingerprint reproducible
later. Three facts, all of which a port can get wrong quietly:

| Fact | Reference | Consequence |
|---|---|---|
the window is a **square**, not a disc | `map.cpp:10237-10248` returns a range of corners | the name has been a misnomer for years; 121 by 121 is 14,641 squares, not a circle's worth |
a window near the map's edge is **smaller** | clamped to `max(0, centre - radius)` and `min(SEEX * MAPSIZE - 1, centre + radius)` | a player at (0,0) sees 61 by 61, not 121 by 121 with half of it outside |
the walk is **x fastest** | `map_iterator.h:44-63` increments x, wraps, then y | the digest is over a sequence, so any other order is a different number for the same world |

**And the items-and-fields half of `maptile_soa` is deliberately not ported as storage.** The reference keeps a `colony<item>` and a `field` per square inside the
submap; the port's architecture makes both **entities** with a position, queried through the ECS. The one thing that was storage in the reference and would have been
lost - `field_count`, the counter that lets `process_fields` skip a submap with no fields - has no counterpart because the equivalent is an ECS query. That is a
decision, not an omission, and it is recorded here so the next reader does not go looking for the array.

## Merging submaps, where two layers break the rules (round 344)

`merge_submaps` (`submap.cpp:391-436`) merges one submap onto another with `copy_from_is_overlay` deciding who wins. Three layers follow the obvious rule and two do
not:

| Layer | When the source's value is taken |
|---|---|
terrain, furniture, traps | the source has one (**not** null) **and** either this is an overlay or the target has none |
radiation | the source is **above zero** and either this is an overlay or the target has none |
light | **always**, and **added** - no null check, no overlay check |
items, fields | always (they arrive with their own layers) |

**The light sum wraps.** `this->m->lum[x][y] += copy_from->m->lum[x][y]` is a `uint8_t` addition: promoted to `int`, added, truncated on the way back. Two squares of 200
and 100 light merge to 44. Item counting saturates at 255 one item at a time, so the merge is the one place the value can leave that range, and the reference wraps
rather than clamping. Reproduced, with the reason written at the call site.

**And uniformity is a save rule, not just a memory trick.** `mapbuffer.cpp:278-284` skips a whole quad of submaps when every one of them is uniform - "regenerated
faster than it would be re-read" - so `needs_saving()` is a question the save path asks. A uniform submap is not a compressed empty one; it is one that is never
written.

## The other four layers, and the asymmetries between them (round 343)

A submap now carries terrain, furniture, luminance, traps and radiation, each a page that costs nothing until it is written. The reference's `maptile_soa` also
holds items and fields; those arrive with the systems that store them.

Three asymmetries in the reference are preserved rather than tidied, and each is now a test:

| Layer | Filling the whole submap | Why |
|---|---|---|
terrain | `set_all_ter( terr, uniform_ok )` - **can** stay uniform | it is the layer a whole submap usually shares (`submap.h:157-165`) |
furniture | `set_all_furn` - always materialises | no `uniform_ok` flag (`:119-123`) |
traps | `set_all_traps` - always materialises | no `uniform_ok` flag (`:107-111`) |

**Light has three branches and one of them is the reference's own arithmetic.** `update_lum_rem` (`submap.cpp:364-388`): a non-emissive item changes nothing (but
it materialises the array first); a count that is neither zero nor saturated decrements; a **saturated** count is recounted from the emissive items left on the
square and written as `count - 1`. At `count == 0` that subtraction underflows and the cast takes the low byte, giving 255. It looks like a bug in the reference
and may be one; the port reproduces it and says so at the call site, because silently "fixing" it would be a divergence nobody could see.

**`is_uniform` had to change meaning.** In the reference it is `!m` - one array shared by all seven layers, so a write to any layer materialises all of them. The
port keeps a page per layer, so `is_uniform()` now asks every layer. A submap is uniform exactly when nothing has ever been written to it.

## Two tile rules the port had backwards (round 342)

Porting submap storage meant reading `submap.h` line by line, and the port's `TilePage` disagreed with it twice. Both disagreements were invisible because every
read and every write went through the same belief:

| Rule | Reference | Port (before) |
|---|---|---|
when does a uniform page allocate? | every setter calls `ensure_nonuniform()` **first**, so writing the value it already holds still allocates (`submap.h:152-158`) | skipped the allocation when the value matched - a nicer data structure and a different one |
what order is the dense array in? | `mdarray<T, point_sm_ms>` is a `std::array` of columns indexed by **x** (`mdarray.h:45,64`), so `x * SIDE + y` | `y * SIDE + x` - the transpose |

The order one is the interesting failure. A transposed SoA reads back correctly through the same transposed index, so nothing complains until something walks the
page in its own order: a save file, a digest, a replay. The test that caught it sets `(1,0)` and looks for the value twelve slots in.

**The old test was the lie.** `tiles.rs` had a test named `cells_are_stored_row_by_row` asserting the transpose, with a comment citing the reference. The comment
was wrong: it said `ter[y][x]` while `submap.h:150` says `m->ter[p.x()][p.y()]`. It is now `cells_are_stored_column_by_column`, and its history is written into it.

## P2 starts with the sizes of the world (round 341)

`--dump-geometry` is the seventh harness dump, and it exists because the map's geometry is the one thing every later system divides by. It states, from the
reference's own constants: `SEEX`/`SEEY` 12, `MAPSIZE` 11, the map 132 by 132, the vertical stack 21 levels from ten below to ten above, and a view distance of 60.

| Thing | Squares | Which number |
|---|---|---|
a submap | 144 | `SEEX * SEEY` |
the map | 17,424 | `MAPSIZE_X * MAPSIZE_Y`, 132 by 132 |
the view | 14,641 | `(2 * 60 + 1)^2`, 121 by 121 - what the state dump fingerprints |

**The last two are not the same number, and that is the point of the dump.** The map is what is loaded; the view is what was looked at. A port that conflates them
is eleven squares wrong at every edge and would not notice until something read the edge of the bubble.

Two layout facts are **transcribed rather than oracle-backed**, and are marked as such in the code: a submap's flat index is `x * SEEY + y` because `maptile_soa`
stores `ter[x][y]` (`submap.h:86-92`), which is the transpose of the obvious choice; and `map::get_nonant` puts the **level** in the outer dimension, so one column
of the world is contiguous. Neither is observable from outside the reference without a dump of internal pointers, so both stay labelled transcription until a
system that reads them can be gated end to end.

## Duplicates: same source is an error, another source is an override (round 340)

The registry machinery had the duplicate rule **backwards**, and nothing had asked it to prove otherwise. `generic_factory::insert` (`generic_factory.h:390-397`)
calls `mod_tracker::check_duplicate_entries`, and that function (`mod_tracker.h:75-82`) has exactly two outcomes:

| Second definition comes from | Reference does | What the port used to do |
|---|---|---|
the **same** mod | **throws**: `"<id> (<type>) has two definitions from the same source (<src>)!"` | replaced it silently |
a **different** mod | replaces the earlier one, in place | replaced it silently |

So the port was right about overrides and wrong about mistakes: a data file that defines the same id twice would have loaded happily, and the reference refuses to
start. `Factory::insert` now returns `Result<InsertOutcome, DuplicateDefinition>` with the reference's sentence in its `Display`.

The corpus test walks every file in load order and inserts every `trap` into a real factory: **zero** same-source duplicates, which is what "the reference loads this
data at all" means. That is a claim about the content, and it would fail loudly if the port's rule were stricter than the reference's.

## Mod load order is parity now, not a reading of the code (round 339)

The oracle learned a sixth dump: `--dump-mods` writes, for every one of the 43 mods, the dependencies it declares **and** the list the reference's own
`dependency_tree` resolves for it. That second list is the point - it is the reference's answer, not a re-derivation - so the port is compared against it rather
than against a reading of `dependency_tree.cpp`.

The rule it reproduces (`dependency_node::get_dependencies_as_nodes`, `:157-197`) is a stack walk: push a mod's dependencies in declared order, pop from the end,
skip what has been seen, push each one's dependencies, then walk the record **backwards** and keep each mod the first time it appears.

**The backwards pass is not a topological sort, and that is worth saying out loud.** On a diamond - two mods sharing one base - the reference's answer puts the
first mod before the base it depends on. That may well be a bug in the reference; the port's job is to answer what the reference answers, and the dump comparison
over all 43 mods is what proves it does.

Also pinned: `dev:default`'s resolved order is `no_npc_food, dda, personal_portal_storms` while the list it declares is `dda, no_npc_food,
personal_portal_storms` - the default list is replayed in its own order, and each mod's dependencies come from the tree.

## The last four that were not empty (round 338)

Four registries were still ungated after the rename table, and each needed a different kind of exception rather than a new rule:

| Registry | Ids | What was missing |
|---|---|---|
`MONSTER_FACTION` | 133 | the factory is `generic_factory<monfaction>( "MONSTER_FACTION", "name" )` (`monfaction.cpp:13`) - its ids are under **`name`**, and one of them is the empty string, the sentinel |
`achievement` | 226 | 206 are `achievement` and 20 are written as a **`conduct`**, which the reference registers in the same factory |
`martial art buff` | 113 | a **slot**, not an entry: every `*_buffs` member (`static_buffs`, `onblock_buffs`, `oncrit_buffs` and their eight siblings) holds objects that register under their own `id` |
`species` | 32 | 31 written as `SPECIES`, plus `spec_null` - the factory's null prototype, made in code, the same wrinkle as `t_null` and `f_null` |

That brings the coverage to **88 of the 90**, and the two that remain are `city` and `magic_type`, which hold **no ids at all**. There is nothing to assert about them and
pretending otherwise would be worse than saying so.

**The id-set rule now has four documented exceptions**, each of which cost a mismatch before it was written down: a renamed type, a nested object that is not an
entry, a null prototype made by code, and an id that is not called `id`.

## Eighty-three of ninety, and a table nobody wrote down (round 337)

Forty-one of the reference's ninety registries are not named after their JSON `type`, and nothing documents which is which. It was found the only honest way: for
every registry, ask which JSON type's id set **equals** the reference's, then gate the pair with the digest so a wrong pair cannot pass.

| Registry | Written as | Ids |
|---|---|---|
`widgets` | `widget` | 1,236 |
`json_flags` | `json_flag` | 731 |
`construction groups` | `construction_group` | 422 |
`mission_type` | `mission_definition` | 306 |
`martial art technique` | `technique` | 203 |
`field types` | `field_type` | 145 |
`zone_type` | `LOOT_ZONE` | 64 |
`item_category` | `ITEM_CATEGORY` | 36 |
`trait` | `mutation` | 959 |
`spell` | `SPELL` | 246 |
| ...34 more | | |

Two registries had **two** candidate types that both matched exactly, which the digest alone cannot resolve; the reading one won - `damage type` ← `damage_type`,
`damage info order` ← `damage_info_order` - and both are gated, so a wrong guess is a red test.

| | |
|---|---|
registries gated by the rule | **75** |
registries gated through a real loader | 8 more |
**total** | **83 of 90** |
the seven left | the ones whose ids no rule produces yet: `overmap special`'s siblings, `ascii_art`-style inline lists, and the empty registry `city` |

**The rule is now stated in one sentence and defended by three counterexamples** (a renamed type, a nested object that is not an entry, a null prototype made by
code), each of which cost a real mismatch before it was written down.

## An entry is a top-level element, and two registries say so loudly (round 335b)

Two more registries finish with a rule, and both of them broke a rule that had been working:

**`spell` is 246, of which the rule found 13.** The other 233 are written as `"type": "SPELL"` - uppercase - which is the convention modern data uses, and a
handful of older entries still spell it lowercase. A registry that answers to two spellings is the same wrinkle as `mutation` registering as `trait`.

**`enchantment` is 353, of which the rule finds 42.** The rest are inline, and the naming rule is exact: `INLINE_ENCH_<owner>_<index>`, where the index counts
**every** element of the `enchantments` list (strings included, since a string is a reference rather than a new enchantment). The trap is the owner: a mutation,
an effect and a vehicle part are named by their **id**, but a **bionic** is named by the words a player reads - `INLINE_ENCH_Alarm System_0`, not
`INLINE_ENCH_bio_alarm_0`. That one cost a digest mismatch, and it is exactly the sort of thing a count cannot see.

**And the rule itself was too loose.** `walk` had been collecting `id`s from any object anywhere in a file. The count still came out right for thirty-six
registries, which is why it survived - until spells: a monster's special attack is written `"type": "spell"` with an `id`, so the loose rule invented three
spells the reference does not have (`star_vampire_invisibility` and two siblings). The rule now says what the reference means: an **entry** is a top-level element
of a file, and the kinds that register inline each have a loader that says so. Two registries lost an id under the strict rule and both are the factory's own null
prototypes - `t_null` and `f_null`, created by code and written by no file - so `terrain` and `furniture` stay gated through their loaders.

## The 239 parts nobody wrote down (round 335)

`vehicle_part` is the first registry in the corpus that a **rule** finishes and no file can. Data files write 462 of them; the reference's registry holds 701.
The missing 239 are invented at finalisation (`veh_type.cpp:565-640`): for every item that could be bolted to a mount, it makes one part named
`turret_<item id>`, based on the generic turret, with that gun as its base item.

The filter is four questions (`:473-491`): does it have gun data, is it **not** a gun mod, and does it carry none of six flags - `BIONIC_WEAPON`, `NO_TURRET`,
`NO_UNWIELD`, `PSEUDO`, `RELOAD_AND_SHOOT`, `STR_DRAW`. The port runs the same four against the same items and gets the same 239, and the digest of
`written + invented` now equals the reference's.

**The oracle had to move first.** The item dump carried no gun-ness at all, so no port could have reproduced this registry; the dump now states `is_gun` and
`is_gunmod` per prototype. That is the working order the plan asks for - the instrument gets the fact before the port needs the fact.

A smaller honesty note: the first gun in load order is `barb_launcher`, whose flags include `PSEUDO` and `RELOAD_AND_SHOOT`, so it earns no turret. The test picks
its subject by *what the rule does with it* rather than by the first gun it finds, which is how that got noticed instead of being papered over.

## One registration rule, thirty-six registries (round 334)

Almost every JSON type is a `generic_factory` of something, and they all follow the same rule: an entry registers under its `id`, and an entry carrying
`abstract` registers nothing - an abstract is a base to copy from, not a name the game can ask for. Rather than port ninety loaders to prove ninety id sets, the
rule itself was ported and then **tested against the reference's own id sets**:

| | |
|---|---|
registries the rule reproduces exactly | **36** |
ids they hold | ~13,600 |
registries now gated in total (with the loader-based ones) | **46** |
missing ids anywhere in the 36 | **0** |

Two wrinkles, both found by the probe rather than assumed:

- **a registry's name is not always its JSON type.** `mutation` registers as `trait` - 959 of them, and a rule that trusted the type name missed every single
  one. That is the kind of error a count would have hidden and a digest caught.
- **a factory's null prototype is made by code, not by data.** `t_null` is the one terrain id in the reference's registry that no file holds, which is why
  `terrain` is 1108 by the rule and 1109 in the reference.

The registries the rule does **not** get right are exactly the ones with inline members - `effect_on_condition` (221 inline), `vehicle_part` (239), `enchantment`
(311), `spell` (233), `achievement` (20) - which is the same shape of problem as the inline effects the objective names, and the same shape of answer.

**What this claims and what it does not.** `digest_ids` says the **id set** is right; only `digest` says a **loader** is right, and the twelve registries where
both run must agree, which they do. A loader that nothing gates is a loader that drifts; an id set that nothing gates is a worse version of the same thing.

## Three registries that were right and unevidenced (round 333)

`effect_on_condition` (913), `harvest_list` (366) and `npc_class` (155) were all loaded correctly and none of them was in the digest gate. A loader a gate does
not cover is a loader that drifts, and these had drifted into the corpus unnoticed:

| Registry | Ids | Reference | Missing | Invented |
|---|---|---|---|---|
`effect_on_condition` | 913 | 913 | 0 | 0 |
`harvest_list` | 366 | 366 | 0 | 0 |
`npc_class` | 155 | 155 | 0 | 0 |

The effect one is the interesting case and the reason the objective names it: 692 are defined by an entry of their own and **221 are inline**, sitting in
someone else's slot (`activated_eocs`, `true_eocs`, `run_eocs` and the rest). The loader walks those slots and nothing deeper, because a slot's value *is* the
effect list - and collecting `id` from arbitrary nesting had once invented seven effects out of sound-effect objects with an `id` of their own.

**The gate is now 18 registries.** Two candidates were probed and dropped rather than forced: `monster_group` is not one of the reference's ninety, so there is
nothing to compare it to.

## Items burn now, and the reals they roll are recorded - on request (round 332)

The item block is wired: before a fire eats its ground it eats what is lying on it (`map_field.cpp:976-1048`), so `time_added` and `smoke` are **assigned** by
the items and then added to by the ground. Items on the ground are entities with their own `burnt`, because a fire eats a plank a forty-eighth at a time and
the arithmetic only works if each turn starts where the last one left it.

**The trace learned a third kind of die: `real`.** `rng_float` is the only door to a non-integer draw, and every probabilistic rounding goes through it -
`roll_remainder`, `x_in_y`, `rng_normal`. It was not recorded at all, so a session that made one was silently unreplayable: the port would take the next whole
number the reference had drawn for something else, and nothing said a die was missing.

It is recorded **only with `CDDA_RNG_TRACE_REALS` set**, and that is a deliberate refusal rather than laziness. A trace with reals in it is strictly better -
the replay can then reproduce `roll_remainder` exactly - but it is also a trace no port can finish until **every** float draw is modelled in the same order.
Turning it on today made all three fixtures fail at the first `real` in the spread loop, which is real work not yet done (bash, rubble, item spawning), not a
bug. So the machinery is ported and tested (`recording_real`, four cases), the oracle can emit the kind, and the default remains the strict-integer trace that
the corpus can actually spend.

**The echo is honest both ways:** with reals off, `fire-plank` replays to its last die and its spread still depends on the item branch (checked by mutation
last round); with reals on, it stops at the first draw the port cannot yet make. Both statements are in `corpus/fields/README.md`.

## The rule behind the plank's one turn of fuel (round 331)

The gap declared last round now has its rule ported, though not yet its wiring: `simulate_burn` (`item.cpp:12543-12596`), the arithmetic that decides what a
burning item gives the fire eating it.

| Facet | The rule |
|---|---|
a material with a `volume_per_turn` | contributes **in proportion to the item's own volume**, so a plank gives `200 / 4130` of a turn's fuel and a rag gives its own fraction |
a material with none | burns flat, which is how a level-three fire keeps eating an item whatever its size |
a **contained** fire | burns flat *and* at level three (`effective_intensity = contained ? 3 : fire_intensity`) - a brazier is fed on purpose |
an immune material | contributes nothing at all, whatever else the item is made of |
several materials | each is scaled by its portion count and the sum is divided by the **portion count**, not by the number of materials |

Six tests hold it, all transcribed from the reference's own source and wood's own numbers from `corpus/prototypes/materials.json`.

**The oracle learned to say what is lying on a square.** `"items"` and `"items_burn"` said how many and whether any would burn; `"items_at"` now names them
(`["2x4"]`, and a locker's thirteen-item contents in full), because the fire eats the items themselves and two squares that both say `items_burn: 1` can feed a
fire by very different amounts. The three fixtures were regenerated and their dice streams are **identical** to the byte, which is what a dump-only change
should look like.

**Not yet wired:** the port does not run `simulate_burn` inside the fire processor, so the plank fixture still stops at its spread. The pieces are now in place -
the rule, the item ids, and the volume and materials in `corpus/prototypes/items.json` - and the wiring is the next slice.

## Three rules the replay found that the unit tests could not (round 330)

The plank fixture - fire on `t_floor`, all eight neighbours `t_concrete` with `f_null`, one plank - spread onto the plank's square at turn six and then held up
the replay at three different draws. Each one was a real rule:

| Draw | What the port believed | What the reference does |
|---|---|---|
the hot-air gate's range | counted the neighbouring fire and asked `rng( 0, 3 )` | asks `rng( 0, 4 )`, because the flashpoint scan only runs while the fire has **fuel banked** (`age < 0`) and a fire with nothing banked counts no neighbours at all |
`int 0 100` then `int 3 35` | smoke was a property of the square, and every square answered zero | smoke is a **local of this turn's fuel block**: every feeding branch adds `2 + windpower / 5`, and the gate reads that |
the newborn's second turn | processed the newborn the turn after it was born and aged it | left the age at 120 for a turn, then 121 - the newborn was **eating the plank** (`time_added = 1_turns` from `item::burn`), which is a rule the port does not have |

The first two are ported. The third is declared: `item::burn` and `fire_data` are not modelled, so the fixture stops at the spread and the sixty-turn run that
found it is recorded in `corpus/fields/README.md` with the numbers (120, 119, 120) for whoever ports it.

**Two corrections of my own work.** The mutation check I ran last round compared against a string that `cargo fmt` had rewritten, so it silently did nothing
and I reported "no fixture exercises the item branch" on the strength of a no-op. Re-run properly, the bench fixture still did not fail - it spreads onto
`FLAMMABLE_ASH` ground and never needed the branch - while the new plank fixture does: with the branch disabled it fails on turn six with
`[(70, 59, 2, 84)]` against `[(69, 59, 1, 120), (70, 59, 2, 144)]`. The bench fixture has been deleted rather than kept as a test that cannot fail.

**Refactor:** `FireMap::smoke_produced` is gone. It was a query that only ever had one honest answer - zero - and now that the number comes from the fuel block
no map has to be asked. `fire_discharge` also stopped taking eight arguments: the four numbers a turn of burning produces travel together as `Discharging`.

## The item branch, and a fixture that would not cooperate (round 329)

The spread rule's last disjunct is `( one_in( 5 ) && dst.get_item_count() > 0 && flammable_items_at( ... ) )`. The port had been **drawing** that die and
throwing the answer away, because nothing in the world it modelled could be lying on the ground. It now decides: `Neighbour` carries `items`, `MapFacts`
carries a per-square answer, and the dump carries `"items_burn"` for every grid square.

Three unit tests pin the branch, including the one that matters: a plank on ground that refuses catches on the item die **alone**, and the die is spent even
when the square is empty.

**What is not proven:** no fixture exercises it end to end. The oracle grew `--place-items`, `--place-terrain` and `--place-furniture` this round precisely to
build one - fire on `t_concrete`, every neighbour `t_concrete`, one plank on one square - and the reference then refused to spread there at all in twenty
turns of trying: the chance is 25%, the skip is one in four, and the item die is one in five, so an isolated square is rolled for about once every forty turns.
That is a fixture to build with a longer session, not a rule to doubt; the unit tests hold the rule until then, and this paragraph is the receipt.

## A fixture is a test now, not a test per fixture (round 328)

The replay is machinery now: `reference_session` loads a fixture (terrain grid, furniture, turns, recorded dice), builds the port's world from it and plays
it; `corpus_sessions` runs **every** session `corpus/fields/replayable.txt` names. A new rule needs a new fixture and one line in that list, not a new test.

Getting the second fixture to replay turned up four rules, three of them wrong in the port:

| Finding | How it showed |
|---|---|
`can_burn` gates the whole fuel block | a fire on a square that cannot burn drew a fuel die the reference never rolled |
`FLAMMABLE` is asked **before** `FLAMMABLE_HARD` and `FLAMMABLE_ASH` | the precedence decides which rule runs, and a bench on a tiled floor is *hard* ground because the harder flag is asked first |
a square's answers are its terrain's **and** its furniture's | `ter_furn_has_flag` reads both; without it a bench square read as bare floor |
`FLAMMABLE_ASH` is not `FLAMMABLE` | folding them together cost one die per bench - the port asked twice where the reference asked once |

**Proving the machinery can fail:** deleting the spread's outcome makes `corpus_sessions` fail with both sides printed, so the harness is not vacuous. The
ring's order is *not* caught by the fixture - one spread does not distinguish the order - so it is pinned by a transcript test against `point.h:450`
instead. That is the honest split: a fixture for what a scene can show, a transcript for a constant.

**Guard added:** `fire-floor-offset` now stands where no locker is adjacent, because a spread onto a locker's square succeeds through the reference's
*items* branch, and the port has no items. That gap was found by the replay refusing a die, not by reading the code.

## The hundred draws were a person, not a fire (round 327)

Last round's replay carved out **104 draws** per hot-air creation, on the theory that they were what `add_field` costs. They were not. Marking each statement
of `map::add_field` and re-recording showed the draws landing between the fifth square's insertion and `on_field_modified` - that is, inside the `hit_player`
branch, which `add_field`'s default argument turns **on** and `create_hot_air` does not turn off:

| What I thought | What it was |
|---|---|
the cost of adding a hot-air field | `creature_in_field( player )` - the player standing in the square the hot air lands on |
field bookkeeping the port lacks | creature bookkeeping the port lacks: body parts, saving rolls, a hundred dice |

The fixture had put the fire **on the player's own square** (`--place-fire 0 0 2`), so the fifth hot-air square always landed under them. The fix is not a
port of the creature layer, it is the right scene: `fire-floor-offset.json` places the fire one square east, and the whole session now replays - five turns,
six processors, **nothing carved out**, `recording_remaining()` asserted at zero.

Both fixtures stay: the first is the ready-made case for the creature layer, the second is the one that proves the field rules.

## Five turns, and the two rules that only a longer replay could show (round 326)

The replay now covers the **whole session**: five turns, six fire processors, one recording, one world carried from turn to turn. It passes, and it caught
two things the single-turn test could not:

| Finding | How it showed | Why it mattered |
|---|---|---|
the neighbour ring is not clockwise | the dice matched but the fire lit `69,61` where the reference lit `68,61` | the reference's `eight_horizontal_neighbors` is NW, N, NE, **W, E**, SW, S, SE - west before east. One step round the ring, a whole square wrong |
newborn fields were being decayed by the turn that made them | the turn-five fire had age 121 against the reference's 120 | `map::process_fields` collects its squares **before** it visits them, so a field created during the sweep waits for the next turn |

The second one was live in production code, not only in the test: `run_environment_turn` ran the fires and then decayed everything in the world, so every
spread cost its newborn a turn of life. The sweep is now split - `process_field_turns` takes the snapshot and `decay_the_fields_the_sweep_found` does the
work - and the environment turn hands over the list it took before the fires ran. `spared_newborns` pins it with no dice and no map.

**Still carved out, and counted rather than hidden:** when the fire asks for hot air the reference spends 104 more draws in the same processor - the cost of
adding a field to each new square - which the port does not model. The test asserts that number, so the gap cannot quietly grow or be replayed as fire dice.

## A whole turn now matches, die for die (round 325)

The ignored test from last round passes, and it took **six** corrections to get there. Each was found by the recording refusing a die, and each was a rule
the port had wrong or missing:

| # | What the trace said | What was wrong |
|---|---|---|
1 | `(0,7)` where the port wanted `(0,2)` | the flashpoint die was not being drawn at all |
2 | `(0,3)` after the phase | the wind-blocker scan start is drawn **before** the phase, used or not |
3 | `(0,4)` where the port wanted `(0,3)` | the last spread disjunct draws `one_in( 5 )` **before** asking whether items are there |
4 | `(0,100)` where the port wanted `(0,4)` | smoke is processed **before** hot air, not after |
5 | `(0,7)` before the spread loop | the spread loop draws **its own** start; the phase's start is for its own scan |
6 | age 21 against 22 | the turn is processors **and** the field's own decay, and the test only ran the first half |

**Ported this round:** `spread_phase` - the flashpoint/growth pair behind one `one_in( 3 )`, with the reference's `if / else if` nesting, the five-minute
transfer between fires, the `10_minutes x intensity` cost of growing, and `adjacent_fires` counted inside whichever branch ran. `Neighbour` gained
`fire_intensity` and `fire_age` so "weaker or older" can be asked at all, and `MapFacts` now remembers each fire's strength and age rather than just its
presence.

**Refactor:** `process_fires` had grown to 135 lines; the phase and the spread loop moved into `spread_and_flash`, and its nine arguments into a `Burning`
value. The function had stopped being one thing, which is the only reason to split it.

## A whole turn against the recording, and the rule it found (round 324)

`fire_session_parity` does what the last four rounds were building towards: it loads the session's **entire terrain grid** (the dump grew a `"grid"` of all 49
squares this round, because terrain under the neighbours is as much part of the answer as terrain under the fire), places the fire where the fixture has it,
and hands the port the reference's own dice for one processed turn.

It did not pass, and what it found is worth more than a pass:

| Finding | Consequence |
|---|---|
the reference draws `one_in( 3 )` **before** the spread loop, for the **flashpoint** rule | the port is one die behind from the second draw of every turn |
the flashpoint and the **growth** share that die as an `if / else if` | the port grows fires on turns the reference does not |
`adjacent_fires` is counted **inside** that block | the port's count is an input where the reference's is an output |

A real ordering bug was fixed on the way: `spread_fire` drew the terrain rolls before the chance roll, while the reference's condition is
`can_spread && rng( 1, 100 ) < spread_chance && ( ... flag && one_in( n ) ... )` and short-circuits left to right. Two tests now pin that order, including the
one that matters most: **a fire that fails the chance never rolls for the ground**.

The whole-turn test is `#[ignore]`d with the reason written into the attribute, rather than deleted or made lenient. It is not a green test; it is a named,
reproducible gap with the reference's dice attached, and the next round's work.

## The fuel rule moves into the fire's turn (round 323)

`FireMap::time_added` is gone. It was a value handed to the fire chain by whoever built the map, which made the one thing the fuel rule is - *a die* - invisible.
The trait now answers `ground( square ) -> Ground`, the four branches the reference distinguishes, and `process_fires` runs `ground_fuel` itself, in the
reference's place and order: hard ground asks `one_in( 3 )` first, then, only if it fed, `one_in( 200 - intensity x 50 )`.

**The session test got stricter in the process.** It used to supply fuel as a number and replay the trace's ages with invented dice. It now supplies a
recording of the reference's **decisions** - three dry turns, the fourth fed, then the bash die - plus each turn's hot-air gate, because the port asks that
question too, and a stream that omits it desynchronises at the first hot-air question. That failure is the reason the gate is in the recording rather than
left out: it is what the replay is for.

**Two expectations of mine were wrong, and both were sign errors about what fuel means.** A fire at age `-4` that eats four turns is at `-8`, not `0`:
negative age is fuel banked, which is the same convention the newborn-fire fix of round 310 turned on. And a `FLAMMABLE` ground gives `5 - intensity` turns
where `FLAMMABLE_HARD` gives `4 - intensity`, so the fixture's `+2` at intensity two can only come from the hard branch - which is why the session's ground is
now `t_floor`'s real class instead of a convenient one.

## A fire reading a real map (round 322)

Every answer the fire chain had been receiving from a test double now has a real source. `TerrainFacts` carries the six flags the reference asks about -
`FLAMMABLE`, `FLAMMABLE_ASH`, `FLAMMABLE_HARD`, `FIRE_CONTAINER`, `NO_FLOOR`, `SUPPRESS_SMOKE` - and `MapFacts` is those facts per square plus which squares
hold fires, implementing `FireMap` in full.

| Question | Answer's source | The reference's rule |
|---|---|---|
`can_burn` | the flags underfoot | `( ter.is_flammable() || frn.is_flammable() ) && !FIRE_CONTAINER` |
`neighbour` | the flags beside | the spread rule's four kinds |
`suppress_smoke` | the flag underfoot | `SUPPRESS_SMOKE` |
`smoke_ceiling_open` | the flag **overhead** | `has_flag_ter( NO_FLOOR, up )` |
`adjacent_fires` | the fire set | the eight around, same floor |
`can_spread` | a container holds its fire | a brazier does not spill |

Six tests, each a transcribed reference rule. The one worth reading twice is the brazier: it is `FLAMMABLE` and it must **not** feed the fire inside it, because
`FIRE_CONTAINER` overrides flammability - the kind of rule that is invisible until a player builds one and the fire burns for ever.

**Layering kept intact:** `cdda-sim` does not depend on `cdda-content`. The tests convert a loaded `MapData` into `TerrainFacts` the way the game's adapter
will, so the systems stay ignorant of JSON and the loaders stay ignorant of systems.

## Smoke rises through an open floor (round 321)

The last declared gap in the fire chain is closed: `FireMap` gained `smoke_ceiling_open` - is there `TFLAG_NO_FLOOR` in the square overhead - and `fire_discharge`
now puts the smoke **upstairs** when there is, at `rng( 1, intensity )` of thickness, and on the fire's own square at full intensity when there is not
(`map_field.cpp:1417-1432`).

| Test | What it proves |
|---|---|
smoke under an open floor goes upstairs | the rising branch, including the die that chooses its thickness |
smoke under a ceiling settles where it was made | the fallback, and that **no thickness die is drawn** there |

The second test's last assertion is the one that matters for parity: the two branches cost different numbers of dice, so a port that always drew the thickness
die would match the visible state of every ceilinged room and still be a draw ahead of the reference from then on.

**The default is "no".** A map that has not looked upward cannot promise a way through, so the trait answers conservatively and every existing test double kept
working unchanged - which is also how the reference behaves when there is no opening.

## Fires answer in whole squares (round 320)

`FireMap` asks about a **square**, not a column: every one of its seven questions carries `z`, and `process_fires` passes the fire's own floor. A newborn
fire lands on the floor it was born on, and a square that already holds a fire is checked with its floor - two fires on the same column one storey apart are
two fires.

| Test | What it proves |
|---|---|
the system asks the map about a whole square | every question names the fire's floor |
a fire upstairs leaves its smoke upstairs | the product lands where the fire is |

The first test is written against a map double that **records the questions it was asked**, which is the only way to see a coordinate that is missing from
a call: a two-coordinate map answers happily and the fire looks right.

**Still declared:** smoke does not rise yet. Whether the ceiling lets it through is a question about the terrain overhead, which nothing answers - so the
rule's "no" is passed in rather than guessed at.

## Smoke falls before it drifts, and rises last (round 319)

`process_gas` now answers all three of the rule's questions about the world, and the store does something with the answers: a gas over a hole **falls**
(cheaply - no die beyond the gate), anything else drifts sideways on its own floor, and a gas with nothing beside it **rises**. The destination is a full
three dimensional square, so the field created downstairs is downstairs.

| Test | What it proves |
|---|---|
a gas over a hole falls | the reference's first move, and that it costs no extra die |
a gas with no hole does not reach the floor below | the fall is conditional, not automatic |
a gas with nothing sideways rises | the last-resort branch |
smoke on the same floor still merges | unchanged by the vertical work |

**Two existing tests had to change, and both were lying.** They used a closure that said *every* square accepts gas - including the square below - so once
falling worked they saw the gas disappear through the floor and then reported the store was wrong. The closure now says what those tests actually meant:
every square **on this floor**. A test double that is too permissive is a test that cannot tell a bug from a feature.

**Still two dimensional:** fires. `FireMap` asks about a column, so a fire's smoke and haze land at street level whatever the fire's own floor is - which
matters not at all today, because nothing places a fire anywhere else yet.

## The store learns which floor it is on (round 318)

`Position` carries `z`, the position index is keyed by the column **and** the floor, and `add_field_strength` takes a three dimensional square. Fires are
still described in two dimensions - `FireMap` asks about a column - so their products land at [`STREET_LEVEL`], and the code says so instead of pretending.

| Test | What it proves |
|---|---|
the index keeps floors apart | the same column on two floors is two squares |
smoke added one floor down does not thicken the smoke above | a fire upstairs does not feed the cellar |
smoke added on the same floor still merges | the test above is not passing for the wrong reason |

The third test is the one that keeps the second honest: a store that never merged anything would pass the first two and be wrong everywhere else.

**Migration cost, recorded:** 52 struct literals, the index key, `add_field_strength`, and every call site that named a square - done mechanically with the
compiler as the checklist, and all 1,223 existing tests kept passing at every step. The systems above still compare two coordinates in places; those are
the next round's work, and each one of them is a rule that will now have a floor to be wrong about.

## The vertical half of the gas rule, and a self-inflicted scare (round 317)

`spread_gas` now carries all three directions the reference tries, **in the reference's order**: the square below first and *for free* (no die beyond the
gate), the eight beside it second, the square above last and only when nothing sideways would take it. `GasDirection` names the four outcomes and `Around`
holds the ten answers the caller has to supply. Two new tests pin the two vertical steps, and they are about **what is not rolled**: a gas over a hole never
draws for a neighbour, and a gas with nothing sideways spends exactly the two dice the scan needs before rising.

**The scare is worth recording.** Editing `crates/sim/src/lib.rs` by slicing between two doc-comment anchors cut out 60 lines of unrelated code - the light
and level section - because the second anchor was *above* the first in the file, so the slice duplicated one region and dropped another. The tests still
compiled and 1,223 of them passed, which is exactly why this kind of damage is dangerous: the compiler cannot miss code that no longer exists and nothing
references the missing light resource by name yet. What caught it was `git diff | grep '^-'` showing deletions I had not made. The file was rebuilt from
`HEAD` and the intended change re-applied by walking braces to the end of the function, with an assertion that the replaced span held exactly one
`pub fn`. Second time in this project that a text slice has eaten code; the brace walk is now the only method used for these edits.

## A whole environment turn, and the property every comparison rests on (round 316)

`run_environment_turn` runs the reference's `map::process_fields` order in one call: **fires** (growth, fuel, spread, hot air, smoke), then the **gases**
(smoke and the four hot air kinds), then **decay** through the sweep that owns it. The port keeps those as three systems so each can be tested alone; this is
the function that puts them back together, and it is what lets a test burn a room for twenty turns instead of calling three systems by hand.

Four tests, and the two that matter are about **determinism**:

| Test | What it proves |
|---|---|
twenty turns of a raging fire leave smoke and haze, and a fire still burning | the chain works end to end |
the same seed burns the same room twice, field for field | every comparison in this project rests on this |
a different seed burns it differently | the determinism test above is not passing for the wrong reason |
an empty level stays empty | the turn invents nothing |

The third test exists because of the second: a "determinism" test that would pass with a broken generator, or with the dice ignored entirely, is worthless, and
the only way to know which one you have is to check that the seed still matters.

## The haze a fire leaves is an entity (round 315)

`process_fires` now ends with `fire_discharge`: hot air first (`map_field.cpp:1429`), then smoke (`:1407`), in the reference's order. Hot air is chosen by the
fire's own intensity (`fd_hot_air1` to `fd_hot_air4`), lands on the five squares the dice name, and goes through the same merge rule as everything else.

Three tests, and the first one is a small lesson in reading a trace: the dice chose five offsets of which **two were the same square**, so the store ends
with four squares and one of them at intensity two. A test written from expectation ("five squares, each at intensity one") failed against behaviour that
was right - the merge rule from round 305 showing up in the wild. It is now asserted as four squares with one doubled, and the comment says which two
offsets collided.

**Refactor in the same round:** `process_fires` had grown to 103 lines and clippy refused it at 100. The tail moved to `fire_discharge` rather than the limit
being raised, because the function had stopped being one thing. The four hot air field ids also gained the documentation they had been missing.

## Gases move in the store (round 314)

`process_gas` is the join between [`spread_gas`] and the entity store: it walks every square holding the gas in the store's own order, asks the map which
neighbours would accept it, and applies the result - the source keeps `age - age / intensity` and one less level, the destination thickens if it already
held the gas or is created at level one with the share of age, and a source that reaches zero leaves the store.

Five tests: a level-one gas spends no dice, a thick gas gives a level and the right share of its age, a square already holding the gas **thickens rather
than becoming a second entity**, a gas with nowhere to go spends only the two dice the scan needs, and a gas giving away its last level... stays, because
level one is still alive.

**Two mistakes of mine that the recordings caught in the same round**, both about how many dice a step costs: I wrote a three-die recording where the
`one_in( candidates )` draw makes four, and `Recording` refused the range instead of answering the wrong die; and I mapped neighbour index 1 to the east
when this project's table puts it **north**, which changed where the gas went in every test. The second is the kind of error that a passing "the gas
moved" test would never show.

## Gases share one rule, and it has five dice with two short-circuits (round 313)

The 106 draws after a hot air creation are not a hot-air special case: hot air is a **gas**, and gases move by `map::spread_gas`
(`src/map_field.cpp:276-360`), which every gas shares because being a gas is `"percent_spread"` in its data rather than a processor of its own.

| Step | Dice | Short-circuit |
|---|---|---|
intensity one | **none** | `current_intensity <= 1 ||` stops before the gate |
the gate | `rng( 1, 100 - windpower ) > percent_spread` | a failed gate returns before any neighbour is examined |
which neighbour the scan starts on | `rng( 0, 7 )` | - |
whether it goes | `one_in( spread.len() )` | an empty candidate list ends the turn after two dice |
which candidate | `rng( 0, spread.len() - 1 )` | - |

`spread_gas` ports all five, plus the transfer arithmetic from `gas_spread_to` (`:250-273`): the source keeps its intensity minus one and gives the
destination `age / intensity`, so a thick young gas carries more of its age with it than a thin old one. Five tests, three of them about **what is not
rolled** - the two short-circuits and the empty candidate list - because a port that rolls one die too many diverges from the reference at the next
rule even when its own answer looks right.

**Still declared:** falling and rising. `spread_gas` tries the square below first and the square above last, and the port's store has no third
dimension, so both are named gaps rather than silently missing steps.

## Hot air, and the first rule ported *from the dice* rather than from the source (round 312)

`create_hot_air` is small and it is the last thing the fire processor does (`src/map_field.cpp:375-399, 1429`): a gate of
`adjacent_fires < 5 && rng( 0, 4 - adjacent_fires )`, then **five** squares drawn as `rng( -1, 1 )` for x and again for y, each set to intensity one.
Three functions now port it - `wants_hot_air`, `hot_air_field`, `hot_air_squares` - and the host test reads the reference's own recording rather than
a table I typed:

| Turn | Gate draw | What happened |
|---|---|---|
1 | `int 0 4 0` | refused - the gate is a **non-zero** roll, not a success test |
2 | `int 0 4 1` | requested, then ten `int -1 1` draws |
3 | `int 0 4 0` | refused |
4 | `int 0 4 4` | requested, then ten draws |

The test asserts the port consumes exactly the recorded dice in the recorded order and lands on the recorded squares, and that a fire with five
neighbours rolls nothing at all.

**The next gap is now measured, not guessed.** The turn that creates hot air is followed by 117 draws in total, of which only 11 are the rule above: the
other 106 belong to the hot air field's own processor, which emits and spreads in `emit.cpp`. So the port can match the refuelling and the spread and
the smoke of a turn and still be 106 draws out at the end of it. That is the next piece of work, and it has a number attached.

## The reference's dice, labelled (round 311)

The oracle now **labels its own dice**. `rng_mark( "fire.before_fuel" )` and its four siblings write a line into the `CDDA_RNG_TRACE` file at the boundaries
of the fire processor, and the session dump records `draw_start` - the draw counter when the session began - so the session's dice can be sliced out of a
file that also holds three and a half million draws of world generation. Both are checked in: `corpus/fields/fire-floor.rolls.txt` is 277 lines and
`core_types_suite/fire_ground.rs` reads it.

**What the labels settled.** The fuel accounting asks `one_in( 3 )` on hard ground and then, *only if the ground fed*, asks `one_in( 200 - intensity x 50 )`
about bashing it. The trace proves the order: the feeding turn shows `int 0 2 0` followed by `int 0 99 11`, and the three dry turns show a single
`int 0 2 1` each. `ground_fuel` now performs exactly that, and the test asserts the port **asks for the same dice in the same order** and leaves none
unplayed - which is a different claim from "gets the same answer", and a stronger one.

**What the trace also showed, and what it costs to ignore.** The first processor run per session is preceded by two `rng( 1, 1 )` draws from the sweep
itself, and creating hot air rolls dozens of dice through `emit`. A port that implements the fire rules but not those will diverge from the reference at
the first hot air, no matter how right its fire is. That is not a reason to stop: it is the reason the trace exists, and the next gap is now named
precisely rather than felt as a vague worry.

## The port replays the reference's fire, and the fixture found a rule I had missed (round 310)

`field_session.rs` reads `corpus/fields/fire-floor.json` and replays it: for each of the five turns it feeds the port the fuel the reference's trace
implies and asserts the port's fire ends at the **same intensity and the same age**, to the turn. It is the first test in the project that compares the
port against a recording of the reference's own behaviour rather than against a transcription of its source.

**What it caught:** the reference's trace goes ages `0, 1, 22, 43, 42`, and the port went `0, 21, 42, 63` - one turn of fuel ahead from the start. The
missing rule is the one already written in the ledger for fields and never carried across to fire: **a field created this turn is spared its
processors** (`map_field.cpp:445-499`) and only decays, so a fire's first turn costs it one turn of age and nothing else. `process_fires` now skips
anything at age zero, and an older test that had a brand-new fire take its turn was corrected to start one turn old - with the reason in the test.

**What the test deliberately does not claim:** that the port rolls the same dice. The reference has consumed an unknown number of rolls during world
generation before a session starts, so a port with a fresh generator cannot be at the same place in the stream. The fuel decision - `one_in( 3 )` on hard
ground - is therefore an **input** to this test, pinned separately by the rules' own unit tests. Saying that in the test file is the difference between a
parity claim and a claim about parity.

## The age that went down, and why it was right (round 309)

Round 308 left two open questions against the first field fixture. Both are answered, and the answer needed a **seeded** run and the **terrain**
in the dump:

* The fixture had no `--seed`, so every run generated a different fixed world (`cli_opts.seed = time( nullptr )`). Two dumps that looked
  contradictory were two different worlds. With `--seed 12345` the session is deterministic and the ages are a clean sequence.
* The ground under the fire was invisible. With terrain and furniture in every row the trace explains itself: `t_floor` is **`FLAMMABLE_HARD`**, so
  each turn either the ground feeds the fire (`one_in( 3 )`, `time_added = 1_turn x ( 4 - intensity ) = 2`) or it does not, and the field's own
  `do_decay` adds one turn either way. A feeding turn therefore moves the age **down** by one - 43 to 42 - while the fire is burning hotter, not colder.

That downward step is the reason the fixture exists. A port that only tested "a fire's age goes up" would pass with the wrong rule, and I had already
written down that I would not encode numbers I could not explain. The trace is now in `corpus/fields/README.md` with the exact command that produces it,
because the command has three traps in it - the seed, the offset being relative to the player, and the terrain being part of the answer.

## The oracle can now film a fire, and it caught a sign error in the port at once (round 308)

`--dump-field-series` runs an **environment-only session**: it places a fire next to the player, runs the map's own
`process_fields` a turn at a time, and writes every field entry - square, type, intensity, age - plus the scent grid, after each turn. That is the
shape the P2 exit gate asks for, and the first fixture is `corpus/fields/fire-floor.json` (five turns, intensity two, radius three).

What the first run showed:

| Turn | Fire at the player | What else |
|---|---|---|
0 | intensity 2, age **0** | nothing |
1 | intensity 2, age 1 | nothing |
2 | intensity 2, age 60, and a **new fire** at a neighbour with age **120**, intensity 1 | `fd_hot_air2` |
3-4 | ages 59, 58 | `fd_hot_air1`, `fd_hot_air2` |

**The sign error it caught.** `add_field( dst, fd_fire, 1, 2_minutes )` gives the newborn an age of **+120**, and a field's age is *time burned* - the
reference's own comment is "Lower age is a longer lasting fire" (`map_field.cpp:1122`), and `do_decay` resets the age when the half-life roll beats it.
The port had the newborn at **-120**, which reads as two minutes of *surplus fuel*, i.e. a long-lived fire where the reference makes a weak one. Fixed,
with the test now asserting +120 and saying why.

**Declared and not yet encoded:** the parent's age going 60, 59, 58 - one turn down per turn - is not explained by the rules I have; the fuel
accounting at `map_field.cpp:1122` moves age by `-time_added` when the fire eats and `+10s x intensity` when it does not, and neither is minus one.
The fixture also contains `fd_hot_air1/2`, which the port has no field type for yet. Both are written down here rather than papered over: a fixture whose
numbers I do not understand is a fixture I must not turn into a passing test.

## The oracle had two authorities, and the stale one would have deleted eleven dump branches (round 307)

`oracle/apply_harness_patches.py` edited the reference call by call, and `oracle/src` was the tree everything actually ran from. Over many rounds the working
copy grew **eleven `--dump-*` branches** in `main.cpp` that the script never learned about, so:

* the script's own `--verify` failed - it reproduced the working copy for every file except `main.cpp`;
* re-running it, which is the first step of `oracle/build.sh`, would have **silently deleted those branches**, and with them every fixture generated since;
* `oracle/harness/harness.cpp` was a second copy that had drifted 126 lines behind the working copy, so the "regenerate the working copy from the pinned
  checkout" story was false in two places at once.

This is the same failure I recorded for fields at round 290 - *a second authority is worse than a gap, because it looks like progress* - and I did not
apply the lesson to the instrument itself. Fixed by removing the second authority rather than reconciling it:

| Before | After |
|---|---|
a hundred string patches in Python | **one generated patch**, `oracle/patches/0001-prototype-digest.patch` |
`oracle/harness/` as an editable copy | deleted; the working copy is the source and `--regen` writes the patch |
`--verify` compared ten named files | `--verify` reproduces the **whole** source tree from the pinned checkout and diffs it |
"already applied" guessed from an error string | a **reverse** dry run, which is the only honest way to tell apart "applied" from "drifted" |

`--verify` prints `the patch reproduces the working copy from the pinned checkout, byte for byte`. There is also a glob of junk (`oracle/src/src/src/`, with
`.rej` files and duplicate harness sources) that a bad patch run left behind and that I deleted - it was never compiled, but it made every `diff -r` lie.

## Smoke reaches the store, and the fire's questions become a trait (round 306)

`process_fires` now asks the map seven questions instead of five, so they are gathered behind a `FireMap` trait: how many fires touch the square, whether
the ground burns, how much was eaten, whether it may spread, what a neighbour looks like, whether smoke is suppressed, and how much smoke the fuel made.
The signature went from six positional closures to `(world, map, windpower, rolls)`, and every fire test now spells out the answers it wants instead of
passing a row of `|_| true`.

**Smoke lands in the store.** A fire's smoke goes through the same merge rule as any other field, so two turns of smoke thicken rather than replace. The
port's store is two dimensional, so the "above" branch of the reference has nowhere to go and the fire's own square is used - which is the reference's own
fallback when the smoke cannot rise. **Declared gap:** a fire on a multi-level map will smoke its own square rather than the one above, until the store
carries z.

Three tests, and I proved them red before trusting them: with the smoke call replaced by a no-op both "eventually smokes" and "adds up" failed, and both
went green when it came back. Writing a test after the code and calling it a gate is a lie the ledger should not tell, so the round records the check.

## Two patches of smoke add up (round 305)

`field::add_field` (`src/field.cpp:158-175`) does **not** keep the stronger of two fields of the same kind on a square: it writes
`prev_intensity + new_intensity` and lets the setter clamp to the type's maximum. The sim's helper was keeping the maximum, which looks reasonable
and is wrong - a second patch of smoke on a smoky square makes it smokier, and the difference is invisible in any single-patch test. Three tests now
pin the sum, its clamp, and the empty-square case.

**A process failure in the same round, recorded because it nearly cost the file:** replacing that helper with a text slice, I cut at the next `fn `
boundary and deleted **109 lines** of `crates/sim/src/lib.rs`. `git diff --stat` caught it before anything else ran, and `git checkout` restored the
file. The replacement then walked braces to find the function's real end and asserted the slice contained exactly one `fn`. A whole-file restore is a
destructive command and would have needed asking; reverting a *file* I had just damaged in a working tree I could see is the smaller, safer act, and
it is now done by brace matching rather than by guessing where a function ends.

## The smoke a fire leaves (round 304)

`smoke_from_fire` ports the block at the end of the fire processor (`src/map_field.cpp:1407-1425`). Three things in it reward being written down:

* **`SUPPRESS_SMOKE` is tested first**, so a sealed fireplace spends **no dice at all** - a short-circuit that is part of the world, because every
  later roll in the turn sees the same stream either way.
* **Wind makes smoke more likely, not less.** `rng( 0, 100 - windpower ) <= smoke` narrows the range as the wind rises, so for a given smoke count
  a windy turn passes more often. The reference's own comment suggests the opposite; the arithmetic is what the port follows.
* **Where it goes depends on overhead:** above at `rng( 1, intensity )` when the square overhead has no floor, otherwise on the fire itself at its
  **full** intensity, which is thicker because it has nowhere to rise to.

Five tests, including that a weak fire smokes less often than a raging one across four hundred seeds each, and that suppression leaves the dice
stream untouched.

## A spread fire that becomes a fire (round 303)

The sim now puts the spread into the store: `process_fires` asks the map for each of the eight neighbours, runs [`spread_fire`], and spawns a
fire on every square that caught - intensity **one** with **two minutes** of fuel (`add_field( dst_p, fd_fire, 1, 2_minutes )`), so a new fire
does not immediately jump again, while the fire that spread pays a minute of its own life. One fire per square: the world is keyed by square
even though the store is keyed by entity.

Four tests, and one of them reproduced last round's mistake exactly: I wrote that a spreading fire ends sixty turns older, and the code said
ninety, because the ten-second drain for standing on burnable ground comes **first**. The same ordering caught me out two rounds running, which is
worth noting as a pattern rather than an anecdote - the fuel accounting is at the top of the processor, and every later rule is a change on top
of it.

## Fire jumping to a neighbour (round 302)

`spread_fire` ports the spread loop at `src/map_field.cpp:1285-1345`. Its **shape** is the rule and each part of the shape is a way to be wrong
without any test noticing:

| Part | Why it is there |
|---|---|
the loop starts at a **random** neighbour and wraps | the order it considers squares in is part of the world, not an optimisation |
a neighbour is skipped on `one_in( intensity * 2 )` | a bigger fire looks at fewer neighbours per turn, which is the opposite of the obvious guess |
the chance is `25 x ( intensity - 1 )`, or `50 + chance / 2` onto a **web** | so a single square of fire can cross a spider web and cannot cross a lawn - the case my intuition missed |
`power = intensity + one_in( 5 )` gates the ground: `FLAMMABLE`/`FLAMMABLE_ASH` at power two with a coin toss, `FLAMMABLE_HARD` at power three with one in five | the flags are not interchangeable, and the hard case is much harder |

Seven tests, including that a neighbour already burning is never re-lit, that a fire in a brazier does not spread at all, and that two runs from
one seed stop at the same point in the dice stream whatever they decide - which is the property a replay depends on.

## Fire in the store (round 301)

`process_fires` is the join: it walks the fire entities in the sweep's order, drops any whose turn begins with nothing left, and hands each
one [`fire_turn`] with the three answers the environment layer owns - how many fires touch it, whether the ground under it burns, and how
much it ate this turn. Five tests: a fed fire with neighbours grows and pays, a fire on nothing ages ten seconds a level, a fire that ate
this turn spends what it ate, an extinguished fire leaves the store, and a field that is not fire is untouched.

**A test I could not make pass, and why that was the right outcome:** I first tried to age a fire to death here. `fire_turn` never lowers an
intensity - the half-life roll in the field sweep does that - so the test could only have passed if the port had put a rule in the wrong
place. It is now a test of the removal path instead, with the reason written down.

## Fire's arithmetic, without the burning (round 299)

`field_processor_fd_fire` needs items, terrain flags, weather and vehicles, and none of those exist yet - so it is **not** ported, and the
ledger says so rather than shipping a rule that would look like progress. What is ported is the part that needs none of them, and it is
the part that decides a fire's shape:

| Rule | Reference | What it says |
|---|---|---|
`growth_cap` | `map_field.cpp:1222-1257` | a lone fire reaches **1**; three neighbours reach 2, seven reach 3; more than 500 minutes of fuel reaches 3 on its own; more than 50 minutes reaches 2 |
`growth_cost` | `:1253` | growing to *n* costs **10 minutes x n**, read after the rise, so growing to three costs thirty and not twenty |
`dying_out_cost` | `:1131` | a fire with nothing to burn loses **10 seconds per level** a turn, so a raging fire dies faster than a small one |
`spread_chance` | `:1308` | `25 x ( intensity - 1 )`, or `50 + chance / 2` onto a web - so intensity one **cannot spread at all** |

**Now ported:** `fire_turn`, the two adjustments in the reference's order (`:1122-1131` then `:1222-1257`) - what the fire ate is credited first, a fire on burnable ground loses ten seconds a level when it ate nothing, and then it grows a level if its age is negative and the cap allows, paying ten minutes a level read after the rise. Ten tests.

**A unit error caught by writing the tests, worth recording because it is the sort that survives review:** my first version of these functions returned the reference's *second* counts while documenting them as **minutes** - a factor of sixty - and the port counts in **turns**, one of which is a second. Two more of my expectations were wrong in the same round and the code was right both times: ten thousand turns of fuel is over five hundred minutes and reaches three on fuel alone, and a fire that grows also pays the ten-turn drain for standing on something burnable.

Six tests, and two of them are the reference's own comments being wrong about its own code: it says "2 for at least 1 neighbor, 3 for at
least 2 neighbors" where it checks **three** and **seven**, and its worked example calls fifty minutes of fuel "2 (medium)" where the test
is `age < -50`, strictly less. Both boundaries are pinned the way the **code** has them, with the mismatch written down - a transcription
that trusted the comments would be wrong at exactly the edges where a fire's behaviour changes.

## Correction: one authority for fields, not two (round 290)

Rounds 285 to 289 built the field turn against a **container on the level** - `Level::field_squares`, `take_field`, a sweep that
walked it - and that was wrong. The ledger already said so two phases ago: fields are **entities in the store**, `process_fields`
walks the store, and *the per-square container in the map layer is deleted, because one authority is the point*. I added a second
one, and it survived five rounds of green tests precisely because nothing compared it against the store.

What survives the correction, and where:

| Piece | Where it lives now |
|---|---|
The turn's numbers per intensity | `FieldLevel::turn` and `turn_at` in `cdda-content`, gated against the reference's own field dump - **every** level hands over its own chance, count, group, radius and upgrade; the data has **two** spawning levels |
The three steps of a turn | `process_field_turns` in `cdda-sim`, over the store: a dead field is gone before anything runs; a field **created this turn** is spared its processors and only decays; the decay comes **last** |
The sweep order | the same system: squares in **x outside, y inside** order, then each square's fields by entity, with a test that records the squares it walked and gets them column by column |
The live intensity | the system asks `turn_of` for **each** field, so a field that upgrades takes the new level's numbers on its next turn - the test observes the turn contents, one monster at intensity one and none at intensity two |
The spawn rule | `spawn_monsters_from_field` in `cdda-content`, unchanged; the sim system hands the turn to the game layer, which owns the group table and the creatures, so `cdda-sim` keeps its one dependency |
The map-layer container | **deleted**, with the three test files that exercised it: a second authority is worse than a gap, because it looks like progress |

`FieldTurn` moved to `cdda-core-types` so the sim can name the turn without depending on the data crate.

## The turn reads the field's own numbers (round 287)

`FieldLevel::turn()` builds the turn from the level's own data instead of a hand-built constant. The reference registers each
processor at load time and reads its numbers off the level it runs at (`field_type.cpp:350`), and a rule handed its constants by
hand drifts while every test stays green.

Two tests: **every** level in the reference's field dump hands over its chance, count, group, radius and upgrade exactly as written -
and the data has **two** spawning intensity levels, which is the count the monster spawn gate was built around; and `fd_smoke`,
which spawns nothing, says so at every level.

## The turn reads the field's own numbers (round 287)

`FieldLevel::turn()` builds the turn from the level's own data instead of a hand-built constant. The reference registers each
processor at load time and reads its numbers off the level it runs at (`field_type.cpp:350`), and a rule handed its constants by
hand drifts while every test stays green.

Two tests: **every** level in the reference's field dump hands over its chance, count, group, radius and upgrade exactly as written -
and the data has **two** spawning intensity levels, which is the count the monster spawn gate was built around; and `fd_smoke`,
which spawns nothing, says so at every level.

## What a field does on its turn (round 283)

`run_field_turn` ports the three steps of `map::process_fields_in_submap` (`map_field.cpp:470-499`) in their order, which is the
whole rule:

1. A field with no intensity is **gone** - removed before anything runs.
2. A field **created this turn is not processed at all**: it decays and nothing else, which is what gives a player standing in
   freshly-lit fire a turn to step out.
3. Otherwise the processors run - spawning among them - and the decay comes **last**, so a field that spawns this turn is still
   at full strength while it does.

Four tests: a newborn field spawns nothing, an aged one spawns and then decays, a level with no spawn data runs no spawner, and a
dead field does nothing.

**One of my assertions was wrong and the code was right.** I wrote that a newborn field's age stays at zero; it decays - that is
the whole of what happens to it - so the age moves to one. The distinction the test now draws is between *skipped entirely* and
*spared its processors*, which is exactly the distinction the reference makes.

## Spawning against a real level (round 282)

A field processor does not have a predicate, it has a **map**: `spawn_monsters_on_level` joins the two, with the level answering
what is on a square and a terrain predicate answering whether that takes a creature. Three tests on a level with a wall down one
column: across sixty seeds **no monster ever stood in the wall**, a field beside the wall put every monster on the open side, and
a field walled in on all sides spawned nothing.

The terrain table stays a function rather than a second copy of the loader, so this remains a rule about spawning.

## Whether a square will take a creature (round 281)

`map::passable_through` is two questions joined by **and** (`map.cpp:2419-2422`): room (`move_cost != 0`) and a floor (not
`NO_FLOOR`). They are different refusals - a wall has a floor and no room, open air has room and no floor - and a port that only
checked the move cost would let a spawned monster stand in mid-air.

The port now has both forms: `passable_through` for a single level, and `passable_through_with_support` for a caller that knows
what is underneath, which is how a bridge over a chasm passes and the chasm does not. Three tests: a wall takes nobody and a
floor takes anybody, `NO_FLOOR` refuses a square that has room, and support below turns that same square passable.

## Gate, group, square - composed (round 280)

The three pieces exist separately; `spawn_monsters_from_field` composes them, and the composition is where the **order** of the
dice lives: whether anything spawns is decided first, so a field that does not trigger spends exactly one die, and only then are
monsters picked and placed. A caller that picked before rolling the gate would produce a plausible world on different dice,
which is the kind of error only an ordered replay catches.

Four tests: three monsters inside the box, a field walled in on every side but one putting **everything** on that square (which
forces the enumeration path in the placement), a radius with no open square producing **nothing** rather than a monster on a
wall, and the same seed producing the same monsters on the same squares.

## Where the monster lands (round 279)

`random_point_in_radius` ports `random_point( points_in_radius( p, radius ), passable )`. Three details matter and none is the
obvious reading:

* **The "radius" is the bounding box**, not a circle: `points_in_radius` is `center - offset .. center + offset`
  (`map_iterator.h:154`), so the corner of the box is a legal spawn even though it is further than the radius in a straight line.
* **Ten optimistic tries** first (`map_selector.cpp:59`), three draws each, before it enumerates the range at all.
* **The enumeration runs x fastest** (`map_iterator.h:44-58`), so the fallback list is row-major.

Five tests: a radius of zero can only produce the centre; a blocked centre is never chosen in fifty seeds; **a corner of the box
is reachable** - the test that fails on a circular implementation; a range that refuses everything returns nothing; and the same
seed lands on the same square.

Also corrected: the first version of the "ten tries" test asserted a hard-coded square and was wrong about what it was checking.
It now compares a refused run and an accepted run **from the same seed**, so they walk the same squares and differ only in what
the predicate says.

## Picking from a group, and the default weight that was hiding (round 277)

`pick_monster` now resolves a group to a monster: weighted pick, and when an entry names another **group** the pick repeats
inside it, bounded so malformed data that refers back to itself cannot hang a spawn. Four tests, including twenty thousand
draws of `GROUP_GOO` landing within three points of the data's own shares (blob 69.3%, small blob 30%, shoggoth under 1%).

**A default I had guessed wrong, caught by a test that kept returning nothing.** Entries without a `"weight"` were loaded as
zero, so they could never be picked - and a group made only of other groups produced no monster at all. The reference reads
`mon.get_int( "weight", 1 )` (`mongroup.cpp:496`): the default is **one**, not zero, and a zero-weight entry is a way to
disable something rather than to leave it unspecified.

## Monster groups: 401 groups, 2,782 entries (round 276)

The table a spawn point draws from is loaded: **401 groups and every one of their 2,782 entries**, checked against a
transcription of the reference's own data rather than against a reading of it.

Two shapes the first attempt missed and the gate caught:

* **Groups nest.** An entry names either a monster or **another group** (`"group": "GROUP_ZOMBIE"`). Reading only `monster`
  dropped a third of the entries - `GROUP_FOREST` came out empty against the data's twenty-three.
* **A group id can appear in several files**, and a mod appends rather than replaces, so entries accumulate in load order
  instead of the later object winning.

That closes the loader the monster spawn gate was waiting on, which is the piece P2's field spawning was missing.

## NPC classes, 155 of 155 (round 274)

The largest remaining hole in the P1 loader families was `npc_class` - what an NPC is before they are given a personality:
their starting skills, traits, worn gear and carried items. Nothing in the port read it, so every NPC-bearing mapgen square
was untranslatable.

The port now loads the class table: **155 classes, exactly the reference's ids, none invented, none missing**.

**A correction to my own last two ledger entries.** I wrote that this moved the content registry from 86 to 87 of 90. It did
not: the registry gate scans the **data files** for each type, not the port's loaders, and it has been reporting **90 of 90
exact with no named gaps** all along. What was missing was a *loader* - the code that turns those ids into something the port
can use - and the count I quoted was a stale figure I had repeated without re-reading the gate that produces it. The gate's
own receipt is the number that counts, and it says 90 of 90. The two tests are the same shape the registry gate uses - the reference's own id
artifact, compared in both directions - plus one that pins the **name** parsing: the data writes a name as a translated string
object (`{ "str": "No class" }`), not a bare string, and a reader that only looks for one form loads 155 empty names without
failing.

The skill and gear tables are not ported: they are what a class *does*, and they wait for the code that consumes them.

## The reference's own dice (round 266)

The reference now records **every die it rolls** - `CDDA_RNG_TRACE` writes `bits <value>` for a raw draw and
`int <low> <high> <value>` for a bounded one, in the order they were rolled. A generation run produced **3,281,874** draws.

On the port's side `Rolls` gained a replay mode: attach a `Recording` and every `rng`, `rng_bits` and `one_in` answers from
it instead. Three decisions make it useful rather than decorative:

* **The bounds are checked.** A recorded bounded draw remembers the range it was asked for, and replaying it under a
  different range **fails** instead of answering - a port asking a different question than the reference did is exactly the
  divergence this exists to expose.
* **Running dry fails too.** Spending more dice than the reference did on the same rules is a divergence in the order of
  draws, and inventing a die would hide it.
* **The ordinary path is untouched.** With no recording attached the engine answers and nothing pretends a reference was
  replayed; the same seed still draws the same value.

This is the mechanism that turns the forest comparison from *shares that match* into *the same chunk*: the same perimeter,
the same palettes, and the reference's own dice.

## The record is a full grid, in the reference's own order (round 259)

Before any dice can be replayed, the record it will be replayed from has to be complete and ordered. The per-square feature
record is now checked as a **grid**: reading it in file order and rebuilding the chunk by coordinate visits all 576 squares
exactly once, and the file order is confirmed column-major - same column, next row - which is the reference's `x` outside,
`y` inside loop.

That closes the class of failure that has cost the last six rounds in three different forms: two records of the same thing
keyed differently (overmap against submap), written at different stages (generator against finished map), or read in the
wrong order (column-major file, row-major dump). The record now states its chunk, its stage and its order, and each of the
three is a gate rather than a convention.

## From shape to mix (round 258)

The comparison is now about **shares** rather than squares. Two independent sets of dice cannot agree square by square, but
the proportions have to be close: the reference puts 258 of 576 squares on `t_region_groundcover` (0.448) and 253 on
`t_region_groundcover_forest` (0.439). A rule that weighted the two wrongly, or a ticket pool that favoured one side of the
chunk, moves those by whole tenths - and the port's own run sits inside a fifteenth of both.

This is the strongest statement the port can make about forest generation **without sharing dice**: same inputs, same stage,
same rules, and a mix that matches. Positional parity still needs the reference's dice replayed step by step, which is the
next rung.

## The port generating the reference's chunk (round 257)

With the four records on one key, the port now runs the reference chunk's **own perimeter** through its own rules and
produces a chunk of the right shape: hundreds of placeholders left in place, ground cover dominating. It draws its own dice,
so this is not a positional comparison - what it settles is that the port stops at the same stage and for the same reason.

**The test caught its own author first.** My initial version expanded the region's default ground cover into `t_grass`,
`t_grass_dead` and `t_dirt` - real terrains - and the port then produced only **55** placeholders of 576. That is the exact
eagerness this test exists to catch, and it was in the test rather than in the port: the region settings write the
**placeholder** `t_region_groundcover`, and expanding it early moves the resolution from the sweep into generation.

## One chunk, four files, one moment (round 256)

A single generated chunk is now recorded four ways that all describe the **same** moment and the **same** square: the
perimeter the edge curve produced, its eight neighbour terrains, the placeholder every square ended up holding, and the
per-square record of which of the six tickets drew it. **574 of 576** squares in it are placeholders; two hold a terrain the
raffle placed directly.

**A coordinate trap worth recording**, because it silently produced a comparison of two different chunks: the perimeter and
neighbour traces name the chunk by **overmap** coordinate, while the placed and feature traces name it by **submap**
coordinate - twice as large. `[298,59]` and `[596,118]` are the same chunk, and comparing them as strings finds nothing.

## The port stops where the reference stops (round 255)

The oracle's finding changed the port's shape: generation now has a **placeholder-producing form**
(`forest_ground_pass_placeholders`) and the resolution sweep is a step of its own. The resolving form of the pass still
exists and simply calls the same code with the sweep attached, so nothing that used it had to change - but a comparison can
now be made **at the reference's own stage** instead of against a finished map.

The reference's chunk, as generated, holds **567 placeholders of 576** squares - only nine squares skip the palettes
entirely - with `t_region_groundcover` 258 times and `t_region_groundcover_forest` 253. That is a far more placeholder-heavy
chunk than I would have guessed, and it is the number a port's generator has to produce before its sweep.

One more expectation of mine corrected: I wrote 400 placeholders, carrying a different run's figure into this chunk. The
data said 567.

## The generator leaves placeholders, 576 squares of 576 (round 254)

Two traces of one run settle it: the per-square raffle, and the chunk read back **from the very map object the placement
used**, written the moment generation finished. They agree on **576 of 576** squares - the terrain a finished forest chunk
holds is the placeholder the raffle chose, unreplaced, and **more than four hundred** of its squares still begin `t_region_`.

That is the answer to three rounds of disagreement. The port must produce placeholders at the end of generation and resolve
them in a separate pass; comparing a generator against a *finished* map's dump compares a chunk before resolution with one
after, which is why half the squares never matched, why no rotation or transposition fixed it, and why a placeholder like
`t_water_dp` appeared to resolve to grass.

**A lesson about the instrument, not the code:** the dump is the wrong ground truth for a generation step, because it holds
the world after every later pass. A trace written *inside* the step that produced it is the only thing that can be compared
with something else from that step. Both traces now carry the chunk identity, so that comparison is a lookup.

## The second draw, found in the source (round 253)

The missing pass has a name: `resolve_regional_terrain_and_furniture` in `mapgen_functions.cpp`. Map generation writes
placeholder names like `t_region_groundcover_forest` into the map and leaves them there; **a separate pass over every
square** replaces each placeholder with a weighted draw from the region's palette, and only then is the chunk finished.

That changes the model the port was built on. A chunk's terrain is the generator's choice **plus a second draw**, and a port
that stops after generation produces chunks full of placeholders - which is exactly what my feature trace was recording,
square after square, without my noticing that the reference resolves them afterwards.

The port now has that pass as a step of its own - `resolve_chunk_placeholders` - taking a chunk, the region's palettes and
the dice, and returning how many squares it changed. Four tests: placeholders become terrain, they land only on entries of
their own palette (ten to one in the sample, and four hundred draws show both entries), the same seed resolves the same
chunk the same way, and a chunk with **no** placeholders spends **no** dice - because the pass draws only where it resolves,
and a port that drew per square regardless would put every later square of the world on different dice.

## The trace names its chunk, and the finding survives (round 252)

Every feature-trace line now begins with the chunk it belongs to, which turns the 576 lines of a chunk into a **lookup**
instead of a guess. The doubt of the previous round is gone and the finding underneath it is intact: even for the chunk the
dump describes - wet woodland at `[129,206]`, its trace identifier `[258,412]` being the submap origin - **260 of 576**
squares hold a terrain their recorded palette could not produce. The placed terrain comes from somewhere this trace does
not record.

Two things the tagged trace also settled, both of them mine to get wrong: this chunk's ground cover is mostly **swamp**
palette (117 squares) with only seven squares of dry ground cover, because it is wet woodland - and I carried the previous
chunk's hundred-and-three dry draws into a test of this one, which the data refused at once. And all eighteen chunks the run
generated are present as blocks of exactly 576 lines, so the identifier is per chunk and not per call.

## Which block of a trace is which chunk (round 251)

The previous round's 273-square discrepancy survived a harder test and changed shape. The feature trace and the perimeter
trace are written **by different call sites** - one per generation call, one per chunk - and nothing ties them together, so
the question "which 576-line block belongs to this chunk" has no recorded answer. Every alignment tried leaves at least
**273 of 576** squares outside their recorded palette, and the best leaves **303 inside**; no constant offset fixes it.

So two things are true at once, and the ledger says both:

- **The counts are solid.** Which palette each square drew does not depend on alignment: 400 ground cover, 97 dense trees,
  75 dense shrubs, 4 young trees. Those come straight from the raffle.
- **The comparison is not yet trustworthy.** Whether the placed terrain follows the raffle cannot be answered until the
  trace **names its own chunk** - the same defect that made a coordinate from the map's absolute submap unreliable in round
  243. The fix is one field: write the chunk's identity on every line, then alignment is a lookup instead of a guess.

The test states the finding as a range for that reason, rather than pinning a number that belongs to a guessed alignment.

## The raffle's choice is not what the map ends with (round 250)

The feature raffle is now traced square by square - the ticket, the palette name it produced and any furniture. Comparing
that against the same chunk's terrain dump settled a question and refuted an assumption.

**Settled:** the palette names are real and countable. Of 576 squares, **400** drew the region's ground cover, **97** dense
trees, **75** dense shrubs and **4** a young tree directly - numbers that come from the raffle itself and not from the map.

**Refuted:** that the map ends with what the raffle chose. **273 of 576** squares hold a terrain the palette they recorded
could not have produced, and the mismatches are systematic - ground cover recorded with underbrush, a shrub or a tree
standing there. So the final terrain comes from **another pass**, and the palette's weights cannot be checked against this
chunk until that pass is traced. The tempting number - forest floor taking 186 of the 400 ground-cover squares against a
weight share of 714 - is therefore **not** evidence about the weight, and recording it as though it were would have been
the kind of plausible wrongness this ledger exists to catch.

**One measurement mistake worth writing down:** the trace is written in the reference's loop order - x outside, y inside -
while the dump is row-major. Comparing the two by position, which I did first, makes every square disagree. They line up by
coordinate only.

## Twenty squares the tickets cannot explain (round 249)

A square reaches the ground raffle when its feature ticket is empty or names a side that owns no biome - both read off the
tickets, so the count has an upper bound however the chunk's sides are assigned. On the recorded chunk that bound is
**553** (every ticket but this square's own falls through) and the ground raffle ran **573** times.

The twenty extra squares are the interesting ones: they drew a ticket naming a biome, and still needed ground, because the
feature pick produced **furniture** - a boulder or a region's own furniture palette - or no layer fired at all. A port that
counted only tickets would put solid ground on those twenty squares. Pinning the number is the first half; reproducing the
next chunk's terrain needs the feature pick traced as well, which is the next piece of instrumentation.

## The corner rule is dead code (round 248)

The gap of the previous round is closed, and it was not a subtlety in the guard - it was that **the rule never runs.**

`mapgen_forest` declares `forest_biome self_biome = current_biome_def_it->second;` - a **copy** of the region's biome -
and then asks `ccw == &self_biome`. The neighbours it compares against are pointers **into the region's biome map**, so
one is a heap address inside that map and the other is the address of a local copy: they are never equal, and both
branches begin with exactly that comparison. `unify_continuous_border` is unreachable.

A traced world says the same thing independently: four chunks generated beside open ground are explained by the **raw**
neighbour weights, plain woodland at density three, on **576 of 576** squares each - no blending anywhere.

The two mistakes this corrects are mine, and both were plausible:

- **The "edge chunks".** Round 246 found six chunks no side mask explained and read that as corner blending firing. It
  was the assumed **density**: those chunks are plain woodland, and evaluating them at the denser woodland's density of
  four fits nothing.
- **The 288-of-576 score.** Round 247 read it as the port's corner arithmetic disagreeing with the reference's. It was
  the port's **test** applying a rule the reference does not execute; the port's own generation pass never called it.

The function stays in the core types, tested, marked as the rule the source *means* to apply rather than the one it
applies - because deleting it would hide the finding, and calling it would invent behaviour.

## The corner rule against a real edge, and the gap it exposed (round 247)

The trace now records each chunk's **eight neighbour terrains** beside its weights, so the whole chain - perimeter to
neighbour weights, corner blending, then the six-ticket pool - runs against a real forest edge.

**What it settled.** The chunk at `[259,21]` is plain woodland, **not** dense: a north weight of 2.30769 is ten squares of
perimeter over thirteen rows, times **three**, and the same numbers times four appear nowhere in its 576 squares. Which
sides count as owning a biome is also settled: open ground and buildings do not, so their weight is the **undivided**
ratio - treating a field as a biome owner made a north weight of thirty into three, and the trace said so immediately.

**What it exposed, and it is a real gap.** With plain woodland the chain reproduces **288 of 576** squares of that edge
chunk. The first disagreement is **(0,12)**: the reference leaves its south weight at **0.25** and its own weight at **3**,
while the port divides the south weight to **0.0769** - which is exactly the arithmetic of the corner rule's **second**
branch. The three names that branch asks for are all present there (the south side is this world's woodland, the
south-west corner and the west side are the denser woodland, and the square's own biome is plain woodland), so either the
branch's guard is not what the port reproduces, or the corner slot passed to it is not the one the reference passes.

The second branch is also the one place I deliberately copied an arithmetic oddity from the upstream source, so this is
precisely the rule that needed a traced world rather than a reading. It stays recorded as an **open gap** with the square
and both numbers named, rather than being papered over by loosening the comparison.

## The forest's own edge, where corner blending runs (round 246)

The corner rule had only ever been checked against the reference's source. A plain generated region settles that: of its
fourteen chunks, **six** cannot be explained by any of the sixteen side masks - `[152,120]`, `[153,119]`, `[153,121]`,
`[155,121]`, `[155,122]`, `[156,121]` - because corner blending moved weight between sides where a corner's biome differs
from the square's. They are recorded whole in `corpus/overmap/reference-forest-edges.json`: each chunk's ninety-six
perimeter depths and all 576 squares' four neighbour weights.

What that proves and what it does not: it proves the rule **runs in a real world** and that a port without it cannot
reproduce these six chunks. It does not yet prove the port reproduces them *with* it - that needs the corner biomes, which
the trace does not record yet, and is the next piece of instrumentation.

**A harness defect found on the way, worth recording:** the perimeter trace is only trustworthy in a run that does not
replay turns. Sixty turns of walking produced perimeter lines whose values turn into heap addresses after a dozen
entries (`17,602,328 42,949,676 ...`), which reads exactly like an array being printed past the part the generator filled.
The weights beside them are fine. Until the writer is understood, edge chunks are taken from plain runs and their weights
are the trustworthy half of the record.

## Thirty-five chunks, and the mask read back out of each (round 245)

The mask that says which of a chunk's four sides owns a biome is the same for all 576 squares of that chunk, so it can be
read back out of the recorded weights: only one of sixteen reproduces them. Deriving it for **every chunk of a whole
generated region** - thirty-five chunks, twenty thousand traced squares - produced **nine distinct masks**, with
seventeen chunks wooded on all four sides and the rest split across eight other combinations. That spread matters: a
derivation that always returned the same answer would pass while proving only that it agreed with itself.

Every one of the thirty-five has **exactly one** explanation, and the neighbour weights of all 20,160 squares follow from
the perimeter plus that mask. This is also the first gate that runs the port's `nesw_weights` across a whole region rather
than a single chunk.

**What it does not cover, stated plainly:** all thirty-five chunks fit the weights **before** corner blending, which means
`unify_all_borders` never fired anywhere in this region - its corners are all absent or already matching. The corner rule
is still only tested against the reference's *source*, not against a traced world. Generating a forest **edge** would fix
that, and the trace is already in place to catch it.

**One expectation corrected by the data:** I expected at least one chunk with no woodland beside it at all. There is
none - every chunk in this region touches woodland on at least one side, which is what a chunk generated inside a forest
should show.

## The arithmetic agrees, 576 squares at a time (round 244)

Feeding the recorded weights back through the port's own functions settles two things that reading the code could not.

**The empty ticket.** Its weight is the chunk's total scaled by the maximum density minus what the neighbours and this
square contribute at their own densities. Those densities belong to the chunk's four neighbours and are therefore the
same for all 576 squares - so the chunk is 576 equations in four unknowns. Exactly **one** set of densities satisfies
every one of them: **[4, 4, 4, 0]** - dense woods to the north, east and south, and **nothing to the west**, the neighbour
that owns no biome at all.

**The four neighbour weights.** They come from the perimeter, the square's distance from each edge, and which sides own a
biome. The last is unknown, so it was inferred the same way: of sixteen possible masks, exactly one reproduces all 576
recorded weights - the same three woods and no biome to the west. The two independent fits agreeing on the same
neighbour set is the strongest evidence this port has produced about the forest rules, and it is the first that involves
**no dice whatsoever.**

## The weights themselves, square by square (round 244)

The feature raffle now writes **six weights per square** beside the ticket it drew, so `reference-forest-pools-trace.txt`
holds 576 lines of `x y ticket north east south west self empty`. That changes what a comparison can mean: until now the
port could only be scored on how often two independent draws agreed, because the pool itself was never on disk. A weight
trace can be compared with **no dice involved at all.**

Three things the trace pinned that I would have guessed wrong: the empty ticket takes **306** of the 576 squares and the
square's own biome **118** - a share of the local biome I would have put lower - every weight in every pool is
**non-negative**, and every recorded ticket is one that **carries weight**, which is what walking a pool by accumulated
weight has to produce.

**A harness defect worth recording, because it wasted a round:** the overmap coordinate written beside a chunk comes from
`m->get_abs_sub()` during generation, and for a chunk other than the player's own that coordinate is **not** the chunk's
own. Dumping the "traced" square `[155,338]` returned `lake_surface` and 576 tiles of deep water. The reliable pairing is
still the chunk the player stands in - here `forest_thick` at `[60,257]`, whose coordinate the dump and the perimeter line
agree on. Dumping an arbitrary square works through a scratch map (`map::load`), which is now what `--dump-map-omt` does;
what is missing is a *trustworthy* coordinate in the trace, not the ability to dump.

## One chunk, described on both sides (round 243)

The dump and the traces now describe the **same** chunk: `corpus/maps/reference-chunk-6049.json` is the thick woodland at
overmap `[60,49]`, and the files beside it hold that chunk's own **ninety-six perimeter depths** and its **576 feature
tickets**, tied together by the coordinate written at the start of the perimeter line. That is the piece the comparison
has been missing since round 224 - before it, the port was scored against a dump taken at the player's square while the
traces described whichever forest chunk happened to be generated nearby, so **every score was the share of squares where
two independent draws agreed.**

The chunk is `forest_thick` rather than plain forest, which is a small bonus rather than a complication: it is the denser
biome, its curve reaches further (depths up to twelve rather than ten), and its west neighbour is the likeliest of the
four at **84 of 576** draws - what a square whose west side is more woodland than field should draw.

**A harness limit worth recording:** `--dump-map-omt` exists and moves the reality bubble, but `place_player_overmap`
lands the player on a **nearby suitable square** rather than the one asked for - the first attempt asked for `[60,48]`
and got `[57,45]`, an apple orchard. So the reliable route is the chunk the player is already standing in, which here is
the traced one. Moving the bubble exactly would need a different call than the one the game provides.

## A chunk's recorded inputs (round 241/242)

A fresh world, generated with all three traces on, yields **one** chunk described end to end: its **96 perimeter
depths** (`corpus/overmap/reference-chunk-perimeter.txt`, values between four and ten, the north edge starting at nine
and settling at five), its **576 feature tickets** and its **573 ground tickets**. Together they are the inputs the port
needs in order to be checked against the reference **given identical inputs** rather than being scored on two
independent draws happening to agree - which, as the last round established, is what every score since round 224 has
actually measured.

**Two corrections came out of reading the files, both of them mine.** The ground raffle is called **573** times against
**461** empty feature tickets, because a ticket that lands on a neighbour owning no biome also falls through to the
ground - 112 squares reached it through a road or a field rather than through an empty ticket; my first version expected
the two counts to agree. And the fresh run's dump is a **different chunk** from the one it traced: the dump follows the
player, who stands in open field at `[192,333]`, while the traced chunk is a forest generated nearby. Comparing them
directly would compare two unrelated squares, so the dump needs to name its own overmap coordinate instead.

## The perimeter trace needs a regenerated chunk (round 240)

The perimeter depths are now written too (`CDDA_MAP_TRACE_PERIMETER`, one line of ninety-six depths per chunk), which is
the input the port needs to be checked against the reference **given identical inputs** rather than on coincidence. The
instrument compiles and is in the patch.

**It cannot be read from a chunk the world already holds.** A run whose save already contains the chunk does not
regenerate it, so the mapgen functions never execute and no trace is written - which is exactly what happened when the
perimeter trace was first tried: the game exited cleanly, wrote its dumps, and logged nothing, because the chunk had been
built by an earlier run. Reading it needs one generation with an empty save, which is the expensive case that stalled
three rounds ago. So the instrument waits on a run that is worth planning rather than starting on a whim.

## The missing-draw hypothesis, falsified in one read (round 240)

The obvious candidate for the dice being out of step was `set_terrain_dependent_furniture`, which rolls `one_in` per
square. It does not apply: **`forest`'s `terrain_furniture` block is empty** in this data, so the reference returns from
that function before rolling anything at all. Nothing is consumed, nothing is missing, and the hypothesis is dead.

**The real reason the chunk cannot match, and it is architectural rather than arithmetic.** The reference's perimeter
curve is built from **dice**, not from a hash of the chunk: the number of critical points per edge and their positions
and depths are all drawn from the engine's stream when the chunk is generated. The port's chunk test builds that curve
from a made-up seed (`Rolls::new(31337)`), which means the port has never been generating the same chunk - it has been
generating *a* chunk with the same rules. Every score since round 224 has been the share of squares where two different
draws happened to agree, and that is why the numbers sat between 280 and 320 while the rules got better.

**What that changes.** Exact chunk parity needs the same dice, which needs either a full replay of the world's generation
order - everything generated before this chunk consumed state - or a **recorded starting state for the chunk**. The second
is the oracle-first answer and it is small: dump the engine state (or seed) immediately before the chunk is generated,
along with the **96 perimeter depths** it computed, and the port can then be checked against the reference **given
identical inputs** instead of being scored on a coincidence.

## The corner, answered (round 239)

The ground raffle is instrumented too (`CDDA_MAP_TRACE_GROUND`), and it answers the question the last four rounds were
arguing about. `corpus/overmap/reference-forest-ground-trace.txt` holds **1,138 lines** - the ground raffle only runs
where the feature raffle left a square without a feature, which is one square in sixteen - across the **two** chunks in
this world that are generated as forest.

There are **two corners** in that file. The first chunk's corner draws **plain ground**; the measured chunk's corner draws
its **west neighbour**. West is woodland, so the corner takes the **west neighbour's own palette** - the forest palette,
which prefers forest floor ten to one - and that is precisely why the reference's chunk has forest floor at `(0,0)` while
the port, drawing plain ground there, has grass.

So the port's weights are not the disagreement: its west ticket is worth about three against the empty ticket's fifteen,
and the reference drew a one-in-six outcome. **The disagreement is the dice.** Something upstream consumes a different
number of draws, and the strongest candidate is visible in the reference's own loop: after setting the terrain of every
square it calls `set_terrain_dependent_furniture`, which rolls `one_in(tdf.chance)` - **one draw per square that the port
does not make at all.** A single missing draw per square would put every later square on different dice while leaving the
kinds and their shares close, which is exactly the shape this comparison has had since round 224.

## The map-chunk trace, taken (round 238)

**The blocker from the last round is gone, and it was my invocation, not the world.** The harness only enters its dump
block when `--dump-frame` is present; every failed attempt passed `--dump-map` alone, so the binary fell through to the
normal game path and sat on the loading screen. With the frame dump restored, the same command finishes in under a
minute and writes both dumps **and** the trace.

`corpus/overmap/reference-forest-trace.txt` holds the result: **17,856 lines, 31 chunks**, `x y ticket` per square. The
tickets are the reference's six choices - `0..3` the compass neighbours, `-1` this square's own biome, `-2` plain
regional ground. The first chunk is **400 plain ground, 83 own biome, 93 neighbours**, and **the corner draws `-2`** -
which is exactly what the port's own arithmetic says it should draw there. So the corner's disagreement is **not in the
feature raffle**: both sides draw the same ticket, and the difference is downstream, in what that ticket turns into.

One structural fact came out of reading the file: **the generator visits columns, not rows** - it loops `x` outside and
`y` inside - so the trace is column-major while a map dump is row-major. Comparing the two without knowing that would
have produced a transposed comparison and a wrong conclusion about the corner.

## Blocker on the map-chunk trace (round 237, resolved)

The oracle can record which ticket its forest generator drew for every square it generates: `CDDA_MAP_TRACE=<path>`
appends `x y ticket` after the pick, so a traced run and an untraced one produce the same world. That is what the last
unexplained square needs - it turns "what did the reference decide at `(0,0)`" from an argument into a lookup.

**It cannot be read at the moment, and the reason is a mistake of mine.** The harness world save
(`oracle/userdir/save/harness-fixed`) was deleted while forcing a fresh world, and no run since has reached map
generation inside a window that can be supervised: four attempts sat on the loading screen past five minutes, writing
neither a trace nor a dump, and each was killed rather than left running. There are no oracle processes on this machine
now.

Mitigation, in order of cost:

1. **Rebuild the saved world** with a harness action that generates and saves a world **without** dumping a chunk - it
   is the dump that needs a map, not the save. Every later run then reuses it, as the earlier ones did at ~40 s each.
2. Let the mapgen dump accept a smaller unit than a full chunk.
3. Leave the corner recorded as unexplained and return to it when the world can be rebuilt.

The instrument itself is not in doubt: it compiles, it is in `oracle/patches/0001-prototype-digest.patch`, and a traced
run of a world that exists will write the file.


## The dice a group draw asks for (round 586)

An item group draw looks like arithmetic and is really a **contract about dice**: a replay against the reference is a list of `(range, answer)` pairs, so a port that
asks `rng( 1, 100 )` where the reference asks `rng( 0, 99 )` consumes none of the reference's dice and drifts silently from the first entry onwards.

| Group | Reference's question |
|---|---|
collection | one `rng( 0, 99 )` per entry; **skip** when the answer is at least the entry's chance |
distribution | one `rng( 0, sum_prob - 1 )` for the whole group, then subtract each entry's raw weight and take the first that goes below zero |

Two loader rules travel with it, both from `add_entry`: an entry whose weight is not positive is **not in the group at all**, and a collection caps its chances at a hundred.

The **count** has the same shape and cost the port a die: `cnt = ( first == second ) ? first : rng( first, second )` — a range of one value is not a range, so no die is
asked, and `for( ; cnt > 0; cnt-- )` means a count of zero makes nothing rather than being floored to one.

The holiday rule is the subtle one. An event-based entry carries two numbers — its raw weight, which counts towards the distribution's total, and its chance once the holiday
gate has been applied, which is zero outside its holiday. The reference subtracts the weight first and tests the holiday second, so a holiday item the die chose is **passed
over** and the walk continues. Removing that skip fails the gate.

## The frame draws what it remembers (round 585)

`map::draw` has one branch that matters more than it looks: a square whose `lit_level` is `DARK`, `BLANK` or `MEMORIZED` becomes a `visibility_type` of `HIDDEN` or
`DARK`, and the loop calls `draw_background` on it, which prints the square's **memorized symbol** in brown and a blank space when nothing was memorized
(`src/map.cpp:6906-6917`). The memory is filled by the same loop, inside `draw_maptile` (`:7216`): a square is remembered **because it was drawn**.

Both halves are ported, and the two-branch shape is the rule:

| Square | Port |
|---|---|
seen and lit | prints the terrain's glyph, and memorizes the glyph on the square |
not seen, or seen and unlit | prints the memorized glyph; a space when nothing is remembered |

`FrameCell` carries `from_memory` so a comparison can tell the two apart — a frame that printed the right glyph for the wrong reason is exactly the bug this row exists to
catch, and the flag is what makes "the square is drawn from memory" an assertion rather than an inference. Removing the memory branch fails the gate.

The frame also had to be drawn **in daylight** to be comparable: the reference's frame was recorded outdoors, its `light` column reading the sky on every open square. That
gate plants the recorded value per square, because a level built from a forty-one-square window has no neighbours for the sun's cascade to walk in from, and the sun is gated
separately.

## The cast, the level and the join (round 584)

Vision had three claims bundled in one row and only two of them were checked; this round separated them, because they fail for different reasons.

| Claim | Gate | Result |
|---|---|---|
the cast is right | `build_seen_cache` over the recording's own transparency and floor bits | all three levels, exact |
the level hands the cast the right table | `Level::seen_grid_of` against the same recording | the character's level, 289 squares |
sight plus light is what is shown | `vision::shown_at` with the brightest-quadrant rule | seven transcribed cases |

The middle one is new and it is the one that catches a port-level mistake: the first gate would pass with a `Level` that ignored its transparency map entirely, because it never
asks a `Level`. Feeding the engine an **empty** map instead fails 163 of the 289 squares, which is what makes the comparison worth having.

What is still missing is the draw path's *memory* half: a square the character cannot see but remembers is drawn from `map_memory` and dimmed. The tile store landed in round
582; the loop that reads it while drawing is the next slice of this row.

## Falling in the turn (round 583)

`game::do_turn` (`src/do_turn.cpp:642-655`) orders the environment like this: the scent deposits and diffuses, the floor caches are built, **then** `process_falling`, and only then
`process_fields`. The order is not cosmetic — a thing that falls is aged by the same turn's field sweep rather than the next one's — and neither is the queue's shape:
`process_falling` moves `support_cache_dirty` out and clears it, so a square whose support went away is asked **once**, and a square that keeps falling is re-queued by
whatever breaks under it next.

The port now does the same, on a [`SupportLost`] resource that a caller queues squares onto, with the field rules read from a resource so that a world which loaded its
prototypes accelerates its cosmetic fields and a world that did not moves items only.

Three tests, and the shape of them is the point: the queue being **empty afterwards** is as much of the rule as the item moving, so the second turn's "nothing falls twice" is
asserted rather than assumed. Removing the step from the turn fails all three, which is the only evidence a gate like this carries.

## What the character remembers (round 582)

`memorized_tile` is small and the rules are all in one place, which is why this row was worth finishing rather than deferring: terrain **and** decoration, each with a subtile
and a rotation, plus the glyph, and four writes that each preserve the rest of the tile because the reference reads it out, changes one field and writes it back.

The clear is where the rules are. `clear_tile_decoration` takes a **prefix**:

| Case | Result |
|---|---|
| prefix matches | the decoration goes, **and the symbol with it** — the glyph belonged to the decoration |
| prefix does not match | nothing changes at all |
| prefix is empty | matches **every** id, which is how a region is forgotten at once |

The gate is a scripted sequence with all three in it, five squares compared one for one, and the non-vacuity asserts are written before the comparison: five squares written,
the cleared square holding an empty decoration and a zero symbol, the unmatched square holding its decoration and its glyph. Removing the symbol clear from the rule fails the
gate, which is the only evidence that a gate of this shape is worth anything.

## How warm a square is (round 581)

`get_temperature` is four lines and a cache: below street level a square answers the weather generator's year-round `base_temperature` and nothing else; above it, the weather's
current temperature plus three terms that come from three different kinds of data.

Recording it took two fixture rewrites, and both were the fixture's fault:

| What I tried | What the recording showed |
|---|---|
a fire as a heat source | heat that rose and fell by no rule of distance — a level-three fire has a translucency of 0.1, and `calc_transp` **multiplies**, so the fire was visible from one square and not from the next |
a cold gas to exercise convection | the cold-air square saw nothing at all, because `fd_cold_air3` has a translucency of zero: the square it sits on is solid to light |

The second one is the interesting one, because it looks like a bug in the rule and is a rule: a line of sight starts at a square, and a square that is solid to light starts no
line. The gate pins that translucency to the data before it relies on it, which is the difference between modelling the fixture and modelling the rule.

## What falls (round 580)

`map::process_falling` is a queue of squares whose support was destroyed, and `drop_everything` is four rules in a row on each of them: creature, furniture, items, vehicle,
fields. Two of the four do not need creatures or a vehicle, and they are the two that are here.

The **item** rule is a loop, not a subtraction: a rock above a three-storey shaft lands at the bottom, so the columns in the fixture fall one level and three, and a gate that
passed the first would fail the second. The **field** rule is one level and only for types whose data says the field is cosmetic — the reference's own comment is that an
active field "drops itself" through its own processor. And a field that lands on a hole is destroyed rather than held in mid-air, because `map::add_field` will not place a
non-gaseous field on a square with no floor. That last rule is why blood, which falls, never appears one level down: it is a liquid.

Two recording lessons, both of which cost a re-record:

| Lesson | How it showed |
|---|---|
clear the *whole* column, not just the terrain | the first fixture kept mapgen's locker and junk items, so it recorded a map with a hole punched in it |
a dead field entry is not a field | `remove_field` zeroes an intensity and leaves the slot, so the smoke column read as "three moved and one stayed" |

The furniture half is **recorded and not ported**: the bench falls two levels, bashes the floor it lands on, and the bash raises dust. Bashing a floor into fields is the bash
port's own declared gap, and the fall gate asserts the recording as a number instead of pretending the column is empty.

## Sealing a blast (round 579)

The blast's first gate claimed the set of squares and left *which square gets which force* open in 28 places. The cause was not the queue, which was already exact, and not the
falloff: it was the **fixture**.

A blast climbs. It bashes the square above the one it is on, and in a room open to the sky the wavefront then travels over the wall and comes back down outside it — so the level
the dump was recording was reachable only from a model of three levels, and the port had one. Sealing the squares above and below with the same unbreakable bulkhead as the walls
took the count from 28 to **zero**, and the recording from 812 squares to 812 squares plus 578 pops on the other two levels.

Three fixture lessons, in the order they cost rounds:

| Lesson | How it showed |
|---|---|
a wall the blast can break is not a wall | it bashes through, and the recording becomes a function of the bash rule as well as the blast rule |
a floor the blast can break is not a floor | `t_floor` is destroyed into open air, which is then impassable in places the model cannot predict |
a sealed room is not sealed in three dimensions | 22 of 360 squares carried the wrong force, and only a recorded **pop order** made that visible |

The pop order is now part of the dump. It is the gate that separates "the queue popped the wrong square" from "the bash hit it with the wrong force", and it is the reason the
blast row closed at all.

## Blasts (round 578)

The first P2 row that needed a **queue** rather than a rule, and the round where the port had to care which standard library the reference is built against.

`do_blast` orders its wavefront with `std::priority_queue< std::pair<float, tripoint>, …, pair_greater_cmp_first >` whose comparator compares the distance and **nothing else**.
Two squares the same distance away therefore come out in whatever order the binary heap holds them, which is different in libstdc++ (right child on a tie) and libc++ (left) —
and the oracle is built with clang and libc++, so the port's queue is a transcription of **libc++'s** `__sift_up` and `__floyd_sift_down`, gated on a recording of the reference
draining a fixed list. A sorted container answers the same set and a different order, and the order decides which square bashes a neighbour first.

What the gate claims, and what it does not:

| Claim | State |
|---|---|
| the squares a blast reaches | exact — 812 of 812 |
| the forces it hits with, as a multiset | exact |
| which square gets which force | **open** — 28 of the 812 carry the other one's force, and the count is pinned so it can only go down |

The residue is the same tie, one level down: equal-distance squares pop in an order that depends on the heap's **insertion history**, and the port's history still differs
somewhere the recording does not show.

## Roofs, and the square below (round 577)

Bashing a floor exposed a gap one layer down: the reference's `get_roof` answers from the **terrain's own `roof` field**, which the port was not loading. Closing it needed a
corpus change, because the prototype dump did not print the field either.

The field has two spellings of "nothing" and only one of them is nothing:

| Value | Means |
|---|---|
| `t_null` (534 terrains) | no roof — the reference's null terrain id, and what `!roof` tests |
| a real id (1,109 − 534 terrains) | the tile below becomes this when it needs a roof |
| `""` (all 660 furniture) | the field does not exist on furniture at all |

With it, the six rows of the bash gate that end as a roof are ordinary comparisons rather than a declared gap: a destroyed floor becomes `t_open_air`, and then the roof of the
square below is put back over it (`src/map.cpp:4229-4236`).

Two lessons worth the round: **`--seed` is not optional** for a dump that touches the world — the first recording read `t_rock_floor_no_roof` under two fixtures because mapgen
had put something else there — and a dump should record the fixture it *set up* as well as the result, which is why `below_before` exists and the gate asserts the fixture held.

## Bashing (round 576)

The first behaviour that *uses* the bash data, and the first oracle mode that needed a hook rather than a dump: `map::bash` takes its roll from the global rng (`map.cpp:4245`),
so a recording would depend on how many draws came before it. `harness_bash` runs `bash`'s own sequence with the params handed in, and the port takes the roll as a parameter —
which is also what makes the port's answers comparable across runs.

97 hits over eight fixtures. Three rules the gate pins, all of which a transcription would plausibly get wrong:

| Rule | What it means |
|---|---|
furniture **hides** the terrain | a chair on a concrete floor is a chair: its block is used and the floor is not touched until the chair is gone |
a failed hit leaves **damage**, at least one point | `max( (strength - str_min) * roll, 1 )` is added to the square, and the next hit reads it as strength — so the same hit three times is not three independent answers |
`success` is **not** "something broke" | under `destroy` the strength check is skipped, so the square is destroyed and `success` stays false; that is the reference's `params.success \|= success` |

Six of the 97 rows are declared out: a destroyed floor with `ter_set` of `t_null` becomes `t_open_air` and then the reference regenerates a roof over it from the terrain below
(`get_roof`, `map.cpp:4230-4236`). The count is asserted so that a fixture change moves it rather than slipping past.

## How hard everything is to break (round 575)

The bash data is the first block whose *defaults* carry more information than its values. 958 terrains and 612 pieces of furniture carry one, and the gate compares all
1,570 field by field.

Three defaults the dump settled, each of which a transcription would have got wrong:

| Default | What it costs to miss |
|---|---|
`ter_set_bashed_from_above` defaults to `ter_set` **only when the block is fresh** | a carpet that renames `ter_set` keeps its parent's above-result (200+ blocks), so it still bashes from above into `t_floor` rather than into its own base — 533 blocks differ this way |
furniture's `furn_set` defaults to the real id `f_null` | an empty string is not a furniture, and every furniture bash would set the square to nothing at all |
a block with no `items` gets the real group `EMPTY_GROUP` | "no items" and "the empty group" are the same behaviour but not the same data, and only one of them survives inheritance |

Two absences are declared rather than guessed: inline `items` arrays (the reference mints anonymous group ids for them, which no port can reproduce) and the half of the
block the bash algorithm does not need yet — `explosive`, `tent_centers`, `hit_field`, `destroyed_field` and the two sound texts.

## The cached question, which skips (round 574)

A zone that is switched off is still a zone: the manager's four list queries return it, and on nine squares of the fixture it is the answer to "what am I standing in". It is
**not** in the point set that `has` and `has_near` read, because `cache_data` skips it (`src/clzones.cpp:897-900`). So the same square answers *yes* to the first question and
*no* to the second, and on five of those squares it is the only zone there.

That is the kind of divergence a single-list port cannot reproduce: one list answers both questions the same way, and every gate that only asked one of them would pass. The
fixture asks both over the same 240 squares and compares 2,640 answers.

Two smaller things the slice settled:

- `has_near` measures `square_dist`, the **larger** of the two axis distances, so a range of one covers the eight neighbours and a diagonal counts as far as a straight line.
- The port builds the point set **on demand** where the reference caches it. That is a declared divergence in the opposite direction from the usual one: the reference can
  answer from a set that is a turn out of date between mutations, while the port always answers from the current zones — which is what the reference's next `cache_data`
  produces.

## The zone manager, from both ends (round 573)

The second slice of zones: the queries. The manager holds its zones in one list, and the reference reaches into that list from **both ends**:

| Query | Direction | Answer |
|---|---|---|
`get_zones_at` / `get_zone_at` | forwards | the **oldest** zone that contains the square |
`get_bottom_zone` | backwards | the **newest** zone of any type |
`get_zone_at( loot_only )` | backwards | the newest zone whose type **id starts with `LOOT`** |

So a player who draws a second food zone over the first keeps being answered by the first, and a square can be "in" one zone for one question and another for the next. The
fixture in `corpus/zones/probe.json` overlaps on purpose — seven zones of the owner's faction, one of another, one inverted, one switched off, one that is not a loot zone at
all — and the gate compares 1,440 answers over 240 squares. Nine of those squares have a different `bottom` zone from their loot-only answer, which is the only thing that
makes the loot filter observable.

Two rules the fixture pinned that a transcription would have got wrong in a way nothing else would catch:

- `has_inside` **ignores `invert`**: the rectangle is the rectangle, and the inversion belongs to whoever reads the zones. Folding the flag into containment answers "outside"
  for every square inside, which flips a whole family of answers at once.
- `has_inside` **ignores `enabled`**: a switched-off zone still contains its squares, and the four queries still return it. The flag stops actions from *using* the zone, not
  the manager from *finding* it.

## Zone types (round 572)

The first slice of zones: the registry. `--dump-zone-types` walks `zone_type::get_all()` and writes the six fields the loader reads, and the port reproduces all 64 — id,
name, description, display field, `can_be_personal` and `hidden`.

The one thing the dump bought that a transcription would not: the JSON tag. Zone types are written `"type": "LOOT_ZONE"` (`src/init.cpp:370`), so a loader that looks for
`"zone_type"` — the name of the class, the header and the field — returns an empty registry and reports no error at all. The gate compares the count first for exactly that
reason.

## The ledger's tables, put back in order (round 572)

The phase tables had drifted into three copies of one another. `P1`'s table carried the whole of `P2` and half of `P3`; `P2`'s carried `P3`; and the stale originals were
still in place beside the rows that had superseded them — so `P3` said "Item prototypes: not started" four lines above the row that says `Constructions` are gated end to
end, and `P2` had a `Traps` row claiming `not started` while `P1` had the receipt.

Every row now sits in **the phase its own Gate column names** (`M1` and `P1` in `P1`, `M2` and `P2` in `P2`, `M3a` and `P3` in `P3`), one row per system. The rule for
merging a duplicate was **"the most-informed version wins"**, not "the last one written": the stale table came later in the file, so taking the last would have kept the
rows that said finished systems were never started. Six rows were dropped outright because a row under another name already holds their receipt (`Submap ownership`,
`Light as a system`, `Vision (\`seen_cache\`)`, `Field processing`, `Sound`, `Map memory`).

The stale rows that remain are the ones that are actually true: the six `P1` rows are the save and mod surface, and three `P2` rows are `Scent`, `Zones` and falling with
explosions.

## The sun, stage by stage (round 571)

The astronomy was gated end to end, at a tolerance of `1.4e-3` degrees, and that is exactly the tolerance that hides a wrong constant: a curve can be a thousandth of a
degree out all day and be minutes out at a dawn the light levels are decided by. It is now gated **stage by stage** against the five intermediates the reference prints —
right ascension, declination, sidereal time, hour angle, days since the epoch — and every stage lands within a hundredth of an arc second, with the light exact to `1e-3`.

Getting there needed two rules the end-to-end gate could not see:

| Rule | What it costs to miss |
|---|---|
the year length is **truncated to whole days** | `to_days<int>( year_length )` turns a four-day year into ninety degrees of sky a day; the port had been taking the fraction |
the timezone is **whole turns** | `angle_to_time` is a `time_duration`, so seventy-one degrees west is 17054 turns and not 17054.4 — three tenths of a second, which is a hundredth of an arc minute of right ascension |

The light curve is now single-authority as well: the astronomy asks `light::sun_light_from_altitude`, which the light system was already gated on, rather than carrying a
second copy of the same piecewise function.

## Smoke in the line of sight (round 570)

The last red gate the environment gate had left open is closed: the frame's transparency table was built from the terrain alone, so a square behind smoke was drawn as plainly
as an open one.

Two things were wrong, and the second was worse than the first:

- the table never asked the **furniture** layer or the **fields** at all;
- the field half was combined with `min`, which reads backwards. The reference **multiplies** (`calc_transp`, `lightmap.cpp:155-165`): open air is about `0.032` and smoke
  at its second intensity is `10`, so a smoky room is ten times as absorbing, and smoke at its third intensity is `0`, which ends the line. Taking the minimum of an
  absorbing number and air keeps the air, which is why mid-intensity smoke did nothing while dense smoke looked right by accident.

The same `min` was in the `sees` query, so both readers now multiply through one grid. Two gates pin the behaviour: a square four past the smoke is seen *less* than the
same square without it, and behind dense smoke it is not seen at all.

## One pile cannot serve two groups (round 569)

The build check was answering group by group, and a kit can satisfy every group on its own and still be one welding torch short. The rule that catches this is
`check_enough_materials`, and it comes in three parts, each now pinned by a transcribed case rather than by a corpus row:

| Overlap | Rule |
|---|---|
component against **tool** | one welder wanted as a component and held as a tool needs **two** welders — the one you consume cannot be the one you hold |
component against **quality** | a pipe is a hammer at level one, so a build that consumes a pipe and needs a hammer needs two pipes: the one in the recipe is not on the bench when the hammering starts |
**size** | a charge count becomes items through the type's `stack_size`, and the components of one craft may not add up past `MAX_ITEM_VOLUME`, which is one thousand litres |

The corpus gate did not need the rule — the seven kits hold enough surplus that no answer flips — which is exactly why the cases are transcribed from the reference's own
function instead of hunted for in the dump. The oracle's dump also grew the item volumes the rule needs, and the port grew `itype::stack_size`, which nothing had asked for
until now.

## Can it be built (round 568)

The construction menu's question now has an answer: given a kit, which constructions can be built. Five thousand two hundred and eighty-five answers across **seven** kits
agree with the reference — an empty one, tools only, materials only, tools that spend charges, everything at once, one that only a charge count can refuse, and the two
together. The kits are what makes the gate worth anything: a construction that answers the same way everywhere proves nothing, and the reference's own answers spread
across all eight combinations of kit and answer.

Four rules came out of the comparison:

| Rule | What it means |
|---|---|
a **positive** tool count is charges | `angle_grinder 200` is two hundred charges spent, and four hundred in the kit is enough — the reference's `charges_of` is asked with a limit and comes back clamped, so its `==` reads as "at least" |
a **negative** one is a tool to hold | `claw_bar -1` means any claw bar, whatever is left in it |
components follow the item's own counting | an item counted by charge is asked for in charges, anything else in items |
`item( id, turn, n )` makes **one** item | a kit of forty planks is forty items, and reading the forty as forty planks made sixty-eight constructions buildable that the reference refuses |

That last one was a bug in the **oracle's own script**, not in the port: the first recording said yes where the reference said no, and the kit probe (`amount_of`,
`charges_of`, `has_quality` written into the dump beside the answers) showed a workshop with one plank in it.

## What a hammer is worth (round 567)

Tool qualities are now loaded from both ends: the catalogue of 87 qualities with their names, and the 1,091 item types that are worth something as a tool, with the level each
one reaches. Every id, name and level matches the reference.

Two things about the levels are worth stating because both are easy to get wrong. **A level can be negative**, and that is not a sentinel: the reference writes a negative
level when a tool is worse at the job than bare hands, so a saw is `BUTCHER -90`. And **a gun is a quality**: every gun that is neither a gun mod nor a primitive ranged
weapon gets `GUN` at level one and the quality named after the skill it fires with — but only when that name is a quality the data defines, which is why a grenade launcher
gets `GUN` and no `LAUNCHER`.

The gate also caught two loader rules the port was missing:

- gun data is written **flat** — `"skill": "pistol"` sits beside `"ammo"` and `"dispersion"` on the item itself, not under a `gun` member;
- qualities are a **container**, so the merge operators reach them: a dutch oven adds `OVEN` to the qualities it inherits from a cooking pot, and a kettle `delete`s the
  `COOK` quality it does not deserve.

## What a construction takes (round 566)

A construction is not finished when its tiles and its time are known: it is finished when you know what it eats. Every one of the reference's 756 constructions now matches the
reference's own resolved requirement group — all 601 that consume something, all 211 that need a tool, all 487 that want a quality — with every alternative of every group at
the reference's count.

The merge is the interesting part, and every rule in it came from a failing comparison rather than from reading:

| Rule | What happened without it |
|---|---|
`"LIST"` marks a **nested group** | a component that named another requirement group stayed unexpanded |
only a flagged name is a group | `autoclave` is both an item and a group's id, and the item was replaced by a recipe's worth of water |
a nested group is scaled by its outer count, floored at `-1` | `[ "surface_heat", 2, "LIST" ]` asked for **two fires** |
only its **first** alternative-group is inlined | groups the reference drops came back |
inner lists are walked per list: tools against tools, components against components | `pickaxe_list` (a tools group) expanded to nothing |
two alternatives of one item keep the **smaller** count | a group asked for the same thing twice |
two quality groups that cover each other collapse | a construction asked for `PRY 3` twice |
a bare string is a tool of count `-1` | `[ "paint_brush" ]` vanished into an empty group |
tools expand into their **subtypes**, components do not | a fire was only a fire, never a lit torch — and a consumed `forge` became every variant of one |

The last one also found a **loader gap**: the port read `subtypes` but not its older spelling `sub`, which is how a lit torch says it is a fire.

## Seven hundred and fifty-six constructions (round 565)

The construction registry is loaded and gated end to end: every one of the reference's 756 constructions matches on id, group, category, time, the tiles it may be built on,
the tile it leaves and the skills it wants. The oracle writes the list (`--dump-constructions`) and the gate is an equality over all of them — the only kind of check that
catches a field silently read from the wrong member.

Three rules came from the data rather than from reading:

- **`pre_terrain` is a set.** Four painted-wall constructions came back in a different order than the JSON, because the reference holds them in a `std::set<std::string>` and
  sorts byte-wise, so `t_concrete_wall_P` comes before `t_concrete_wall_b`.
- **Skills have two shapes.** Seventy-one constructions have no `required_skills` at all but a legacy `skill`/`difficulty` pair, and the reference defaults to fabrication at
  zero when neither is present (`src/construction.cpp:2203-2211`).
- **Time is a duration string in moves.** `"1 h 30 m"` is 540,000 moves, because one turn is one second and one second is a hundred moves. A bare number is already in moves.

## Heavy, and not big (round 564)

The pocket session learned a second limit. A manhole cover is 20,000 ml — comfortably inside the backpack's 25,000 ml pocket — and 115 kg, and the reference refuses it. So
`can_contain` asks about weight as well as room, and it asks **per item**: `it.weight() > weight_capacity()`, not a running total of what is already inside.

`ItemFacts` carries the weight now, `Pocket` carries the pocket's own capacity from `pockets.json`, and the refusal is `TooHeavy { capacity_grams, wanted_grams }` where the
room refusal is `TooBig`. Both heavy payloads in the recording are refused by both sides.

One honest limit on what this proves: the reference's message for a refusal like this is *"item is too heavy, item is too long, or item is too short"* — three limits in one
sentence, so the gate can see **that** it refused and not **why**. The port models weight and room; the length and shortness limits stay declared, and the ledger says so.

## A pocket is its own limit (round 563)

The oracle answered a question I had been guessing at. Two steps added to the pocket script — a 30,000 ml branch and a 500 ml crystal — showed that the reference **refuses**
the branch in a backpack whose pockets hold 25,000 + 510 + 5,000 + 5,000 ml. So pockets are not a pool: `item_contents::insert_item` offers the payload to each pocket in turn
and takes the first that accepts it, and a payload bigger than any single pocket is too big no matter how much room the container has in total.

That made the port's one-capacity model wrong, and wrong in the direction that hides: it accepted things the game refuses. `Container` is now a list of `Pocket`s, an item
records **which** pocket took it (`InPocket`), and the room a refusal names is the largest accepting pocket's room — 20,120 ml for the log, 25,000 for the branch on a fresh
backpack — and not the sum.

The session also had to learn to break where the recording breaks: the script returns to the backpack at the end, **empty**, and a replay that carried the first run's
contents forward would have been comparing against numbers from a different container.

## The phase rule, from the data (round 562)

The pocket session's declared gap is gone. The reference will not put a liquid in a backpack because none of its pockets is `watertight` (`src/item_pocket.cpp:1396-1401`);
`corpus/items/pockets.json` (`--dump-pockets`) now records every type's pockets — kind, capacity, `watertight`, `airtight`, `rigid` — and the port asks the same question of
the same data.

Two things became true at once. The session's **capacity is no longer hard-coded**: the backpack's four container pockets hold 25,000 + 510 + 5,000 + 5,000 ml, and that sum
is what the recorded room numbers add up to, so the test now checks the data against the session instead of asserting a number it chose itself. And the water step flipped
from *declared difference* to *reproduced refusal*: the port refuses it with `WrongPhase { Liquid }`.

The stale half of the old test survived one round and taught the lesson twice: a hard-coded constant that the outcome never depends on is worse than no constant, because
it looks like evidence.

## A session, not a table (round 561)

`corpus/items/pocket-session.json` is the first **pocket session**: a scripted sequence of `item::put_in` calls, recorded step by step with the container's weight, volume and
remaining room after each one. The difference from a dump of numbers is that it records the **decision and its consequence together** — a port can be wrong about either
alone and still match a table of one.

The backpack half replays exactly: three rocks at 657 g and 250 ml each, a plank at 2440 g and 4130 ml, then a log refused because 50,000 ml does not fit in the 30,630 ml
left. The first cut of the test hard-coded the capacity in a way the outcome never depended on — it passed with a capacity of 30,000 — and the log step is what fixed that:
the port now names the room and the want the reference implies, and a wrong capacity fails.

The second refusal is **declared**: the reference will not put a liquid in a backpack (`can_contain`'s phase rule) and the port will. The test asserts both answers rather than
skipping the step, because a gap that a gate refuses to look at is a gap nobody knows about.

## The fourth number (round 560)

`item::volume( integral: true )` is ported, and with it the last of the four questions the measures gate asks. The rule is the same one over the type's other volume, so the
charge division, the stack size and the round-up all still apply — a folded frame takes 500 ml where it stands at 4000, and six charges of a 1 ml stack-of-nine take one
millilitre packed down and two as they stand.

The data decided the loader's shape again: **120 types** take a different integral volume, and **97 of them say zero on purpose** (a pseudo magazine, a debug item, `null`).
The loader copies the ordinary volume only when the field is still unset — the same "zero is a value" rule the integral weight taught, now in its second place, which is why
it was written once and reused rather than re-derived.

## The dataset gate, and the bug it found (round 559)

`corpus/items/measures.json` is a new oracle dump: for each of the 9,912 item types the reference **constructs an item and asks it** what it weighs and what it takes up,
through `item::weight`, `weight( integral )`, `item::volume` and `volume( integral )`. The prototype dump next door says what the *data* is; this says what the *rules* make
of it, and the port replays all four numbers against it.

It found two things that no hand-written test would have.

**Weights have to be counted in milligrams.** Twenty-four charge-counted types weigh a fraction of a gram a charge — `aluminum_foil` is 135 mg, and two hundred of them
weigh 27 g. The prototype dump carried whole grams, so the port's unit weight was zero and the pile weighed nothing.

**Charges do not always multiply the weight.** A torch carries twenty charges and weighs one torch; a battery carries a hundred and weighs a hundred times one. The
reference multiplies inside `if( count_by_charges() )` (`src/item.cpp:7273-7275`), and the port multiplied unconditionally — which was invisible until a type with charges
that is *not* counted in charges walked through the gate. Both dumps now carry `count_by_charges`, and the rule asks for it.

Sixty-six types still differ, every one of them declared: eighteen corpses (the reference answers with the monster's size), forty-four guns and magazines holding mods or
ammo (the reference adds them to the gun's own weight and volume), and `null`. The test names each category and pins the totals, so a new disagreement cannot hide among
them.

## What a pile takes up (round 558)

`item::volume`'s charge rule is ported: `volume * charges / stack_size`, with the division **rounding up a millilitre**, and contents added on top for a container.

The corpus decided what the numbers mean. A type's volume is the volume of a **whole stack** — the battery is 100 ml for 100 charges — so one charge is one millilitre, and
a port that multiplied and stopped would take a hundred times the room at a hundred charges. The dump now carries `stack_size` and `count_by_charges` beside the volume, and
a gate pins the 1,261 charge-counted types.

The test that mattered is the one that does not divide evenly: four charges of a 1 ml stack-of-three take **two** millilitres, because the reference adds a millilitre when
the division has a remainder rather than truncating. That is the kind of branch that a plausible implementation gets wrong silently — truncation looks right until a pile
crosses a stack boundary.

## The second weight (round 557)

`item::weight( integral: true )` is ported. The rule is not a fraction of the ordinary weight: it is **another number in the type**, which is why the port carries two
(`TypeWeight { grams, integral_grams, no_drop }`) and a second entry point.

The corpus decided the shape of it. Only **two** types weigh a different non-zero integral weight — `robofac_gun_shotgun_breach` is 1000 g as made and 650 g damaged,
`retool_ar15_223rem_pistol` 1134 against 1819, so the rule can go either way — and **twenty-three gunmods say zero on purpose**, because a gunmod's weight does not count
toward the gun's whole weight.

That last group caught a loader bug in the first cut: `integral_weight` was defaulted to the item's `weight` per key, so an inherited `"integral_weight": "0 g"` was
overwritten and a stocked rifle would have weighed its stock twice. The reference copies `weight` only when the field is **still unset** (`-1`,
`src/item_factory.cpp:398-400`), and the loader now does the same — which is the difference between "zero is a value" and "zero is an absence".

## M2 is closed, and P3 opened with the `NO_DROP` shortcut (round 556)

Every clause of the M2 exit gate now has a replayed reference session and a receipt: weather transitions (`0x4c221d74263f52f4`), fire and gas spread (three receipts),
light across z-levels (175 of 175 squares), scent diffusion (`0xaac13754071041c0`) and vision (867 of 867 squares). The table is in `05-roadmap.md`; what is *not* in M2 stays
declared there — the processors that need monsters or items, and a weather that moves across an overmap.

P3's first slice is the smallest branch of `item::weight`: **a `NO_DROP` item weighs nothing, contents included**, because the reference returns zero before it looks at
charges or contents. The port now carries that as a fact about the type (`TypeWeight { grams, no_drop }`) rather than as a base weight of zero, and the difference is the
gate: a container of rock weighed **600** grams as an ordinary box and **100** with the flag folded into the base weight — the port's old answer — where the reference gives
**0**.

## The oracle built every level and read whichever was last (round 555)

The `lit` column was wrong, and the cause was the dump, not the game: it built the three levels **and then** read them, so every square was read from whichever level's
cache had been built last. A 160-lumen fire therefore came out as a dark room. The fix is to build each level and **read it immediately** — measured, not reasoned:

| Square | before the fix | after |
|---|---|---|
the fire's own square | `lit` 4, `raw` **4.0**, source 160 | `lit` 4, `brightest` **160.0**, source 160 |
one square away | `lit` 1, `raw` 4.0 | `lit` 3 (lit), `brightest` 155.5 |
the level below | `lit` 1, `raw` 3.5 | `lit` 1, `brightest` 3.5, source 0 |

Two more things the fix taught, both now in the dump's column names:

* **the column called `ambient` was `lm`'s maximum**, not the sky. It is now `brightest`, and the sky is a separate column (`natural_light_level` per level). The
  round-553 classifier gate had been reading that column as if it were the sky; it reads `sky` now, and its threshold is the honest one;
* **a level's light does not cross to another level** — the square below the fire stays at 3.5 with no source — which is what the reference's per-level `generate_lightmap`
  does, and what the port's per-level cast already matched.

With the fixture re-recorded and the replay comparing **answers as well as sources**, all 175 squares of the fire's level now agree: the port's cast and the port's
classifier together reproduce the reference's `lit` on every square of seven turns.

## The light replay, and the oracle bug it found (round 554)

The fire's level of `light-session.json` now replays: the port builds a `Level` from the record's terrain, places the record's fields, runs its own cast, and compares every
square — 175 of them, seven turns of twenty-five. The comparison it can make today is the **source**, the light a square emits itself, and that matches exactly.

**The `lit` column cannot be gated yet, because the dump is wrong.** The fire's own square is recorded as carrying a 160 source and reading **4.0** — the ambient
brightness of a dark room — so the recorded answer came from a lightmap build that did not include the fire's own light, even though the source column from the same build
does. The replay measured the same thing from the other side: the port's cast puts 142 to 154 on every square of the room, and the record says LOW everywhere.

That is an **oracle** defect, not a port one, and it is the first the instrument has had in a while. What it means in practice:

* the `source` column is trustworthy and is gated (exact match, all 175 squares);
* the `lit` comparison is written and waiting, so the fix is a one-line change on each side;
* the fixture's own gate in `light_session.rs` was written against the columns that are consistent (a square with its own source reads BRIGHT — which is a fact about the
  source, not about the cast) and the classifier's 700-square check reads `ambient`, which is a third column and agrees with itself.

**Next round starts with the oracle fix**: re-record the session and see whether `lit` and `source` agree on the fire's square. The suspicion is that `build_map_cache`
regenerates `lm` for a level whose `light_source_buffer` was filled by a *different* build, but that is a hypothesis and the recipe to test it is in the corpus README.

## The classifier before the cast (round 553)

`map::light_at` is now ported — four arms, in the reference's order, and it never answers `BRIGHT_ONLY` or `BLANK` because those belong to `apparent_light_at` where
obstruction, range and memory come in. Six threshold cases pin the boundaries (10.0 is bright, 9.99 is not; 3.5 is low, 3.49 is dark) and a sweep asserts the two
player-only values never appear for any input.

The classifier is also held against **700 measured squares** of `light-session.json`, which is the useful half: every square carrying its own source reads BRIGHT, no square
reads bright-only, and every DARK square's ambient is under the low threshold. Those are falsifiable by the fixture — a record with a dark square in daylight would fail
them — and they cost no cast.

The cast itself is the next slice: a per-turn lightmap over four levels, fed by the recorded ambient and the port's own field emitters, compared to `lit` square by square.

## Light on four levels, per turn — the last M2 clause gets a fixture (round 552)

The M2 exit gate asks for light changing across z-levels, and no session had ever carried any: every dump said what the fields were doing and nothing about what that
made visible. `--dump-field-series` now writes a **`light` block** per turn — 25 squares on each of four levels, each with `map::light_at` (0 dark to 4 bright), the
square's own light source, and its ambient.

Measured from `corpus/fields/light-session.json` (a fire at intensity three, seven turns):

| Where | `lit` | `source` | Reading |
|---|---|---|---|
the fire's own square | 4 (bright) | 160 | the fire is its own light |
the square beside it | ≥1 | 0 | rays reach it, dimmer than the fire |
the square **below** | 1 (low) | 0 | the floor is a ceiling from underneath — the rule the sight cast found, seen from the light's side |
above (the sky) | 4 | 0 | daylight comes down; the fire does not go down |

The fixture's own gate pins those numbers now, before any replay exists: the replay is the next slice, and pinning the answers first is what stops it from becoming a
comparison of the port with itself. One thing the dump taught: `lm` (the cache's quadrant array) is **not** what the game reacts to — `light_at` is — so the dump carries
that and the source beside it, and a replay is held to the observable.

## The last two processors that needed no subsystem (round 551)

`extra_radiation` and `wandering_field` were the end of the list: every other processor still waiting needs monsters, items, or the fungal and radiation grids to exist
first. Both are now ported, wired into the turn in the reference's order (radiation before wandering, `src/map_field.cpp:2223-2238`), and gated by six tests — five on the
processors themselves and one through a whole `process_field_turn`, because a processor that is correct and never called simulates nothing.

Two things the port had to be corrected on, both by a test rather than by reading:

* **The reach is a square, not a circle.** `points_in_radius` is a `tripoint_range` over `center ± (r, r, 0)`; the circle is the separate `points_in_radius_circ`. A vent
  at intensity two fills **nine** squares, and the first implementation filled five.
* **`add_field` sums and then clamps.** A square already as strong as the vent does not get skipped — it gets the vent's intensity added to it and the setter clamps the
  total to the type's level count, so smoke at two under a vent of two becomes three, not four. The expected four was the port being optimistic about the data, not about
  the rule.

Radiation landed as a store (`Radiation`) and a processor that draws **nothing** when the level's maximum is zero — the guard is on the maximum, so ordinary fog costs the
RNG stream nothing. Nothing reads the value yet: a character's dose needs a character, and that is declared in the ledger.

## The ledger was stale, and two processors got their data (round 550)

Two rows of the ledger had stopped being true, and both were the kind that mislead a reader planning the next slice: **Gas spread** still said "direction not
ported" though the wind roll, the starting neighbour, the clockwise walk and the vertical order had been ported and replayed die for die over three sessions (with
receipts); and **Sunlight curve** still said the sun's altitude "is not ported yet" while the Astronomy row two lines above it says it is gated on 424 samples of a
year. Both rows now say what the code does.

The field dump is **version three** and carries the data the two processors that need no subsystem read:

| Processor | Data | In the corpus |
|---|---|---|
`field_processor_extra_radiation` | `extra_radiation_min` / `_max` per intensity | one field radiates: `fd_nuke_gas`, 0–1, 0–2, 0–3 |
`field_processor_wandering_field` | the type's `wandering_field` | three vents wander: a gas vent into toxic gas, a smoke vent into smoke, the tindalos rift into its own gas |

The loader carries both, and a gate pins the corpus (`the_dump_carries_what_the_radiation_and_wandering_processors_read`), so a future dump that dropped either number
fails loudly instead of leaving a processor to be written against a guess.

## The sky is layer two, and the M2 receipts are complete (round 549)

The weather session has a receipt — `0x4c221d74263f52f4` over twenty-seven turns — built from the port's **own** recomputed sky rather than read back from the record: the
temperature, humidity and pressure are recomputed from the generator's inputs, quantised to the millionth the session gate compares at, and folded in beside the manager's
weather id, wind, clock and time-to-next-change. The test computes the same fold from the recording and asserts the two numbers are equal, so the receipt is a gate on
the port and not a transcription of the fixture.

That completes the milestone's proof line for P2: **a replay hash per turn over the weather, scent and field sessions**:

| Layer | Session | Turns | Receipt |
|---|---|---|---|
fields (0) | `gas-60` | 59 | `0xbb4513a5ce73269a` |
fields (0) | `gas-fall` | 39 | `0x033342d5a7845618` |
fields (0) | `gas-wind` | 23 | `0x12ca5c3c692106d4` |
scent (1) | `scent-session` | 13 | `0xaac13754071041c0` |
sky (2) | `weather-session` | 27 | `0x4c221d74263f52f4` |

## Scent folded into the digest, and the dump lists a square three times (round 548)

The scent session has a receipt now — `0xaac13754071041c0` over thirteen turns — and folding it in found something about the **recording** that no square-by-square
comparison could: the dump lists the whole window on **each of three levels**, so a square appears two or three times in a turn. A digest over the raw list would depend on
the shape of the dump rather than on the scent, so the digest collapses to one value per square first.

That is the second time the digest has caught a property of the fixture rather than of the port (the first was the scent window's radius). It is worth having for exactly
that: a per-square comparison silently forgives duplicates, a zero that could have been a value, and a square the port never had.

Layers so far: fields are zero, scent is one. The sky is next.

## Every gas session has a receipt number now (round 547)

The milestone's proof line is *replay hash per turn* (`05-roadmap.md`, M2). The gates had been comparing turn by turn and square by square, which says **where** a turn
differs but never says **that** the turn differs as a single fact - and a layer nobody compared is a layer no gate would miss.

There is now a digest: every square of a turn is written into a canonical byte string (layer, x, y, z, key, value) and hashed with FNV-1a, and a session's receipt is the
fold of its turns in order. Three are pinned:

| Session | Turns | Receipt |
|---|---|---|
`gas-60` | 59 | `0xbb4513a5ce73269a` |
`gas-fall` | 39 | `0x033342d5a7845618` |
`gas-wind` | 23 | `0x12ca5c3c692106d4` |

A pinned digest is a **receipt, not a specification**: it says what the port produced when the gate was written, and it has to be re-derived from the recording whenever the
port is meant to change. That is the point of it - a change that is meant to happen becomes a line in a diff rather than a silent drift, and a field that stops spreading
moves the number even if every existing assertion survives.

The digest is also where the other layers will go: scent and the sky are their own layer numbers in the same function, so folding them in is a value per square rather than
a new gate.

## The smoke gate was the lightmap cast talking, not the reference (round 546)

Swapping the sight cast left one test red: a square behind smoke read `0.926` and came out `Blank` where the test wanted `BrightOnly`. The measurement said the smoke
was not crossed at all, which looked like a fold bug - and it was not. Two things were wrong with the test, not the cast:

* the smoke had been moved behind the character while chasing the bug, so the ray to the square never crossed it;
* once it was back in front, the cast measured `2e-9` - because the reference's own attenuation is `exp(-transparency * distance)` and smoke's transparency of ten gives
  an answer of essentially zero. `vis * lm` is then zero and the square is `Blank`, which is **the reference's behaviour**: the old cast had been diluting the smoke
  across a whole octant row and reporting `0.078`, a number the reference never produces.

The gate now uses a thickness chosen for the branch it covers: smoke at `1.5` puts the cast at `0.0498` - under the tenth that makes a square `obstructed` - and the
square's own light of 250 multiplied by it is 12.4, over `LIGHT_AMBIENT_LIT`, which is what makes it `BrightOnly` rather than `Blank`. Two hundred would have landed at
9.96 and passed a hundredth short of the branch.

## The level's sight cast is the reference's now, and smoke shows a hole in the fold (round 545)

`Level::seen_cast` - the one place the port answers "how clearly is this square seen" - now runs `cast_zlight` instead of the recursive light cast. Everything above it
(`seen_at`, `seen_through`, `SeenGrid`, the frame's visibility column) is unchanged, and the whole suite stayed green through the swap except one test.

That test is worth keeping red for a round: a square behind a column of smoke read **0.926**, which is `sight_calc` of open air at distance two. It turned out to be the
test's own geometry - the smoke had been moved behind the character while chasing the number - and the red is closed in the entry above.

The lesson of the round is that the fixture gate made the swap safe: without a record of what the reference's cast actually produced, "the cast is transcribed" would
have been a claim about the port agreeing with itself, and the smoke bug would have been invisible.

## The vision cast matches the reference on 867 of 867 squares (round 542-544)

Getting there needed three instrument fixes, one wrong constant, and one wrong metric.

**The instrument** (`--dump-visibility`) now records the cast's inputs beside its output: vision transparency per square, its floor bit, its level's `no_floor_gaps`, and
the sight penalty. It had been lying three ways - it read `transparency_cache` (the *light* cache) instead of `vision_transparency_cache`; it recorded no floors at all,
so the port inferred "can I see below?" from the seventeen-square window instead of the level summary; and it built with `skip_lightmap = true`, where the sight cast
lives, so it read a `seen_cache` left by an earlier build. Then two build-order facts: each build wipes every level and only the build targeting the character's level
writes `VISIBILITY_FULL` into the character's own square, so the levels are built `-1, +1, 0` and made dirty first. And `--avatar-at` plus `--seed` pin the recording,
because the fixed world's start square and its map both vary per process - without them "the fixture" was a fixture of the run, not of the world.

**The constant.** `MAX_VIEW_DISTANCE` is `SEEX * HALF_MAPSIZE` = **60** (`map_scale_constants.h:12-27`), not the ported 72. Every sight and light value was about four
percent off: the reference's recorded vision transparency is `2.3/60 = 0.038333` and the port computed `2.3/72`. Fixing it moved `light_range`, `sight_calc` and
`vision_threshold` - all three of which had tables generated from the wrong constant, so those tables were the port agreeing with itself and were regenerated from the
reference's formulas.

**The metric.** `rl_dist` is not Chebyshev: `CIRCLEDIST` defaults to `true` (`options.cpp:1706-1710`), so `rl_dist` is `trig_dist`, the Euclidean length truncated to
an integer (`line.cpp:1025-1030`). A diagonal square is further away than a straight one, which is exactly the "one distance step brighter on every diagonal" the
fixture had been reporting for three rounds.

With those, `the_port_s_cast_reproduces_the_reference_s_seen_cache` is **green: 867 of 867 squares**, over three levels, including the two squares the character sees
through a hole in the floor and the two hundred on its own level.

## The whole cast is transcribed (round 541)

`cast_zlight` is in the port: eight horizontal segments and four vertical ones per direction, the four vertical transforms, and the direction gate. The port now has
the reference's sight algorithm end to end, over a plane it builds from three levels' transparency and floor caches.

Two things the transcription settled that reading the code twice had not:

* **The origin's own level is not a separate cast.** The reference has no "one level only" direction - `vertical_direction` is `UP`, `DOWN` or `BOTH`
  (`shadowcasting.h:32-36`), and the flat case is the **zero-distance row** of every segment. A port that casts the character's level separately and then adds the
  neighbours would double-count the level boundary.
* **A sealed level still sees upwards.** `directions_to_cast` starts at `BOTH` and narrows to `UP` when the character's own level has `no_floor_gaps` (`lightmap.cpp:1060-1068`):
  a whole floor means nothing below can be seen, while the sky above still can.

Four gates: the shaft straight below is seen and a floor under the feet seals it; a sealed level casts upwards only; one hole in a floor is exactly the square seen
through; and the character's own level arrives through the same cast rather than a separate one.

The cast is still **not wired** into `Level::visibility` - that swap is the next step, and it wants the vision fixture compared against it, not just synthetic planes.

## The port's visibility is the wrong cast, and now it says so (round 540)

`cast_zlight` is what fills a `seen_cache` (`lightmap.cpp:1079`). The recursive `castLight` the port ported - and which `Level::visibility` and `seen_at` are built on -
is the **lightmap** cast (`castLightAll`, `lightmap.cpp:988`). They are two different sweeps, and over the same patterned grid they disagree about **93 of 441**
squares, in both directions: squares one sees just past a corner that the other does not.

That is measured, not inferred. `the_flat_sweep_and_the_recursive_cast_are_different_algorithms` runs the z-cast's flat case (`z_transform = 0`, the reference's
`vertical_direction::NONE`) beside the recursive cast on the same grid and pins the divergence as a tripwire: **the day the port switches its visibility to the
z-cast, that gate fails and has to become an equality test.** Getting there is the next step, and it needs the z-cast's four vertical segments as well as the eight
horizontal ones.

The consequence for the vision fixture is direct: `corpus/vision/start-location.json` records what `cast_zlight` reached, so the port cannot match it while its
visibility comes from a different algorithm - no matter how right the port's own cast is about its own world.

## The cross-level rule is transcribed (round 538-539)

`cast_horizontal_zlight_segment` (`shadowcasting.cpp:160-350`) is in the port as `cdda_core_types::zlight`, over the port's own `Slope`, `Span`, `split_span`,
`sight_calc` and `accumulate_transparency` - the pieces were already there, which is why the *plane* could be transcribed rather than reinvented.

Four gates:

| Gate | What it pins |
|---|---|
a sideways ray descends, never deeper than its distance | the shape of the rule |
a floor is a ceiling from underneath | a roofed level is hidden, a hole in it is seen through |
`0.891366` at distance three through `0.038333` | the reference's traced intensity is the port's own `sight_calc` |
intensity falls with distance and stays a fraction | the range a seen value can take |

Round 538 left a fifth gate red - the intensity a descending ray carries was `0.9686` where a guess said `0.9624`. That guess turned out to be the problem, not the code:
the exact intensity depends on the span's **running mean of transparency**, and the reference's trace records that mean only for its own world. A gate comparing a
synthetic plane against a running mean this port computed itself would be the port agreeing with the port. The guess was removed and replaced by the two facts that are
anchored: the traced intensity, and the direction the value moves in as the ray travels.

## The visibility dump was reading a stale cache, and the cross-level rule is measured (round 537)

Round 536's fixture claimed the character saw two squares of the level below. It does - but not because the dump built them: `build_map_cache` rebuilds **one** level's
caches, and `seen_cache` on the level below still held whatever an earlier build had left there. The dump now builds every level it reports, so the numbers it records are
ones it produced itself.

With that fixed, `CDDA_ZLIGHT_TRACE` (a print of every write `cast_zlight` makes *off the origin's level*) answers a question reading the code could not: **which segment
crosses a level**. It is the horizontal ones, not the vertical ones:

```
zlight HORIZ/SEG 75,72,-1 dist 3 intensity 0.891366 cum 0.038333
zlight HORIZ/SEG 76,72,-1 dist 4 intensity 0.857844 cum 0.038333
```

So a ray walking sideways also descends, one level per minor step, while its intensity falls with `sight_calc` over the accumulated transparency - which is why the
squares below are seen at `0.891` and not at the `1.0` a square beside the character gets. The vertical segments (straight down a diagonal) are the other half of the
reference's rule and were not what this world exercised.

The fixture's numbers moved with the corrected build: the two visible squares below are now `(64,71,-1)` and `(64,72,-1)`, still `0.891` each, and the character's own
level still shows 220 squares with none above.

## Vision across levels is recorded now (round 536)

Vision is the one environment layer with nothing recorded for it: `--dump-frame` carries the **light** a square stands in, and light is not sight. `--dump-visibility`
dumps each level's `seen_cache` and `transparency_cache` after the same `build_map_cache` the game would run, over seventeen squares square and three levels.

`corpus/vision/start-location.json` is the first recording, from the fixed world's start square at `(71,70,0)`, and it says three things a per-level cast cannot:

| Level | Visible squares | What it means |
|---|---|---|
`z` (the character's own) | 220 of 289 | an open room with walls where the cast stops |
`z + 1` | **0** | the character is under a roof, so the level above contributes nothing |
`z - 1` | **2**, at `0.891` each | two squares of the floor below are visible through an opening |

The last row is the gap the port's per-level cast has: its sight never leaves the level it starts on. The dump's `transparency_cache` column explains why the fixture
is a good one - every square in this world reads either `0` (solid) or `0.0383`, the open-air value **already multiplied by the weather's sight penalty**
(`lightmap.cpp:152-157`), so a port that used the open-air constant would see further than the reference in bad weather.

Two gates pin what the fixture says rather than what the port does: three levels of the right shape with the character's own square always visible, and the
reach that crosses a level - two squares below at `0.891`, nothing above.

## Scent diffusion replays, and the fixture's window was the bug (round 535)

The scent session is green: fourteen turns of the port's window against the reference's, every square of a thirty-seven-square window, from one seeded square to
sixty-five.

The red gate that opened this round was misread twice before it was measured, and both misreadings are worth keeping. The port looked **one point high on the ring**
and then, with a decay added, **one point low on the corners** - two plausible stories about the turn's decay order, neither of them true. Running the reference's own
`update` arithmetic over the recorded layer in Python reproduced the record exactly, which took decay out of the picture. Then `CDDA_SCENT_TRACE` - a print of
`here`, `squares_used`, `temp_scent`, `incoming` and the result for one square per update - showed the reference computing **1** at `66,59` on the fifth turn while
the record said nothing there.

That was the answer: **the dump's scent window was three squares**, because it reused the fields' radius. A fire stays where it is put; scent walks a square a turn,
so the record stopped seeing the layer at exactly the turn the replay began to disagree with it - and "absent" was being compared as zero.

The window is now six times the fields' radius. Everything else was already right: no decay in this path, the reference's truncating division, and weights of ten and
two for plain and `REDUCE_SCENT` squares.

## The session's weather is decided from the reference's dice (round 533)

The other half of the sky. The point is a formula, but *which* weather and *when the next change is due* are decided from dice, and those dice were invisible: the
manager's five an hour were unlabelled, and the wind's sector is drawn with a `std::discrete_distribution` on the engine itself (`weather_gen.cpp:256-276`), so it
leaves nothing in the trace at all.

Two additions to the instrument:

| Addition | What it makes visible |
|---|---|
`mark weather.session.hour.<n>` around the manager's update | which hour the dice belong to - the manager is asked about the hour the clock has just moved into, so these belong to the **next** turn, not the one whose sweep runs beside them |
`drawn wind.sector <n>` in `get_wind_direction` | the sector the engine chose, for the same reason the decay deadline needed a label |

The replay seeds the manager with turn zero's sky and direction, then runs twenty-six hours on the reference's own dice. Every id and every next-change turn match, and
the dice are checked as they are spent: each draw asserts the range the reference asked for, so a wrong number of draws or a different range fails where it happens
rather than shifting everything after it.

## The session's sky is recomputed, not just read back (round 532)

The session dump now carries the generator's own **inputs** - where the player stood (`common_x/y/z`), the season, the fraction of the year and of the day, and the noise
field's seed - and the point the reference produced from them: temperature in kelvin, humidity, pressure.

With the inputs in the file, the sky stops being a number the port can only copy: it is a pure function of four values, so the port recomputes every turn of the
weather session and matches the reference to a millionth in temperature, humidity and pressure. No die is involved, which is the point - a difference there is a formula
difference and not a roll that landed elsewhere. That is the half of the sky the light multiplier and the sight penalty hang on.

The other half - the manager deciding *which* weather, and when - needs the hour's dice and the wind sector, and is the next piece.

## The session dump carries the sky, and the clock can move (round 531)

The M2 exit gate wants a per-turn hash of an environment session that covers a **weather transition**, and until this round a session could not contain one: the dump
recorded the wind but not the weather, and the calendar stood still for the whole run. A dump with the wind and no weather cannot tell a storm from a calm night at the
same air speed - the manager's id decides the light multiplier, the sight penalty and the sound attenuation.

Two additions to the instrument, both ahead of any port work:

| Addition | What it makes visible |
|---|---|
`"weather"`, `"temperature"`, `"nextweather"` on every turn | the sky the manager reported, the air temperature in kelvin, and the turn it said the sky was next due to change |
`--turn-minutes <n>` | how far each turn advances the calendar, with the manager updated after each step - default zero, so every existing fixture keeps its frozen clock |

`corpus/fields/weather-session.json` (seed `w2`, one game hour per turn, twenty-six turns) crosses **two** transitions: light drizzle, then cloudy at the fifth turn,
then clear. Two gates pin the fixture's promise rather than the port's behaviour - that it really does change sky, and that every pair of turns is exactly 3600 apart with
the manager's next change never behind the clock - because a re-record that quietly lost the transition would leave every later gate passing on a session that never
changed weather.

Re-recorded with the wider header, all three gas sessions still replay die for die: sixty turns indoors, the stairwell fall, and the roof's wind. Their fixtures now carry
the clock, the sky, each field's `alive` flag and `decay_at`, and the decay draws are labelled with the square that rolled them.

## The roof session replays (round 530)

`gas-wind` is green. The three gas sessions - sixty turns of indoor toxic gas, a stairwell fall, and a gas alone on a roof in an eighteen-knot wind - all replay
square for square against the reference's own dice, **including the branch that had never run in the port**: the wind-blocker scan, with its three real draws, its
nine-row bearing table, and the choice it makes to blow into the wind.

The last three differences were all on the replay's side of the fence, and each one is a rule rather than a detail:

| Difference | The rule |
|---|---|
the dump prints fields whose `alive` flag is false | the reference marks an entry dead and leaves it in the square until the sweep tidies up; the port despawns on death, so the comparison drops corpses |
the replay aged a whole submap at once | the sweep ages **each square as it reaches it**, so a newborn on an already-passed square waits a turn - batching gave every newborn an extra turn of age |
`spent` counted only dice | the decay deadlines are drawn from the engine, not through `rng()`, so they are reported separately: ten dice **and** ten deadlines, both asserted |

That last one matters for the shape of the gates: a replay that spends every die and no deadline gets every square right and every death wrong, which is precisely the
state this fixture was in for six rounds. The tests now say how many of each were spent, so a regression of either kind fails loudly.

## The decay deadline is drawn every turn, and the replay was drawing it from the wrong dice (round 529)

Two bugs, both found by making the reference say what it was doing rather than by reading the port:

**The reference draws a fresh deadline on every decay.** `CDDA_DECAY_TRACE` prints `decay_time` on the entry of every `do_decay` call, and it is **zero every time** -
so `initialize_decay` runs each turn and the deadline it computed last turn is never reused. The port cached it, which with the harness's frozen calendar meant a
deadline set ahead could never be reached: a field the reference buried after two turns stayed alive for the whole session. Fixed with a red gate
(`every_decay_draws_a_fresh_deadline`) that spends two recorded draws and dies on the second.

**And the replay was decaying with a random engine.** The decay call in the replay passed `&mut Rolls::new(7)` - a fresh generator with no recording attached - so the
recorded `drawn decay.turns` values were never spent and every field's death was decided by a schedule the reference never had. The trace that found it printed
`engine draw decay.turns: no recording attached` on the first field of every turn. One line: the decay now uses the turn's own dice.

With both fixed the roof session's dead entries line up, and what is left is small and named in the ignored gate: the dump prints fields whose `alive` flag is false
and the port despawns them, so the comparison has to drop corpses before it compares; after that the port is missing the source at state three, and its lateral newborn
arrives one age older than the reference's.

## Every decay is attributable now (round 528)

The reference draws a field's decay deadline from the engine itself, so the value landed in the dice trace with **no square on it**. Two fields decaying on one turn
therefore looked identical, and that ambiguity is what kept a risen field alive in the port across three rounds of chasing.

`mark decay.<x>.<y>.<z>` (and `decay.newborn.<x>.<y>.<z>` for the branch that only ages a field) now precedes every `do_decay` call in `process_fields_in_submap`.
Re-recorded with the labels, the roof session reads like a table:

| Turn | Square | Drawn | What happened |
|---|---|---|---|
0 | 71,71,1 (newborn) | 47 | the placed gas, aged once |
1 | 71,71,1 | 16 | the source, its deadline reset by the rise |
1 | 71,71,2 (newborn) | 2 | the risen field, born this turn |
2 | 71,71,1 | 28 | the source again, reset by its lateral move |
2 | 71,71,2 | **1** | the risen field, and a draw of one turn kills it |
3 | 70,71,1 | 12 | the lateral newborn |

Two more things fell out of the same work. `field_entry::set_field_age` resets `decay_time` (`src/field.h:70-78`), and everything that touches an age goes through it -
`gas_spread_to` on both source and newborn, and `do_decay`'s own level loss - so a field that spreads draws a **fresh** deadline next turn. And the port's
`decay_start` now spends the recorded value when one is attached, with two unit tests that pin the truncation: a draw of one turn kills a level, a draw of three keeps
it.

The roof fixture's replay is still ignored, and the reason is now narrow: the port computes the right deadlines but **reaches the wrong set of them** - its risen field
keeps the deadline it drew on turn one, where the reference drew a fresh one on turn two. The mechanism that resets it between those turns is the open question, and it is
a question about one member of `field_entry` rather than about the sweep.

## Three instruments the roof fixture needed (round 527)

Chasing the risen field taught more about what the oracle could not see than about the port:

| Instrument | What it settled |
|---|---|
a **destination** line in the gas trace (`went to x,y,z (rise / into the wind)`) | which square the dice actually chose, instead of inferring it from a state dumped after the whole sweep |
a **turn label** on stderr (`== gas.turn.<n>`) | I had been attributing trace lines to the wrong turn and reading a rise as a lateral move |
`drawn decay.turns <n>` in the dice trace, plus an `alive` flag per dumped field | the reference's decay draws its exponential **straight from the engine** (`field.cpp:86-88`), so it never appeared as a dice line, and a dead entry stays in the dump reading like a live one |

The `alive` flag alone resolved the mystery that started this: the field that rose to z 2 had **died** on its first decay - `initialize_decay` truncates the exponential
to whole turns, so a draw under one turn puts `decay_time` in the past and the field loses its level at once - and the dump had been showing its corpse as
`intensity 1, age 0`.

With the destination trace the two spreads in the recording are now known exactly: turn one a **rise** into z 2, turn two a lateral move **into the wind** to
`(70, 71, 1)` - the blocker branch choosing a blocked square, which is what the three recorded reals are for. The port replays both correctly, dice and squares.

**What is left** is decay bookkeeping, and it is named in the ignored gate: the reference draws eleven decay exponentials over twenty-four turns and the port reaches
five fields, so the newborn and the risen field keep an age one turn too old. The draws are parsed and consumed; the counts disagree, so the next step is to find which
fields the reference decays that the sweep never reaches.

## The sweep is square by square (round 526)

`run_environment_turn` now walks a submap **square by square**: for each `(x, y, z)` it runs the fire processor, then the incendiaries, then the gases standing
there, then decays that square's fields. That nesting is the rule (`process_fields_in_submap`, `map_field.cpp:457-505`), not a detail of the loop: a field born on a
square the sweep has already passed waits a turn to age, so a gas that spreads **forward** in the scan order is aged the same turn and one that spreads **backward**
is not.

It also came with a lesson about the previous round's alarm. The rewrite was reverted last round because a burning room stopped smoking. The real cause was not the
sweep at all: **the test was a coin flip**. The reference only counts smoke for flammable ground - `smoke += 2 + windpower / 5` (`:1089-1122`) - or for burning items
(`:1065`), so a fire on the test's inert floor can only smoke through the gate's own hundred-to-one draw. Twenty turns of that is a coin flip wearing a rule's clothes,
and the seed had been landing heads.

The pair of tests that replaced it: `a_burning_room_fills_with_haze_over_twenty_turns` asserts what is guaranteed - the fire survives and leaves haze - and
`a_fire_smokes_a_flammable_room` asks the smoke rule itself, on ground that feeds it, with five hundred turns so p ≈ 1 - (1 - 0.03)^500. The rate is honest now instead
of lucky.

**What the roof fixture still shows** is a smaller and sharper disagreement, named in its ignored gate: the ages of the fields the sweep keeps now agree, but the
reference **loses the field that rose to z 2** between turns two and three, where the port keeps it. The reference's decay compares `decay_time` against a calendar this
harness never advances (`field.cpp:92-114`), so that death is not decay - it is a rule the port does not have, and finding it is the next instrument's job.

## The sweep's scope, and why the per-square rewrite is not mechanical (round 525)

The remaining gas-parity disagreement is the sweep's **decay order**: the reference decays each field inside its per-square loop, the port batches a submap. The
obvious change - ask each rule for one square at a time - was written, run, and **reverted**, because it took the fire path with it, and the reason is worth writing down
before the next attempt.

Scoping the rules is now clean: `Where { Anywhere, Submap, Square }` replaces a bare `Option<(x,y,z)>` at the same arity in all four sweep doors, so the next attempt
starts from a rule that can be asked about one square without a new signature.

What the experiment showed: with `Where::Square`, a burning room's fire **discharged three times in sixty turns** where the submap-wide pass discharges every turn, and
the smoke gate - `rng( 0, 100 - windpower ) <= smoke_produced` with `smoke_produced = 2 + windpower / 5` (`map_field.cpp:1417`), about three turns in a hundred in still
air - went from "sometimes" to "never". Two separate facts fell out of that:

| Fact | Consequence |
|---|---|
| `process_fires_in_submap` only discharges for a fire whose entry it re-reads **in the order it collected**; scoping by square changed which fires it saw | the per-square rewrite has to move the fire processor into the square loop, not filter it by square from outside |
| the 20-turn "fills with smoke" test asserts a **~3% per-turn draw** | it was passing on a lucky seed; it is a coin flip, and the honest version either runs long enough for the rate to matter or seeds a turn that must make smoke |

So the round's delivered change is the scope type, and the finding is that the per-square sweep needs the fire processor rewritten as a per-square function first - a
change with its own tests, not a filter.

## The decay clock is data, and the dump did not have it (round 524)

Three instrument gaps fell out of trying to replay one roof fixture, each found by a divergence rather than by reading:

| Gap | What the recording said | What the port did |
|---|---|---|
| `half_life` missing from the field dump | the extinguisher loses a level every **ten** turns | every field was born with a placeholder million, so nothing ever decayed |
| `--dump-fields` had no `linear_half_life` | six of the corpus's fields set it | the flag was unexpressible |
| the series dump stopped **one level short** | the gas **rose** to z 2, thinning its source with no visible destination | the port read a rise it had no terrain for |

`half_life` is a **type-level** field (`field_type.h:217`), not per intensity, so it sits beside `percent_spread` in the dump; 65 of 145 field types have a real
half-life once recorded. And the dump's window now runs one level above the old one, because a gas that finds no neighbour goes **up** and a window that stops short
shows a thinning source with no destination - which a port can read as a decay and get right by accident.

The replay also had **two hardcoded numbers** that were one gas's: `percent_spread` 30 and a placeholder half-life. Both are read from the prototype dump now, through
a `GasType` the fixture's own id selects. With 90 read from the data the extinguisher's recorded dice finally lined up - the three ints of turn two are a gate, a scan
start and a refused `one_in`, where they had been a gate that refused on the port's 30 and left the rest unspent.

**What is left** is the sweep's decay order and it is named in the ignored gate: the reference decays each field **inside its per-square loop**, so a newborn is aged
this turn only if the sweep has not yet passed its square. The port batches a submap's decay after its gases and ages every newborn once. Measured against
`corpus/fields/gas-wind.json`, turn three puts age two on a newborn the reference leaves at one and two on a risen field the reference resets to zero - squares and
intensities already agree, which is why the remaining disagreement is nothing but ages.

## The wind is per square, and the roof fixture says so (round 523)

`Wind { power, direction, sheltered }` is now read **per square**, which is what `map::spread_gas` does at its top, and the sweep passes it through
`process_gas_in_wind`. The older `process_gas_in_submap` keeps its single-wind door for callers that have one wind to give.

The second instrument error of the wind work, found the same way as the first: the dump recorded the windpower at the player's square, so a gas on a roof and one in the
cellar beside it recorded the same zero. Each field entry carries its own `windpower` and `sheltered` now, and the measured example is exactly the case that matters - a
gas on a roof under a thirteen-speed wind gets a local power of **18**, where every indoor square before it read zero.

`corpus/fields/gas-wind.json` is the first fixture whose gas is **outdoors**: `fd_extinguisher` at level three on a roof, twenty-four recorded turns, **three real
draws** in its gas blocks - the wind-blocker branch, in a recording.

Its replay gate is written and **ignored**, with the reason in the attribute: the session is red on the field's own decay rather than on the wind. The extinguisher drops
from intensity three to two on turn two having spent three dice and made no destination, and the replay's ageing pass hands `do_decay` a clock of **zero** because the
fixture does not record the calendar - so the port ages the field differently and diverges one turn later. The branch the fixture was made for is exercised and correct:
the three recorded reals are spent where they belong and every range matches. The next instrument is `now` in the fixture.

## The wind blocks the squares it faces (round 522)

`map::spread_gas` has a branch the port never had: when the gas is **not sheltered** and the wind is at least five, the three squares facing the wind are asked
`x_in_y( 1, max( 2, windpower ) )` - **one real draw per blocked square**, in the order the clockwise scan collected them - and a gas whose every candidate lost stays
put rather than falling through to the rising branch, because that branch is the `else` of the lateral attempt and not a second chance after it.

Ported with four transcribed tests, and the branch's shape is the kind of thing that only a recording settles: `get_wind_blockers` is a **nine-row table** keyed by
bearing with first-match-wins (330, 301, 240, 211, 150, 121, 60, 31, 0), naming world directions rather than "left of the wind". A rotation formula that was one row
off would block the wrong three squares and nothing but a session replay would notice.

**Two instrument errors fell out of the same work:**

- The dump recorded the windpower at **the player's square**, not the gas's. `spread_gas` asks `get_local_windpower` about the square the gas stands on, and a gas on a
  roof feels a wind that one in the cellar beside it does not - the recorded value was 0 for both. Each field entry now carries its own `windpower`: measured, a gas on
  a roof under a thirteen-speed wind gets 18.
- `spread_gas` took the wind **twice**, once as a parameter for the chance's ceiling and once in `Around` for the blockers, so a gas could blow freely through a wind its
  own chance never felt. There is one wind now, in `Around`, which is what the reference does at the top of `map::spread_gas`.

The gap this leaves is named: the sweep still hands the rule a **direction of zero** and `sheltered: false`, because the direction is not threaded through
`process_gas` yet. Indoors that is harmless - the recorded windpower is zero there and the branch never runs - but a roof fixture would spread into the wrong three
squares until it is threaded.

## One gas rule, two doors (round 521)

The port had two gas-spread implementations and they disagreed. The session sweep's followed `map::spread_gas` including the square below and the square above; the
older per-field door (`spread_gas`, used by `process_field_turn`) only ever looked **sideways**. Two rules that disagree are worse than one rule that is wrong: the
session gates were green while the other door silently dropped every fall, and the same divergence would have hit wind next.

There is now one `process_one_gas`, and both doors call it. The older door's terrain question was `can_enter(x, y)`; it is now the same `can_take(from, to)` the sweep
takes, because a closure that cannot name a level cannot answer a question about one. The legacy door answers the lateral half and refuses a step between levels
rather than guessing, which is what it did before - but it can no longer *disagree* about where a sideways gas goes.

Verified rather than assumed: re-recording the lateral session with `CDDA_RNG_TRACE_REALS=1` and slicing its gas blocks gives **84 int draws and zero real draws**, so
the wind-blocker branch (`x_in_y`) never fired in that session and the current gate covers everything it exercises. A windy fixture is what would exercise it, and it
is not written yet - the ledger says so.

`draw_start` in the fixture moved between the two recordings and the states did not: worldgen draws dice and loading a saved world does not. The replay slices by marks
and never reads the offset, so this is not a determinism failure - but a reader comparing two fixtures would have thought it was, which is why the corpus README now
says it.

## The incendiary field, and a wisp rule that was stacking fields (round 520)

`field_processor_fd_incendiary` is in: two dice pick a neighbouring square, it catches fire if any of the three flammable flags says so, and the hot air is made
either way - **after** the two offset dice, so a field about to die still spends them.

Writing it corrected two things that were wrong in the port already:

- **`create_hot_air` was stacking fields.** It spawned five entities, so a square that caught two wisps held two thin fields where the reference holds one at
  intensity two. `add_field` thickens; the port now has one `add_or_thicken` that every field creation goes through, and the five-wisp rule goes through it.
- **Hot air was hardcoded to z 0.** A fire on the first floor left its wisps on the ground floor. `create_hot_air` now takes the level.

The rule that fell out of the data: `fd_hot_air2` has **three** levels, so five wisps on one square are intensity three, not five. The old `fire_vent` test asserted
five *entities*, which is what the stacking bug produced; it now asserts five wisps by total intensity, which is what the reference means.

The gap is named rather than hidden: the reference also asks `flammable_items_at`, and the item layer does not exist yet.

## The sweep is spatial, and that is a rule about ages (round 519)

A fixture with a stairwell - `t_stairs_down` with open air under it - made the gas **fall** on turn six: source to intensity two at
z 0, destination born at intensity one at z -1. It also broke the port in a way that hand-written cases never would have.

The reference's sweep is **spatial**, not per kind: `map::process_fields` walks submaps in ascending `(z, x, y)` and ages a field when it reaches the field's own
submap. A gas that falls into the submap the sweep has already passed keeps the age it was born with; one that spreads into a later submap is aged in the same turn. The
port aged everything at the end of the turn, which gets the second case right and the first wrong - a one-turn age error that the lateral fixture happened not to
expose, because a gas spreading sideways in the same building lands in a later submap.

Three things had to change, and each is smaller than the bug:

- `run_environment_turn` now walks the sweep with a **cursor** over `submaps_with_fields`, doing one submap's fires, then its gases, then the ageing of what stands on
  it. A cursor rather than a list because a field born in a later submap is visited by the same sweep.
- `process_gas` and `process_fires` gained `_in_submap` variants; the old signatures delegate, so every existing caller is unchanged.
- The **field half of `gas_can_spread_to` moved out of the caller and into the crate.** Whether a square already holds gas at least as thick as the gas arriving is
  readable only by whoever can see the field a previous square of the same sweep left behind, and a caller that snapshotted the map before the turn got it wrong at the
  first exchange between two levels. The caller now answers only the terrain question - can the square be walked, is the step legal - which is what the test's `can_take`
  had been trying to say all along.

Two gas gates stand on this: the lateral session (fifty-eight transitions) and the fall session (thirty-nine, including the two levels trading intensity as each
spreads into the other). Both compare position, intensity and age every turn and refuse to pass unless every turn ran and the reference's dice were spent.

## A gas needs permission to fall, and the rule is `valid_move` (round 517)

The first gas session gate went red for a reason worth keeping: the port spread the gas **down through the floor it was standing on**. The reference asks two questions of
a vertical step - `gas_can_spread_to` for the square, and `map::valid_move` for the move - and the port only had the first.

`valid_move`'s vertical half is now its own rule (`cdda_core_types::movement::can_step_vertically`) with seven tests, and the shape is the reference's, not the obvious
one:

- the **upper** square must be passable at all;
- an impassable square below refuses the step, **unless** the upper square is a ledge - open air may have rock under it, because the reference deliberately forces a drop
  there rather than trapping a player on a roof;
- a square with a floor refuses the fall unless it is a ledge or says `GOES_DOWN`;
- `NO_FLOOR_WATER` counts as no floor for the floor test and **not** as a ledge, so the two flags part company and only one of them stops a fall.

The function takes its two arguments as **upper** and **lower** rather than from and to: the reference's first act is to sort them that way, and a caller that had them the
wrong way round would get a plausible answer to the wrong question. Two of the seven tests were wrong on the first write - the ledge-over-rock case reads backwards, and
water is a hole - which is the kind of thing a transcribed rule catches and a remembered one does not.

**Two instruments fell out of the same work.** The gas processor now marks its draws with the **square** (`gas.begin.<x>.<y>.<z>`), because the reality bubble holds other
gases and their draws land in the same trace: a slice that cannot tell whose draw is whose replays the bubble, not the fixture - the first gas recording had 2,863 draws
for a session that made 84. And the remote runner now syncs with `--no-times`: preserving modification times delivers a fresh file with an old timestamp from any machine
whose clock is behind, and cargo then reuses the artifact it already has, so the gate silently tests the previous round's code.

`corpus/fields/gas-60.json` is re-recorded under `--world harness-fixed` and its dice are sliced to the watched square: sixty turns, 84 draws, a split at turn 48 into a
source at intensity two and a destination at intensity one.

**The replay gate is green** (round 518), and the per-turn marker is what made it possible: the sweep now writes `gas.turn.<n>` before each `process_fields`, so every turn
gets its own recording and a turn that spends a different number of dice says so where it happens instead of shifting every turn after it. Fifty-eight transitions are
compared field for field - position, intensity, age - and the gate refuses to pass unless every turn ran and at least eighty of the reference's own dice were spent.

One thing the fixture taught that is worth keeping: **the reference's first sweep does not see the gas.** The dump places the field and then sweeps, but the square is not
in a cached submap yet, so the trace has no dice for the transition into turn one while the age still advances. The replay therefore seeds from the first state that sweep
left rather than from the placement - the honest read of the recording, and one that a test written from the intent would have got wrong.

## The field dump learned to look up and down, and the gas learned to say what it rolled (round 516)

A gas fixture needed two things the oracle could not do, and both were gaps in the instrument rather than in the port.

- **The field dump read one level.** `--dump-field-series` scanned the radius around the player on the player's own z, so a gas that falls or rises would have been
  reported gone and two runs would have agreed about nothing. It now scans three levels, and every square carries its `z`.
- **The gas processor's dice were unlabelled.** The fire processor has had `fire.begin`/`fire.done` marks for three rounds; the gas processor drew its four dice into
  an unmarked trace, which made a gas session unreplayable. It now marks `gas.begin`/`gas.done` around each spread.
- **The wind is now part of every field turn.** A gas's spread chance is `rng( 1, 100 - windpower )` and its destination is resisted when it blows into the wind, so a
  fixture whose dice show the ceiling moving between 100 and 94 has to say why. Each turn carries `windspeed`, `winddirection` and the local `windpower`.
- A `--place-gas <id> <intensity>` option places a gas on the fixture's square, and a fire intensity of **zero** now means "no fire", which is how a session can be
  about the gas alone: a fire adds smoke of its own and there would be no telling the two apart.

`corpus/fields/gas-60.json` is the first recording made with it: `fd_toxic_gas` at intensity three for sixty turns under a light wind. It sits still for forty-seven
turns and then splits - the source from three to two with its age falling from 47 to **33**, the destination arriving at intensity one and age **16**. Those are the
spread rule's arithmetic, and a port that gets them wrong looks plausible until the split. None of the three levels is touched, which is itself the fact worth
knowing: a gas on a floored square drifts rather than falls, so the fall and rise branches need a fixture with a hole in it, and that is the next instrument.

## Ten recorded days, and three bugs the longer gate found (round 514)

`--dump-weather-manager` runs the reference's **manager** rather than the generator: it owns the null id, the first update, the overrides and the next turn, and none of that is
visible from `weather_generator` alone. Ten recorded days (seed `weatherwatch`, 240 hours, 1,192 dice) contain fog, drizzle, rain and **fourteen changes of sight absorption**.

The port replays all 240 hours through the running world and matches every id, every next turn, both wind numbers, the humidity and the pressure to a millionth.

Three real bugs only this gate would have found:

1. **A data-defined function could not see the point.** `jmath`'s bodies were evaluated with the *builtins* as their outer context, so `dprhx`'s `weather('humidity')` had nowhere to
   go; every fog condition failed from the inside out and the sky came out clear. The failure was invisible because `Condition::holds` treats an evaluation error as false.
2. **The chain was missing.** A test-side loader built weather types from `..Default::default()`, which drops `required_weathers`, so `drizzle` could appear while the sky was
   `fog`. The lesson is the same one the manager gate taught: a partial type is a trap that reads as a full one.
3. **The sight penalties were all one.** The same loader left `sight_penalty` at its default, so the flush count was zero on both sides of the comparison - a vacuously green half of
   the gate. Types are now read by **one** field reader shared by the dump loader and the data loader.

**One number is not bit-identical, and the reason is not a formula error.** Round 515 chased it down: the recording is written to seventeen significant digits for every
double, and the port then matched the pressure exactly once it multiplied the way the reference does. Three findings came out of the chase.

- **`noise * 15` is a single-precision product.** `raw_noise_4d` returns `float` and the literal is an `int`, so C++ promotes the literal to `float` and multiplies in
  `float` before widening; multiplying in double leaves a residual of a few tenths of a microbar. The oracle's own pre-narrowing double - `harness_weather_temperature_terms` -
  is what made this visible instead of arguable.
- **The recording had been rounded by the fixture, not by the game.** Ten-digit fractions made `cos` differ in its tenth decimal, and nine-decimal temperatures truncated a
  `float` that needs seventeen significant digits to round-trip. Every recorded number is now written exactly, and the gate compares the temperature with no tolerance at all.
- **`cos` is not the same function on both platforms.** The recording is made on macOS and the tests run on Linux; the two C libraries disagree by one unit in the last place
  on some angles. That is the whole of the remaining residual: one hour in two hundred and forty, one ulp, in a `double`. The temperature is exact because storing it as a
  `float` absorbs the bit.

A **metric was also caught being vacuous**: the harness patch that added the inline capture changed the compiler's code generation, and the reference's pressure moved by three
tenths of a microbar between two builds of identical source. Capturing a value from inside a function is not free, so the probe was reverted and the question was settled by
arithmetic against the values already recorded.

## The weather in the world, and the wind's one answer (round 513)

The weather is now where the game keeps it: a `Climate` resource holding the system, the wind the last update reported, the id, and the absorption the light is cast under.
`tick_climate` runs one turn of it and hands the world what changed - the map's light staleness, the `Weather` resource's sight penalty, and the wind.

Two rules that only a running world can get wrong:

- **The world's wind wins over a caller's.** `run_environment_turn` keeps its wind parameter because a session replay drives the wind from its fixture and has no climate; once a
  climate exists the parameter is a fallback. A world that quietly used both would spread half its smoke one way and half the other.
- **The absorbed light and the decided sky come from the same rules.** The resource's sight penalty is read back through the same `Rules` the decision used, so a world cannot end up
  drawing fog at a clear sky's brightness.

The gate runs a fog sky through the world and checks all three surfaces at once: the id the climate reports, the 1.7 the `Weather` resource carries, and the map's light marked stale -
then a second fog update that changes nothing and leaves the light alone.

## The weather's one reach into the map (round 512)

`weather.cpp:938-941` is three lines and the only place weather touches the map: when the sight penalty changes, every level's transparency cache is dirtied and the seen cache with
it. The port's manager reports `Change::Weather { sight_changed }` and `Map::on_weather_change` receives it.

The distinction the reference draws is the sight **penalty**, not the weather: `sunny` to `clear` is a different sky with the same absorption and costs nothing, while `clear` to `fog`
dims every ray on the level. A flush that fired on every change would be correct and would redraw the world every time the weather moved; the gate pins both directions - an
unchanged penalty leaves the light alone, a changed one rebuilds it exactly once.

A flush is a **note, not a construction**. `Level::mark_weather_changed` sets the stale flag and the next read rebuilds, which is the reference's ordering too: the weather update
cannot cast light, and a rebuild inside it would have nowhere to put the result.

**The trap this round set for itself:** the first version of the invalidation gate was written but never registered in the suite, so the run came back green against methods that did
not exist. Registering it turned the run red immediately - a reminder that a green suite only proves what the suite actually contains.

**Declared gap, unchanged:** the reference's flush loops over `-OVERMAP_DEPTH..=OVERMAP_HEIGHT`; the port's map holds one level, so the loop is one mark until the levels exist.

## The weather as a tick (round 511)

`weather::System::tick` is the first thing in the port that is a **tick**: one call that computes the point from the generator, decides the sky from the data's own conditions, and
books the next change with the manager.

> Driven only by the reference's recorded dice and two sector draws, it reproduces the recorded day hour by hour: the weather id, the temperature to a millionth of a kelvin, the
> humidity, the pressure, the wind power, the wind direction, and the turn the next change is due.

Two things had to be got right for the dice to line up, and both are behaviours rather than formatting. **A strike costs nothing**: the reference computes the point inside its due
check, so an hour that does not recompute rolls no dice at all - which is why the first recorded hour has an empty trace and why the port cannot "warm up" its generator on a turn
that did not happen. And **a calm hour draws no direction check**, because `one_in` short-circuits at a chance of one or less.

The gate also settled a trap worth naming: `WeatherType::default()` used to carry durations of **zero**, while the loader defaulted to five minutes. Anything built from
`..Default::default()` - a test, or a mod that only sets a name - would have been a weather that ended the instant it began. The struct's default now carries the data's default, and
the tick fails loudly if it ever drifts again.

**What is left of the weather:** nothing behavioural. The reference's update also raises a UI distraction, cancels a wait activity and reactivates effect-on-conditions, and those
belong to the systems that own them; the port reports the change so they can act on it. The transparency flush itself is the map's job and is the next piece of M2.

## The manager, and the one place weather touches the map (round 510)

`weather_manager::update_weather` (`weather.cpp:901-947`) is ported as a value: the weather id, the point it was decided from, the turn it is next due, and the lightning flag. It
recomputes only when there is no weather yet or the turn has reached the next due turn, and the order is the reference's - store the point, decide the id, clear the lightning, then
roll the next change from the bounds of the id that was **just chosen**.

Two details that only a test can pin:

- The reference's null id starts with a sight penalty of zero, so the **first** update always reports a sight change. That is why a map is drawn clean at the start of a game.
- A new weather with the **same** sight penalty does not dirty the caches. `sunny` and `clear` both absorb 1.0, so the recorded day changes sky at the twelfth hour and flushes the
  transparency caches exactly **once**, at the start.

The port returns what changed rather than acting on it: `Change::Weather { sight_changed }` is the signal the caller turns into "mark every z-level's transparency cache dirty and the
seen cache too", which is the reference's `for( i = -OVERMAP_DEPTH; i <= OVERMAP_HEIGHT; i++ )`.

Against the recorded day the manager walks all twenty-four hours, lands on every recorded next turn, and flushes exactly once - with the id taken from the recording, because the
decision has its own gate.

**Deliberately not ported, because they are not weather:** the reference's update also raises a UI distraction, cancels a wait activity, and reactivates effect-on-conditions. Those
belong to the systems that own them, and the manager now reports the change that would trigger them.

## The day, the night, and a weather decision with nothing left out (round 509)

`is_day` (`calendar.cpp:368`) is `sun_altitude >= sunrise_angle`, and the angle is **minus one degree**, not zero: the game lets the day begin while the sun's centre is still below
the horizon, so a port that used the horizon would move every dawn and dusk by minutes. `is_night` is not the negation either - it is `sun_altitude <= -6`, so there is a twilight band
the game calls neither.

The astronomy underneath was already ported and gated against a year of samples (`corpus/astronomy/sun.json`), so this round only added the two predicates - and then closed the
weather's **decision** end to end:

> the data's own seventeen conditions, three levels of data-defined functions, the ported sun, the ported sort, and the condition loop, against the twenty-four recorded hours -
> every hour decides the sky the reference decided.

That gate cannot pass by luck. At hour twelve the humidity is still 69.91 (below sunny's seventy) and the pressure is 1029 (above its 1020), so the *only* thing that turns the answer
from `sunny` into `clear` is the sun going down - which is exactly what the recording shows and what the port reproduces.

**What is left of the weather:** the manager - the object that holds the id, the point and the schedule between turns, calls the decision, and clears the map's transparency caches
when the sight penalty changes.

## The expression language, data-defined functions, and the conditions they decide (round 508)

Three new pieces, each read from the data rather than transcribed from it:

| Piece | What it is | Where |
|---|---|---|
`math_expr` | Tokenizer and recursive-descent evaluator: numbers, single-quoted strings, `+ - * / %`, six comparisons, calls, unary minus | `crates/content/src/math_expr.rs` |
`jmath` | `jmath_function` entries - a body string with `_0`, `_1` bound - evaluated against an outer context | `crates/content/src/jmath.rs` |
`condition` | `and` / `or` / `not` / a named condition / `math` | `crates/content/src/condition.rs` |

Two decisions worth recording. **An unknown name is an error, never a zero** - a silent zero makes every condition false, every sky clear, and the failure invisible. And a context
built on another context passes errors through rather than turning them into "unknown name": the first attempt swallowed the inner failure, so a missing `log` was reported as a
missing `dew_point_factor`, which is the sort of error message that wastes an afternoon.

The end-to-end gate is the payoff. The **data's own** seventeen conditions run against the twenty-four recorded hours: three levels of data-defined function (`dew_point_factor` →
`dew_point` → `dprhx` → `weather('humidity')`), no precipitation condition ever holds, and `sunny` holds on hours zero through twelve and not after - because its humidity bound is
seventy per cent and the recording crosses it between the twelfth hour (69.91) and the thirteenth (71.01). That is a real boundary at a real number, not a fixture that agrees with
itself.

**Not yet:** `is_day` (the sun's altitude is a port of its own, and the sunny condition is the only one that needs it) and `distance`/`value_or`/the portal-storm variables, which no
shipped condition reaches without a portal storm.

## The condition loop and the schedule (round 507)

Two rules, both gated on transcribed cases because neither needs a world to be wrong.

**The condition loop** (`weather_gen.cpp:196-215`) starts at `clear`, walks the sorted types, and replaces the answer whenever a type's own requirements are met *and* its condition
holds - so the **last** match wins. The conditions are dialogue expressions and are not ported, so the loop takes the evaluation as a parameter. That is not a way around the gate: the
loop's own rules are what the tests pin, and a chain that cannot start cannot finish even when every other condition holds.

**The schedule** (`weather.cpp:901-925`): recompute when there is no weather yet or the turn has reached `nextweather`, then set `nextweather = turn + rng(min, max)`. The boundary is
inclusive, and every recorded hour's distance to its next change is exactly the duration it rolled.

That last check has a wrinkle worth keeping: every type in the shipped data takes the **default** five minutes, so all twenty-four draws are 300 - a constant that is a default rather
than a rule. The test asserts the draw lies inside the type's own range and that the range's lower bound is that default, so a future type that sets a real range will not quietly
inherit the arithmetic.

## The order weather is decided in (round 506)

`get_weather_conditions` walks a sorted list and lets the **last** type whose condition holds win, so the order is behaviour: two weathers that could both apply are decided by which
comes later. `--dump-weather-order` now writes both orders in one file - the registry's own and the sorted one - and the port reproduces the sorted one exactly, all twenty types.

The sort is `std::sort` on `priority`, which is **not stable**, but the only tie in the data is `mist` and `fog` at 15, and they come out in declaration order. So the port sorts stably,
and the test proves the stability rather than assuming it: reorder the registry and the tied pair follows, which a sort by priority-then-name would not do.

The two code-made types are in there too - `null` at -1 and `clear` at 0 - which is why the registry holds twenty while the data declares eighteen.

**What this unlocks:** the condition loop now has its list. What it does not have is the conditions themselves: each type's `condition` is a dialogue expression, so the next step is
either the condition machine or a call gate that takes the evaluation as a parameter and transcribes a chain or two.

## The wind's direction, and a calm that redraws itself (round 506)

The direction is a `static int current_winddir = 1000` in the reference - process state that survives every call - and the port now models it as a
`WindDirection` value with `Option<i32>` degrees, because a system that remembers something between turns should say so in its type.

The rule is `one_in( W * 2160 )`, and `one_in` returns true **without rolling** whenever its chance is one or less. Since the wind is never negative, that
means a dead calm redraws the direction every time it happens. The recorded day shows it exactly: of twenty-three hours, one has wind zero, that hour rolls
no direction check at all - and it is the only hour whose direction changes, 202 to 315.

So the gate is a predicate rather than a value: the hours the port would redraw are the hours the reference's direction moved, and nothing else. The
sector-to-angle conversion is pinned separately on transcribed cases (sector 9 is 202 degrees, not 203, because `22.5f` is truncated and not rounded),
and the unset-sentinel case has its own test because no recorded day can show it - its first hour was decided before recording began.

**Left in the weather generator:** `get_weather_conditions`, which picks an id from a `w_point` by walking `sorted_weather` and evaluating each type's
dialogue condition, and then the manager's turn loop.

## Humidity, pressure, and the wind's dice (round 505)

Humidity and pressure come from the same noise field the temperature uses, at seeds `+101` and `+211`, and both match all twenty-four recorded hours to within a millionth. They are
doubles in the reference, unlike the temperature, so there is no storage rounding to explain away: agreement here is agreement with the formula.

The wind is the first generator number that rolls, and it is where the oracle earned its keep. `weather_gen.cpp:160-164` asks for three dice - a multiplier, the exponent the pressure is
raised to, and the seasonal term's multiplier - and the port takes all three from the caller. The harness now writes a `mark weather.hour.N` before each update, so the dice trace
(`CDDA_RNG_TRACE`) labels which draws belong to which hour:

```
mark weather.hour.1
int 1 2 2        the multiplier
int 9 80 46      the pressure exponent
int 1 2 2        the seasonal multiplier
int 0 6479 4432  the wind direction's check, one_in(W * 2160) with W = 3
int 300 300 300  the weather's duration
```

Every recorded hour's wind power now reproduces exactly, and the test asserts the **order** the port asks for the dice, not just the answer - a port that drew its own would be
comparing luck, and a port that drew them in another order would agree by accident on an hour or two and then diverge.

**Still open in the generator:** the wind direction (`get_wind_direction` and `convert_winddir`, plus the `static current_winddir` the reference keeps between calls), the
`get_weather_conditions` loop that turns a `w_point` into a weather id through each type's dialogue condition, and the manager's turn loop. The fixture already records the direction,
the id and the rolled duration, so each has its target.

## The weather generator's temperature, and three storage details (round 504)

The port reproduces all twenty-four recorded hours of the reference's weather to within a millionth of a kelvin, from the recorded intermediates. Three things had to be right before
that was true, and none of them is the formula:

| Detail | What it costs if missed |
|---|---|
`raw_noise_4d` is **f32**, in the reference's own operation order | the tables were copied from `simplexnoise.h` by a script, because a wrong number in a 512-entry permutation gives a world that still looks like a world |
`units::temperature` is a **float**, not a double | at 275 K a float's spacing is 0.00003 K, larger than every other arithmetic difference in the port |
`from_celsius` adds the float `273.150f` to the **double** and narrows afterwards | narrowing first is off by one ulp on some hours - exactly one, out of twenty-four |

The third one only showed up after the first two were fixed, which is the useful part: a gate at a thousandth of a kelvin would have forgiven all three.

**What is ported:** the year and day cosines, the baseline, the noise field, and the temperature. **What is not:** humidity and pressure (the same field at seeds +101 and +211), the
wind (which needs `rng(1, 2)`, `rng(9, base_wind_distrib_peaks)` and `one_in(W * 2160)`, so it arrives with the caller's dice), the condition table that turns a `w_point` into a
weather id, and the manager's turn loop. The recorded series has a field for each of them, so each arrives with its own target.

## The weather system's oracle, before its port (round 503)

`--dump-weather-series` runs the reference hour by hour and writes what the weather system actually did: the weather id, the generator's temperature, humidity, pressure, wind direction
and wind power, and the duration it rolled before the next change. Two runs of one seed are byte-identical, which is what makes it a gate: the reference's weather is not random from run
to run, only from seed to seed.

`corpus/weather/series-24h.json` holds the first fixture - 24 hours on seed `weatherwatch`, `sunny` giving way to `clear` - and it was recorded before a line of the port's weather
system exists, so the system arrives with its target already in hand rather than after the fact.

## Nearly four of every five overmap ids are a rule (round 503)

The reference holds 6,269 overmap terrain **types** and 24,218 **instances**. The difference is not data: a rotatable type becomes four instances
(`_north`, `_east`, `_south`, `_west`), a linear type becomes sixteen (one per line shape), and everything else stays one (`overmap.cpp:989-999`).
Measured against a fresh dump: 5,903 rotatable types, 16 linear, 350 plain, and 23,612 + 256 + 350 = 24,218 instances.

The port's `overmap.rs` already derived this - the digest gate had been comparing both registries - so the new work is an oracle and a sharper
test, not a loader. `--dump-overmap-terrains` writes every instance with its type and the two facts that decided it, and the gate now says *why*
when one disagrees: which type, what the port thought, what the reference said. 24,218 ids and 6,269 per-type flag pairs, both exact.

A duplicate module was written during this and deleted: `overmap_terrain.rs` re-implemented `overmap.rs`. Two copies of a derivation rule is how a
port ends up disagreeing with itself, and the digest gate would have gone on passing while they drifted.

## The three shapes of a palette reference (round 502)

A chunk's `palettes` list holds three different things, and the port now keeps them apart instead of flattening the list to strings.

| Shape | Count in the data | What the port does |
|---|---|---|
`"shelter"` | most | gathers its terrain symbols |
`{"distribution": [["cabin_palette", 3], ["cabin_palette_abandoned", 1]]}` | 32 | takes the first candidate; the weights stay in the structure for a caller with dice |
`{"param": "food_type"}` | 272 | resolves to nothing, because the overmap special owns the choice |

A palette's terrain value can also be **a parameter with a fallback** - `{"param": "exterior_wall_type", "fallback": "t_brick_wall"}` - and the fallback is what the format says to draw when
nobody supplies the parameter. That is why the shelter still disagrees with the reference on its `#` squares: the reference was handed `t_concrete_wall` by the overmap special that placed
the shelter, and the port, having no parameter source, draws the fallback.

## One direction, three masks (round 502)

A mapgen's `terrain` is written **symbol first** - `".": "t_floor"` - and the port read it id-first for many rounds. Every symptom wore a different mask:

| Symptom | Read symbol-first | Read id-first |
|---|---|---|
a chunk with no inline terrain needed a `fill_ter` | yes, but only for furniture squares | every square fell back to the fill |
the symbol gatherer produced 33 nonsense entries | 122 real symbols | single letters where terrain ids belong |
stairs in a temple came out as plain rock floor | `'>'` is `t_stairs_down` | `'>'` is not a symbol at all |

All three were the same bug. Two fully-inline chunks now match the reference on **every square the port draws**, under one of four quarter turns, and a transcribed three-square case pins
the direction so it cannot flip back silently.

**What the gate covers now:** `temple_chunk` (4 terrains, 48 drawn squares) and `billboard_chunk` (3 terrains) agree exactly with the reference's own mapgen run. The shelter stays at
334 of 385 with every disagreement a `#` wall square, because those walls arrive through a palette that takes **parameters** and the reference then connects neighbours into
directional ids - both still missing, both named.

## The mapgen oracle, and three things it caught (round 501)

The reference can now be asked what a **mapgen** draws, with no world, no player and no seed: `--dump-mapgen <key> <file>` runs one mapgen and writes the terrain and furniture id of
every square. The first target, `shelter`, produced a 24 by 24 chunk with thirteen terrain kinds and four furniture kinds.

Getting there exposed three of my own mistakes, each found by a measurement rather than by re-reading my code.

| Mistake | What it looked like | What was true |
|---|---|---|
`--dump-map` nested inside the `--dump-frame` branch | "the map dump hangs" | it never ran; the game fell through and rendered its UI forever |
the chunk was named from `here.get_abs_sub()` | a chunk three quarters blank | that is the **map buffer's corner**, five submaps from the player |
palette terrain was read id-to-symbols | 33 nonsense symbols where 122 belong | a palette's `terrain` is **symbol-to-id**, the opposite direction from a mapgen's own map |

**And a third model error the gate itself found:** a symbol missing from the terrain maps is not an error. `furniture`, `items` and `traps` carry their own symbol maps, and a square
that wears one keeps the chunk's `fill_ter`. The shelter is the proof: `fill_ter` of `t_floor`, no inline terrain at all, and rows full of furniture symbols.

**The gate's claim, exactly.** The port draws 385 of the 576 squares; 334 match the reference under the best quarter turn, and **every** disagreement is a `#` square. Those are the
two mechanisms still missing and both are named rather than tolerated: the shelter's walls arrive through a palette that takes **parameters** (`parametrized_walls_palette` with
`wall := t_concrete_wall`), and the reference then connects neighbouring walls into directional ids (`t_concrete_wall_w`). The test asserts that every disagreement is one of those
squares, so any other symbol disagreeing fails it.

## A chunk is drawn from its palettes and its own map (round 496)

The generator's draw step, gated over the whole data rather than a sample:

| Rule | Reference | Port |
|---|---|---|
a symbol resolves through the entry's own `terrain` first | `mapgen.cpp`'s draw | `draw_chunk` |
then through its named palettes, transitively | same | `palette_symbols` |
a later palette wins over an earlier one | same | names walked in reverse |
a space draws nothing | same | `Option` per square |
an unknown symbol is an error naming the symbol | debugmsg | `Err` |

Run over the **2,411** chunks that draw a grid and name palettes - the ones a resolver reading `object.terrain` alone would leave silently blank - and every one draws, over a
hundred thousand squares.

**What is still missing before a chunk is compared with the reference dump:** the twenty-six generators registered in code have no data to draw from at all; the post-draw placements
(furniture, items, nested chunks, `fill_ter`) are unwritten; and predecessor mapgens, of which the data has one. Until those exist the port can produce a chunk's *terrain* from the
data - the largest piece - but not a chunk the game would agree with square for square.

**And the omission that nearly went out:** the first attempt at this entry edited a heading that does not exist, so the write succeeded and the commit found nothing to do. A commit
that says "nothing to commit" is the cheapest possible check that the edit did not happen.
