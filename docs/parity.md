# Parity ledger

The single place where port status lives. One row per system: what it is, which
phase it belongs to, and the gate that proves it matches the reference. Update it
in the same change that ports the system.

Legend for **Status**: `not started` · `porting` · `ported` (gates red) ·
`gated` (gate green, evidence linked) · `declared divergence` (differs on purpose,
reason recorded in [04-validation.md](04-validation.md)).

Nothing is `gated` without a link to the run that proved it.

---

## P0 — Foundation

| System | Status | Gate | Notes |
|---|---|---|---|
| Coordinates, origins and scales | done (P0 scope) | M0 | 13 tests. Projection floors toward negative infinity (`coordinates.h:601-644`) and the vertical axis is never scaled. The origin/scale legality matrix is in the type system: six origins, four legal `project_remain` targets, `combine` only where the fine origin is the one the coarse scale names, and a segment that divides terrain tiles but not overmaps. Distances, midpoint and hashing match `coordinate_test.cpp` |
| Units and quantities | done (P0 scope) | M0 | 23 tests; 83 of the reference's 202 assertions reproduced, the rest being time/JSON/option-driven conversions that belong to later tasks. Rounding truncation rules pinned from the source |
| Calendar and time | done (P0 scope) | M0 | 28 tests = every assertion in `tests/calendar_test.cpp`; sun and moonlight cross-checked against the reference's own formulas over 309 configurations (max deviation 2e-16 rad) |
| Random numbers | done (P0 scope) | M0 | Engine stream matches the reference draw for draw over 4 seeds ([corpus/rng/engine.csv](../corpus/rng/engine.csv)). The wrappers the game actually calls — `rng`, `rng_float`, `normal_roll`, `rng_normal`, `rng_bits`, `one_in`, `x_in_y`, `dice`, `roll_remainder`, `random_entry`, `rng_sequence`, the exponential rolls and the world-seed hash — reproduce 93 calls bit for bit against the same libc++ ([corpus/rng/wrappers.csv](../corpus/rng/wrappers.csv), generator recorded beside it). `normal_roll_chance` is ported and held to 4e-8 rather than to equality, because the error function is the platform's; the centre case is exact. See [03-decisions.md](03-decisions.md). Not ported: the chi-squared roll, which needs libc++'s gamma sampler - and it is not dead code, it drives NPC mutation timing (`src/npc.cpp:663`), so that path cannot be claimed until it lands |
| Identifiers (string, int, type) | done (P0 scope) | M0 | 17 tests, 88 assertions: interned string identity, integer ids as ordinal caches with version invalidation, invalid use returns the dummy rather than panicking; misuse of an int id without its registry fails to compile |
| Parse cache | done (P0 scope) | M0 | 5 tests: a hit avoids the parse, a changed source is a miss, a damaged entry is a miss and never an error, unused entries are pruned at open. All 6,220 files read identically cold and warm; cold 11.0 s, warm 3.5 s |
| Registry machinery | done (P1-T1 core) | M1 | Abstracts kept apart but usable as bases, deferred `copy-from` retried at finalisation, cycles reported as unresolved ids with no recursion, duplicate ids keeping their position. All 8,743 `copy-from` chains in the core content resolve except 25 overmap-terrain objects whose base the map generator creates |
| Terrain and furniture | first family done | M1 | All 1,769 prototypes match the reference on name, symbol per season, colour, move cost, light, coverage and the full flag set — including inheritance, `flags` replacement, `extend`/`delete`, and both null prototypes |
| Traps | done | M1 | All 125 match on name, glyph, colour, visibility, avoidance, difficulty, radius, benign-ness, invisibility and flag set |
| Materials | done | M1 | All 193 match on name, density, chip resistance, breathability rating and the three heat constants |
| Field types | done | M1 | All 145 match, including the rule that a level inherits the level before it inside the same object |
| Item types: identity, measure, category, flags, names | done (first slice) | M1 | All 9,912 match on id set, name, plural, glyph, colour, weight, volume, price, category, flags and materials — including armour that derives its materials from its per-part entries, and materials added or dropped through `extend` and `delete`. No outstanding notes |
| Monster types | done (first slice) | M1 | 1,154 monster types match on id set, name, glyph, colour, body type, hp, speed, difficulty, melee skill, dodge, morale, aggression, weight, volume, species, categories and materials, including the null type and the dormant twins the reference generates |
| Generated prototypes | in progress | M1 | the ids the reference builds for itself are reproduced: the null monster and its dormant twins, 311 inline enchantments, 113 martial-art buffs, 239 turret parts |
| Content registry | in progress, 86 of 90 registries | M1 | 86 of 90 registries hold exactly the reference's ids, and **no invented ids anywhere**. The gate runs on every build and names the four gaps with their exact shortfalls, so a regression in any of the other 86 fails it and finishing a gap means editing the list |
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
| Prototype registry machinery | done (P1-T1 core) | Prototype digest (M1) | Load, inherit, finalise: abstracts kept apart but usable as bases, `copy-from` retried at finalisation, cycles reported rather than recursed, duplicate ids keeping their position, the factories' null prototypes created. 8 tests ([factory.rs](../crates/content/tests/factory.rs), [rules.rs](../crates/content/tests/rules.rs)) |
| Content type loaders (160 types) | in progress, 7 families | M1 | Terrain and furniture, traps, materials, field types, item types, monster types, plus the generated-prototype families. Each is held to a field-by-field corpus dumped by the reference. See [06-inventory.md](06-inventory.md) for the rest |
| Inheritance and merge operators | done | M1 | `copy-from`, `extend`, `delete`, `relative` and `proportional`, with the two orders the reference uses: readers check proportional before relative, `assign` checks relative first. All 8,743 `copy-from` chains in the core content resolve. [inheritance_corpus.rs](../crates/content/tests/inheritance_corpus.rs) |
| Mod discovery and load order | not started | M1 | User-ordered list, not a topological sort |
| Mod dependency graph and cycles | not started | M1 | |
| Save reading and writing | not started | M1 | Scope depends on blocker B2 |
| Save migrations | not started | M1 | 14 content-level migration types plus per-field fix-ups |
| Compression and archives | not started | M1 | Dictionary content must match bit-for-bit |
| Map memory and seen-tile persistence | not started | M1 | |
| Overmap terrain ids | rule proven, loader not written | P1 | The `overmap terrain type` registry is exactly the distinct `type: overmap_terrain` ids in the load set - 6,269 measured against 6,269 reported. The `overmap terrain` registry is those types expanded into rotation and line-drawing variants; the shape is known and the rotation rule is proven exactly: `!NO_ROTATE && !LINEAR` with flags inherited through `copy-from` and `delete` applied last, giving 5,901 rotatable types with no disagreement against the dump, plus 256 line-drawing shapes for the 14 `LINEAR` types. See [15-overmap-terrain-ids.md](15-overmap-terrain-ids.md) |
| Weighted lists | done (whole-number lists) | P1 | `WeightedIntList` reproduces `src/weighted_list.h`: a non-positive weight is refused, `add_or_replace` with zero deletes, a single entry short-circuits, and the draw is `(randi % total) + 1` against the running total. Picking from an empty list still costs one draw, because the reference evaluates `rng_bits()` before it checks the total - a port that returned early would put every later roll out of step. The fraction-weighted list scales the draw across the whole 32-bit range instead of wrapping it modulo the total, and has no one-entry shortcut, so it is ported as its own type rather than a flag |
| Line walk | done (2D walk) | P2 | `line_to` reproduces the reference's Bresenham step for step, including the starting error term that decides a tie; 14,400 target/offset pairs land on their target (`tests/line_test.cpp:416`), and the degenerate line yields one cell because the reference's walk enters its body once. Not ported: `bresenham` over 3D bubble points, `squares_closer_to_test` |
| Test regime | in place | every gate | Corpus gates carry receipts (what was compared, how much, how long); failures name the id and field and count the differences by kind; the id gate asserts the exact size of each known gap rather than being skipped. Rules: [14-test-porting.md](14-test-porting.md); inventory: [13-test-inventory.md](13-test-inventory.md) |
| Oracle harness modes | done (record/replay/state/digest) | Gate 0 | Replays a recorded session with no terminal and reaches a byte-identical state; see [11-oracle-harness.md](11-oracle-harness.md) |

## P2 — World and environment

| System | Status | Gate | Notes |
|---|---|---|---|
| Tile storage and uniform regions | not started | Environment replay (M2) | |
| World submap owner and bubble view | not started | M2 | Ownership decided once |
| Per-level caches and dirty flags | not started | M2 | |
| Light and illumination | not started | M2 | Four quadrants per square, 21 levels |
| Vision and visibility | not started | M2 | |
| Fields and field processors | not started | M2 | 21 data-gated processors |
| Scent | not started | M2 | Already a dense grid |
| Sound events | not started | M2 | Transient, no world storage |
| Temperature | not started | M2 | Per-turn cache |
| Weather | not started | M2 | Tuning constants transcribed exactly |
| Traps | not started | M2 | |
| Zones | not started | M2 | |
| Falling and explosions | not started | M2 | |

## P3 — Items

| System | Status | Gate | Notes |
|---|---|---|---|
| Item state | not started | Item replay (M3a) | |
| Contents, pockets and containment | not started | M3a | |
| Item location and references | not started | M3a | Replaced by generational handles |
| Item groups and spawn tables | not started | M3a | Recursive polymorphic tree |
| Item prototypes | not started | M3a | |
| Inventory | not started | M3a | |
| Crafting and recipes | not started | M3a | |
| Requirements and qualities | not started | M3a | |
| Constructions | not started | M3a | Named special functions |
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
| Activity actors | not started | Activity replay (M3d) | 69 classes |
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
