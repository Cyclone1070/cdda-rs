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
| Random numbers | in progress | M0 | Engine stream matches the reference draw for draw over 4 seeds ([corpus/rng/engine.csv](../corpus/rng/engine.csv)); the library distributions are being transcribed from libc++ and checked against the same corpus |
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
| Content registry | in progress, 86 of 90 registries | M1 | 86 of 90 registries hold exactly the reference's ids, and **no invented ids anywhere**. Four remain: inline effect-on-conditions (their slot owners are per-type), overmap terrain and its types, and overmap specials — all three built from mapgen |
| JSON reading | in progress | M0 | Reader matches the reference on all 6,220 data files (accept/reject and structural digest); duplicate keys are a hard error, member order is sorted, trailing content is ignored |
| JSON writing | in progress | M0 | Writer matches the reference byte for byte on 127 sample files, including its fixed 6-decimal float format |
| JSON parse cache | not started | M0 | |
| Options table | in progress | M0 | 276 options extracted mechanically: 188 from `options.cpp`, 88 from JSON. Four are broken in the reference itself (see 03-decisions.md) |
| Paths and filesystem roots | done (P0 scope) | M0 | Seven logical roots, 12 tests over the platform x build-option matrix; the trailing-separator bug that bit us earlier is now a pinned behaviour |
| Text lookup and formatting | done (P0 scope) | M0 | Formatter agrees with all 339 reference format rows and its error cases; 71/71 reference string assertions; 11,058 message ids extracted and the lookup returns each unchanged with no catalogue. Byte/column width and C-locale quirks recorded rather than fixed (B4) |
| Colours | done (P0 scope) | M0 | 23 tests: packed attribute layout, the 145-name table, pair table, name parsing, highlights and inversion. RGB exists only in the graphics build and is ported from there |
| Containers and hashing | not started | M0 | Stable-address pools, dense bit sets |
| Diagnostics and debug levels | done (P0 scope) | M0 | 10 tests: level and class filtering, log formatting, the error-observed flag, prompt as a caller decision, assertion policy |

## P1 — Content and persistence

| System | Status | Gate | Notes |
|---|---|---|---|
| Prototype registry machinery | not started | Prototype digest (M1) | Load, inherit, finalise, consistency-check |
| Content type loaders (160 types) | not started | M1 | See [06-inventory.md](06-inventory.md) for the full list |
| Inheritance and merge operators | not started | M1 | `copy-from`, `extend`, `delete`, `relative`, `proportional` |
| Mod discovery and load order | not started | M1 | User-ordered list, not a topological sort |
| Mod dependency graph and cycles | not started | M1 | |
| Save reading and writing | not started | M1 | Scope depends on blocker B2 |
| Save migrations | not started | M1 | 14 content-level migration types plus per-field fix-ups |
| Compression and archives | not started | M1 | Dictionary content must match bit-for-bit |
| Map memory and seen-tile persistence | not started | M1 | |
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
