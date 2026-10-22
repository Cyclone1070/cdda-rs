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
| Prototype registry machinery | done (P1-T1 core) | Prototype digest (M1) | Load, inherit, finalise: abstracts kept apart but usable as bases, `copy-from` retried at finalisation, cycles reported rather than recursed, duplicate ids keeping their position, the factories' null prototypes created. 8 tests ([factory.rs](../crates/content/tests/content_suite/factory.rs), [rules.rs](../crates/content/tests/content_suite/rules.rs)) |
| Content type loaders (160 types) | in progress, 7 families | M1 | Terrain and furniture, traps, materials, field types, item types, monster types, plus the generated-prototype families. Each is held to a field-by-field corpus dumped by the reference. See [06-inventory.md](06-inventory.md) for the rest |
| Inheritance and merge operators | done | M1 | `copy-from`, `extend`, `delete`, `relative` and `proportional`, with the two orders the reference uses: readers check proportional before relative, `assign` checks relative first. All 8,743 `copy-from` chains in the core content resolve. [inheritance_corpus.rs](../crates/content/tests/content_suite/inheritance_corpus.rs) |
| Mod discovery and load order | not started | M1 | User-ordered list, not a topological sort |
| Mod dependency graph and cycles | not started | M1 | |
| Save reading and writing | not started | M1 | Scope depends on blocker B2 |
| Save migrations | not started | M1 | 14 content-level migration types plus per-field fix-ups |
| Compression and archives | not started | M1 | Dictionary content must match bit-for-bit |
| Map memory and seen-tile persistence | not started | M1 | |
| Overmap terrain ids | done | P1 | The `overmap terrain type` registry is exactly the distinct `type: overmap_terrain` ids in the load set - 6,269 measured against 6,269 reported. The `overmap terrain` registry is those types expanded into rotation and line-drawing variants; the shape is known and the rotation rule is proven exactly: `!NO_ROTATE && !LINEAR` with flags inherited through `copy-from` and `delete` applied last, giving 5,901 rotatable types with no disagreement against the dump, plus 256 line-drawing shapes for the 14 `LINEAR` types. The type registry is **exact**: 6,269 against 6,269. **24,218 of 24,218 exact**, with rotations, the sixteen line-drawing shapes per `LINEAR` type and plain types all matching. The last eight came down to flag semantics: a plain `flags` member *replaces* what inheritance supplied, `extend.flags` adds, `delete.flags` removes last. See [15-overmap-terrain-ids.md](15-overmap-terrain-ids.md) |
| Effect-on-conditions | done | P1 | **913 of 913 exact**, 692 defined and 221 inline. An inline effect is registered under the `id` inside the slot it sits in (`activated_eocs`, `true_eocs`, `result_eocs`, `eocs` and the rest, `effect_on_condition.cpp:119-133`). Collection stops at the slot boundary: a sound effect inside a dialogue effect also has an `id`, and collecting it invented seven effects that do not exist |
| Overmap specials | done | P1 | **742 of 742 exact.** `overmap_special` and `city_building` are one namespace, and the `FakeSpecial_<id>` ids the reference mints for single-tile buildings named by a region but defined by no special are generated from the `city` lists of `region_settings`. `overmap_special` and `city_building` are one namespace - the reference's `city_buildings::load` is an alias for `overmap_specials::load` - so both are read into one set: 707 of 742. What is missing is exactly the 35 `FakeSpecial_<oter>` ids the reference mints for single-tile city buildings a region names but no special defines. See [15-overmap-terrain-ids.md](15-overmap-terrain-ids.md) |
| Fields as things in the store | done (P2-T0c) | P2 | A patch of smoke is an `Entity` with a square, a strength and an age; `process_fields` walks the store in the reference's order (x outer, y inner, insertion order within a square) and despawns the dead. **The per-square container in the map layer is deleted** - the map no longer knows what smoke is on it, which is the point: one authority, not two |
| Gas spread | **partial** (arithmetic done, direction not) | P2 | `gas_spread_to` reproduces `map::gas_spread_to` (`src/map_field.cpp:250-273`): gas thins as it moves and gives up the fraction of its age equal to age divided by intensity; a square already holding the gas gets one thicker instead of a second field. Four tests transcribed from that function, including the guard that a single unit of gas does not spread and the check that the arithmetic draws no dice - the dice belong to the direction choice, so drawing one here would shift every later roll. **Not ported:** the direction itself, which needs wind (`get_local_windpower`) and `rng(1, 100 - windpower) > percent_spread` |
| Sunlight as a system | done (P2-T0c) | P2 | `cast_sunlight` writes the sun's answer into the same [`Light`] resource the torches write, so daylight and lamps meet in one place. Four tests: open sky is bright at noon and black at midnight; a room with no sky depends on its torch; a torch outdoors never makes a square darker and never doubles it, because sources take the brighter corner rather than summing (`update_light_quadrants`); and clearing the answer leaves nothing, because a stale answer is not an answer. The pipeline is clear, then sun, then sources, and a test caught the reason: a merge by maximum will happily keep last turn's brighter value and hide a dimming |
| Light as a system | done (P2-T0c) | P2 | `cast_lights` reads the store - terrain from the map resource, sources and smoke as components - and writes a [`Light`] resource of four quarters per square. The smoke test that used to live in the map layer now lives here: a torch lights a room, and an entity standing between torch and wall dims the wall. Terrain transparency is still a two-valued rule (solid or open air) because per-terrain transparency has not been ported; the ledger says so rather than pretending otherwise |
| Point light sources | done (memoised transparency) | P2 | `Level::add_light_source` casts from a source with the ported caster and raises the quarter facing it by `sight_calc` of the luminance, falling off with distance. Five properties hold: nearer is brighter, the corner facing the lamp is the lit one, a wall shadows what is behind it, a source under `LIGHT_AMBIENT_LOW` casts nothing (its range is zero), and a second lamp raises a tile without doubling it. The ray accumulation the reference does is now ported too: `cast_light_values` averages transparency along each ray as it travels (`accumulate_transparency`), so smoke dims what is behind it - gated by a test that puts a band of smoke in front of a wall and requires the wall behind it to be darker. The sign is worth stating because it cost a round: the value is attenuation *per tile*, so open air is 2.3/72 = 0.032, smoke is 0.38, and higher means dimmer - while solid is 0.0 and special-cased as blocking |
| Daylight on the level | done | P2 | `Level::daylight` reproduces the ambient pass of `map::build_map_cache`: outdoors takes the sun in all four quarters, an indoor tile with open sky to its south lights only its south quarters and leaves the back of the room dark, a tile with no opening stays at the indoor level, and the same room is black at midnight. `outside` is asked tile by tile rather than decided here - it depends on terrain and roofs, which the level does not own |
| Astronomy | done (gated on 424 samples) | P2 | `sun_azimuth_and_altitude_in_year` reproduces `sun_azimuth_altitude` (`src/calendar.cpp:123-239`) to **1.4e-3 degrees** over 424 samples of a year; the residual is the reference's angle helpers rounding, since every intermediate matches to the printed digits. Two things the dump taught: the year length is an option, not a constant, so the function takes it (a four-day year is a different curve, not a scaled one); and a dump that does not record its calendar cannot be gated against - the harness now prints the year and season length, and exposing the intermediates (`--dump-astronomy` carries ra, dec, sidereal, hour angle) is what made a six-step chain debuggable instead of a black box |
| Astronomy (was) | **oracle ready, port pending** | P2 | `--dump-astronomy` writes 424 samples of the reference's `sun_azimuth_altitude` and `sun_light_at` across a year (`corpus/astronomy/sun.json`): altitude -69.83 to 68.57 degrees, light 0 to 125. The port of the altitude chain has not started, so nothing is claimed for it yet |
| Open-sky light | done | P2 | `open_sky_lit_level` joins the sun's curve to the level thresholds: at altitude -15 the curve gives 1.53 which is under `AMBIENT_LOW` and reads dark, at -9 it gives 4.24 which is merely hard to see by, and full daylight is far past lit. Two ported halves, no new numbers |
| Sunlight curve | done | P2 | `sun_light_from_altitude` reproduces `sun_light_at` (`src/calendar.cpp:381-402`): piecewise, not a cosine - exponential across the three dawns, exponential to the horizon, linear to 60 degrees, flat at 125 above it. Gated on twelve values computed from the expression and on monotonicity over the whole range, because a wrong branch boundary shows up as a dip at dawn. The *altitude* itself (astronomy: latitude, day of year, hour) is not ported yet |
| Daylight quarters | done | P2 | `FACING` and `light_in_from_outside` reproduce `dir_quadrants` and the ambient pass in `map::build_map_cache` (`src/lightmap.cpp:384-438`): a tile whose south side is open sky lights its two south quarters and leaves the north ones dark, which is why a doorway lightens the floor inside it and not the back of the room. Daylight raises quarters, never lowers them, so a lamp in one corner survives sunrise |
| Sight falloff | done | P2 | `sight_calc`, `sight_check`, `accumulate_transparency` and `update_light_quadrants` (`src/shadowcasting.h:92-116`). Light falls off as `1 / e^(transparency * distance)` - the Beer-Lambert law with the coefficients merged into a running average - only solid stops it, and two rays reaching one corner do not add because it is the same light however many ways it is computed |
| Four quadrants | done | P2 | `FourQuadrants` and `Quadrant` reproduce `src/shadowcasting.h:24-90`: four floats per tile indexed NE, SE, SW, NW in the reference's order, with `max`, `fill`, elementwise `multiplied` and `max_with`/`max_with_value`. A tile's light is as bright as its brightest quarter, which is what `lit_level` reads - a lamp around a corner still lights the floor you can see |
| Level tick | done | P2 | `Level::tick` is the one place per-turn work happens: it takes the clock and the rolls, does the field pass, and reports what it did. Light is deliberately not rebuilt there - a turn nobody looks at costs no light map, which is the reference's own lazy-cache behaviour. Pinned by replaying twenty-five turns on two identical levels and requiring the same answer every turn |
| Field processing | done | P2 | A turn ages every field on every fielded tile and takes them off when they die, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing. The visit order is the reference's: x outer, y inner (`src/map_field.cpp:168-170`), which is what makes the turn independent of the order fields were added - pinned by a test that builds two levels in opposite orders and compares the shared RNG stream afterwards, not just the intensities | A turn ages every field on every fielded tile and takes off the ones that died, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing; a tile that loses one field keeps the rest and moves its drawn type to what is left. What is not ported yet is the processors themselves - fire spreading, smoke drifting - which need the map to react to a field rather than merely to age it |
| Field layers | done | P2 | `FieldLayer` reproduces `field::add_field` (`src/field.cpp:158-182`): one entry per type per tile, merging sums intensities (clamped to the type's maximum), a dead entry revives at the *new* strength rather than the one it died with, the null type is refused, and the drawn type is the highest priority with ties going to the later arrival. Adding a type that is already there returns false - the difference between "this tile caught fire" and "the fire got stronger" |
| Field decay | done | P2 | `FieldEntry` reproduces `src/field.cpp:58-113`. Three rules that read like bugs and are not: intensity never falls below one, death is a separate flag, and a decay resets the age *and* the decay time so the next half-life is drawn afresh. The decay time itself is `now - age + half_life` where the age has already been ticked, so a field with a half-life of five decays on the fourth tick after it starts, not the fifth. Fire's own branch (`dice(2, age)` against the half-life) is ported but only its shape is tested: the exact draw belongs to the shared RNG stream and will be gated by a replay |
| Cached light | done | P2 | (the previous round's commit says 347 tests; the suite held 346 - 344 plus the two that round added) |
| Transparency memo | done | P2 | Whether a tile blocks light is worked out once per tile, not once per cast: two casts from nearby tiles pay only for the row and column the other never saw, and a write drops that one tile's entry rather than a whole window. The reference caches transparency per level; a per-tile memo is the same idea at a finer grain |
| Fields as things in the store | **in progress (P2-T0c)** | P2 | `Field` is a component and a patch of smoke is an `Entity` with a square, a strength and an age; `process_fields` walks them in the reference's order (x outer, y inner, insertion order within a square) and despawns the dead. Three tests hold it, including that a dying field leaves the store and that the walk order is the one the dice stream depends on. **The old per-square container in `Level` still exists and still feeds the light**, so the store and the container disagree for now - that is the next cut, and it is why this row says in progress rather than done |
| Point light sources | done (memoised transparency) | P2 | `Level::add_light_source` casts from a source with the ported caster and raises the quarter facing it by `sight_calc` of the luminance, falling off with distance. Five properties hold: nearer is brighter, the corner facing the lamp is the lit one, a wall shadows what is behind it, a source under `LIGHT_AMBIENT_LOW` casts nothing (its range is zero), and a second lamp raises a tile without doubling it. The ray accumulation the reference does is now ported too: `cast_light_values` averages transparency along each ray as it travels (`accumulate_transparency`), so smoke dims what is behind it - gated by a test that puts a band of smoke in front of a wall and requires the wall behind it to be darker. The sign is worth stating because it cost a round: the value is attenuation *per tile*, so open air is 2.3/72 = 0.032, smoke is 0.38, and higher means dimmer - while solid is 0.0 and special-cased as blocking |
| Daylight on the level | done | P2 | `Level::daylight` reproduces the ambient pass of `map::build_map_cache`: outdoors takes the sun in all four quarters, an indoor tile with open sky to its south lights only its south quarters and leaves the back of the room dark, a tile with no opening stays at the indoor level, and the same room is black at midnight. `outside` is asked tile by tile rather than decided here - it depends on terrain and roofs, which the level does not own |
| Astronomy | done (gated on 424 samples) | P2 | `sun_azimuth_and_altitude_in_year` reproduces `sun_azimuth_altitude` (`src/calendar.cpp:123-239`) to **1.4e-3 degrees** over 424 samples of a year; the residual is the reference's angle helpers rounding, since every intermediate matches to the printed digits. Two things the dump taught: the year length is an option, not a constant, so the function takes it (a four-day year is a different curve, not a scaled one); and a dump that does not record its calendar cannot be gated against - the harness now prints the year and season length, and exposing the intermediates (`--dump-astronomy` carries ra, dec, sidereal, hour angle) is what made a six-step chain debuggable instead of a black box |
| Astronomy (was) | **oracle ready, port pending** | P2 | `--dump-astronomy` writes 424 samples of the reference's `sun_azimuth_altitude` and `sun_light_at` across a year (`corpus/astronomy/sun.json`): altitude -69.83 to 68.57 degrees, light 0 to 125. The port of the altitude chain has not started, so nothing is claimed for it yet |
| Open-sky light | done | P2 | `open_sky_lit_level` joins the sun's curve to the level thresholds: at altitude -15 the curve gives 1.53 which is under `AMBIENT_LOW` and reads dark, at -9 it gives 4.24 which is merely hard to see by, and full daylight is far past lit. Two ported halves, no new numbers |
| Sunlight curve | done | P2 | `sun_light_from_altitude` reproduces `sun_light_at` (`src/calendar.cpp:381-402`): piecewise, not a cosine - exponential across the three dawns, exponential to the horizon, linear to 60 degrees, flat at 125 above it. Gated on twelve values computed from the expression and on monotonicity over the whole range, because a wrong branch boundary shows up as a dip at dawn. The *altitude* itself (astronomy: latitude, day of year, hour) is not ported yet |
| Daylight quarters | done | P2 | `FACING` and `light_in_from_outside` reproduce `dir_quadrants` and the ambient pass in `map::build_map_cache` (`src/lightmap.cpp:384-438`): a tile whose south side is open sky lights its two south quarters and leaves the north ones dark, which is why a doorway lightens the floor inside it and not the back of the room. Daylight raises quarters, never lowers them, so a lamp in one corner survives sunrise |
| Sight falloff | done | P2 | `sight_calc`, `sight_check`, `accumulate_transparency` and `update_light_quadrants` (`src/shadowcasting.h:92-116`). Light falls off as `1 / e^(transparency * distance)` - the Beer-Lambert law with the coefficients merged into a running average - only solid stops it, and two rays reaching one corner do not add because it is the same light however many ways it is computed |
| Four quadrants | done | P2 | `FourQuadrants` and `Quadrant` reproduce `src/shadowcasting.h:24-90`: four floats per tile indexed NE, SE, SW, NW in the reference's order, with `max`, `fill`, elementwise `multiplied` and `max_with`/`max_with_value`. A tile's light is as bright as its brightest quarter, which is what `lit_level` reads - a lamp around a corner still lights the floor you can see |
| Level tick | done | P2 | `Level::tick` is the one place per-turn work happens: it takes the clock and the rolls, does the field pass, and reports what it did. Light is deliberately not rebuilt there - a turn nobody looks at costs no light map, which is the reference's own lazy-cache behaviour. Pinned by replaying twenty-five turns on two identical levels and requiring the same answer every turn |
| Field processing | done | P2 | A turn ages every field on every fielded tile and takes them off when they die, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing. The visit order is the reference's: x outer, y inner (`src/map_field.cpp:168-170`), which is what makes the turn independent of the order fields were added - pinned by a test that builds two levels in opposite orders and compares the shared RNG stream afterwards, not just the intensities | A turn ages every field on every fielded tile and takes off the ones that died, in one place (`map::process_fields_in_submap`). Tiles with no fields hold no entry, so an empty map costs nothing; a tile that loses one field keeps the rest and moves its drawn type to what is left. What is not ported yet is the processors themselves - fire spreading, smoke drifting - which need the map to react to a field rather than merely to age it |
| Field layers | done | P2 | `FieldLayer` reproduces `field::add_field` (`src/field.cpp:158-182`): one entry per type per tile, merging sums intensities (clamped to the type's maximum), a dead entry revives at the *new* strength rather than the one it died with, the null type is refused, and the drawn type is the highest priority with ties going to the later arrival. Adding a type that is already there returns false - the difference between "this tile caught fire" and "the fire got stronger" |
| Field decay | done | P2 | `FieldEntry` reproduces `src/field.cpp:58-113`. Three rules that read like bugs and are not: intensity never falls below one, death is a separate flag, and a decay resets the age *and* the decay time so the next half-life is drawn afresh. The decay time itself is `now - age + half_life` where the age has already been ticked, so a field with a half-life of five decays on the fourth tick after it starts, not the fifth. Fire's own branch (`dice(2, age)` against the half-life) is ported but only its shape is tested: the exact draw belongs to the shared RNG stream and will be gated by a replay |
| Cached light | done | P2 | `Level::cached_visibility` mirrors `map::build_map_cache`: a level's light is rebuilt when the level is dirty, not once per query, so an unchanged scene costs one build however often it is read. The cache is keyed by where the light is as well as by whether the world moved - same world, different viewer, different answer - and any terrain write dirties it, which is the only way a stale answer can be avoided without tracking what each submap feeds |
| Level coordinates | done | P2 | `Level` joins the three pieces: submaps own the tiles, the level owns the submaps, and a world coordinate is turned into a cell with **Euclidean** division - truncating division puts tile -1 in submap 0 and folds the world's north-west edge into its middle. A coordinate whose submap was never made reads as nothing and blocks light, so light does not travel through a world that does not exist. The origin does not light itself, and a wall at the edge of the world is still visible while what is behind it is not |
| Light levels | done | P2 | The thresholds of `src/lightmap.h:9-19`, `LIGHT_RANGE` (checked against a table the reference printed), and `map::light_at`'s order (`src/lightmap.cpp:674-696`): out of bounds, then a bright source in the tile, then daylight, then anything worth seeing by, then dark. `MAX_VIEW_DISTANCE` is 72, not the 60 the shadow-caster's radius uses - two constants that look alike and are not |
| Shadow casting | done (eight segments) | P2 | `cast_light` reproduces `oldCastLight` (`tests/shadowcasting_test.cpp:33-90`) driven by the eight segments of `src/shadowcasting.cpp:545-580`, and is gated on five grids the reference itself printed (`corpus/shadowcasting/`). Four segments leaves two quadrants dark; the origin does not light itself |
| Per-level caches | done (dirty submaps) | P2 | `MapCache` reproduces `src/map_cache.cpp`'s promise: a changed tile marks its submap stale, the next read rebuilds that submap once, and a clean read costs nothing. Twenty marks in one submap are one rebuild - a cache that rebuilt per mark would be correct and useless. Invariant I8 is what the tests check |
| Submap ownership | done (one store, ids not pointers) | P2 | `SubmapStore` owns every submap and hands out `SubmapId`, which is an index that survives the store's growth - the reference hands out raw pointers into its map buffer and the port does not, because a pointer is a lifetime the whole simulation would carry. An id that was never made returns nothing rather than a default: an ungenerated submap has no terrain, and the reference's null pointer says the same |
| Tile storage | done (uniform pages) | P2 | `TilePage` is the reference's `submap` storage (`src/submap.h:74-130`): no array until the first write that differs from the uniform value, one-way - writing the uniform value back does not collapse it, because `ensure_nonuniform` does not. Cells are stored row by row as `ter[y][x]`, so a walk sees the reference's order |
| Weighted lists | done (whole-number lists) | P1 | `WeightedIntList` reproduces `src/weighted_list.h`: a non-positive weight is refused, `add_or_replace` with zero deletes, a single entry short-circuits, and the draw is `(randi % total) + 1` against the running total. Picking from an empty list still costs one draw, because the reference evaluates `rng_bits()` before it checks the total - a port that returned early would put every later roll out of step. The fraction-weighted list scales the draw across the whole 32-bit range instead of wrapping it modulo the total, and has no one-entry shortcut, so it is ported as its own type rather than a flag |
| Line walk | done (2D walk) | P2 | `line_to` reproduces the reference's Bresenham step for step, including the starting error term that decides a tie; 14,400 target/offset pairs land on their target (`tests/line_test.cpp:416`), and the degenerate line yields one cell because the reference's walk enters its body once. Not ported: `bresenham` over 3D bubble points, `squares_closer_to_test` |
| Test regime | in place | every gate | Corpus gates carry receipts (what was compared, how much, how long); failures name the id and field and count the differences by kind; the id gate asserts the exact size of each known gap rather than being skipped. Rules: [14-test-porting.md](14-test-porting.md); inventory: [13-test-inventory.md](13-test-inventory.md) |
| Oracle harness modes | done (record/replay/state/digest) | Gate 0 | Replays a recorded session with no terminal and reaches a byte-identical state; see [11-oracle-harness.md](11-oracle-harness.md) |

## P2 — World and environment

| System | Status | Gate | Notes |
|---|---|---|---|
| World submap owner and bubble view | not started | M2 | Ownership decided once |
| Light and illumination | not started | M2 | Four quadrants per square, 21 levels |
| Vision and visibility | not started | M2 | |
| Placing a map | **partial: glyphs into a level** | P9 | `MapgenObject::place` walks a map's rows and writes each square into a level at an origin, resolving symbols through the map's own terrain table and reporting the furniture that stands on a square. Two rules are the reference's and not the port's invention: **a symbol the map does not explain is a data error**, collected rather than silently becoming floor, and a furniture-only square is floor underneath. `terrain_id_of` is a caller's closure, because turning a content id into the level's number is the terrain loader's job. Five tests: a map written glyph by glyph, the origin moving it, an unexplained symbol reported **and left blank**, a furniture symbol that is not unknown, and an id the content does not know reported rather than guessed |
| Mapgen objects | **partial: the rows and their symbols** | P9 | `MapgenObject` and `load_objects` read the JSON method's `rows`, its `terrain` and `furniture` symbol tables, and its weight, and answer the questions a placer asks first: the map's size, whether the rows agree (the reference rejects ragged maps), and what a square resolves to. Four tests against hand-written objects. **The bad gate was the useful part.** Last round's corpus gate read zero maps and the loader "worked": the rows live under `object`, not beside `om_terrain`, and my hand-written fixture had them at the top level, so the test agreed with the port instead of with the data. Fixed, the loader finds **1,071 mapgen objects across 845 files**, and the fixture now uses the real shape with a companion test that a flat object is **not** a mapgen. Width is counted in **characters, not bytes** - a non-ASCII glyph made 200 sound maps look ragged - and exactly **three** maps in the reference tree really are ragged (`lighthouse_small_z1..z3`); the reference loads them anyway, so that is pinned as a fact rather than treated as a fault | 
| Frame diff | **machinery done, comparison pending** | P9 | `frames::read` and `frames::diff` are where both sides meet: the dump is read into squares and two frames are compared field by field - **glyph, terrain, item count and light** - with each difference named by square and by field, carrying both values. Light is compared to a hundredth because two renderers rounding the same value differently is not something a player could see, and would drown the real differences. Six tests: a frame against itself is empty, a changed glyph is named once rather than as a whole frame, each field is reported separately, a sub-hundredth light difference is **not** a difference, a missing square is reported as such, and the reference dump reads back identically twice. **Not yet done:** the comparison itself, which needs the port and the oracle to build the *same* map - the oracle can only dump the shelter it generated at world start, and asking it for a named map at a known rotation needs a dump built on `mapgendata` |
| Frames | **gate opened; the port can draw now** | P9 | `--dump-frame` writes what a player would **see**: for every square of a 41x41 view, the glyph the interface would draw (furniture over terrain, an item over that), the terrain id, the furniture id, how many items lie there, and the ambient light. `corpus/frames/start.json` is the reference's own view of the starting shelter - **1,681 squares**, the player on `t_floor` at light **4.0**. Four tests guard the dump itself while the renderer does not exist yet: the view is full, every square has a glyph and a terrain, no square appears twice, and the player stands on a lit floor. The port's half is `render_frame`: given a level, a centre and the light, it produces the same square list, **in reading order so the two can be compared row by row**. Precedence is the reference's - an item hides furniture, furniture hides the floor - because a diff is worthless if the two disagree about what covers what. Five tests: coverage, glyph precedence, an item covering a table, light passed through, and the reading order itself. **Not yet possible:** the actual diff, because the shelter the reference dumps comes from map generation, which the port does not have; the renderer is exercised against hand-built levels until P9 |
| Eating | **partial, declared** | P3 | `eat` and `MealTally`: **one serving spends `charges_to_use` charges** - one for food, so the rule only shows on a soup that costs five - and the nutrition gained is the type's per serving. Seven tests: a bite costs one charge, a serving of soup costs five, the last serving empties the square, **a partial serving is not a serving and spends nothing**, a rock is not food, an empty square, and a tally that adds up over a meal. **Not ported:** hunger and thirst themselves, the eating time and its interruption, the container a drink comes from, and the rot that food does while it is carried |
| Eating and drinking: the data | **partial, declared** | P3 | The item dump carries `nutr`, `quench` and `charges_to_use` now, and `nutrition_from_dump` reads them into a table: **1,315 of 9,912** items carry nutrition, meat is **46 nutr and one charge a bite**, water quenches without feeding. The nutr value is the reference's `get_default_nutr()` - kilocalories over its own `kcal_per_nutr` - and is taken from the dump rather than recomputed, because that constant is internal to the reference and reproducing it would be inventing a number. **Not ported:** the eating action itself - hunger, thirst, the charge spending, and the container a drink comes in |
| Crafting activity | **partial, declared** | P3 | `start_craft`, `craft_turn_on`, `cancel_craft` model the activity, transcribed from `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`): the unfinished craft is **an item that holds its parts from the moment they are taken**, which is why cancelling loses nothing - the reference only removes an empty componentless recipe item. An unfinished craft carries a sentinel kind until it becomes its result. Four tests: the parts go into the unfinished item, working the turns produces the result, **cancelling keeps the parts inside**, and an interrupted craft keeps its progress. Writing them found a modelling bug: `items_at` was returning things **inside** containers as if they lay on the square, so a craft holding its own parts looked like it had left them there - contents are now excluded, which is true of every container, not just crafts. **Not ported:** the reference's `item_counter` percentage encoding, practice ticks, assistants, and the checks that abort a craft when the recipe or the workbench changes |
| Crafting time | **partial, declared** | P3 | `batch_time_moves` from `recipe::batch_time` (`src/recipe.cpp:108-149`), transcribed part by part because every part is load-bearing: a speed of zero means "cannot craft here" and reports the time as if at full speed rather than dividing by zero; a recipe that does not batch takes the whole time per unit; one that does follows a **scaled logistic curve**, which is why twenty units cost far less than twenty times one and the saving flattens at `batch_rscale`; one assistant gives three quarters and two or more give three fifths; and **no arrangement beats a single unit**, which is a floor in the reference and a test here. Six tests, including the asymptote and the floor. The work-turn loop is ported as `CraftingWork` and `craft_turn`: a turn's moves become progress scaled by `base_total / current_total`, so a slow crafter accrues less per turn rather than needing a different total, progress never overshoots, and a place that cannot be crafted in **cancels** the craft instead of crawling. Five tests, including finishing twice in a row staying finished rather than starting over. **Not ported:** proficiencies and their time maluses, the light and workplace speed factors themselves, practice ticks, and the interruption rules |
| Skills | **partial** | P3 | The recipe dump now carries `skill` and `difficulty`, and `Recipe::skilled_enough` follows the difficulty check in `Character::can_craft`: the crafter's level in the recipe's skill must reach its difficulty, and **knowing the recipe is a separate question** because the reference keeps them apart - a character can be skilled enough for a recipe it has never learned. Six tests, and the corpus gates pin the reference's own numbers: all 6,291 recipes name a skill, **2,960 are fabrication**, and **exactly 1,396 are difficulty zero** - the set a fresh crafter with the recipes known can make. **Not ported:** skill progression and practice, the borrow-a-recipe-from-another-crafter rules, and `can_train` exemptions |
| Crafting | **partial, declared** | P3 | `craft_on_square` from the component handling in `Character::craft` (`src/craft.cpp`, `consume_components`): **availability is checked before anything is taken** - the reference's `has_comps` only asks - alternatives are tried in the order the recipe lists them, and a failure leaves the square exactly as it was. Five tests, one of which found a real bug: an alternative that came up short was consumed and then abandoned when the next alternative filled the group, so a two-alternative recipe could eat something it never used. The fix was to do what the reference does - check, then consume - rather than consume and put back. **Not ported:** crafting time and progress, skill and difficulty checks, the knowledge check, tools that wear out, and the failure products |
| Recipe requirements | **partial, declared** | P3 | `Recipe::components_available` from `requirement_data::has_comps` (`src/requirements.cpp:895-940`): each group is a set of **alternatives** - one of them in sufficient number satisfies it, and every group must be satisfied - with quantities scaled by the batch. Eight tests, two of them over the whole corpus: **all 6,291 recipes are satisfiable with an unlimited stock** (a rule that wrongly rejects a group shows up for every recipe at once) and **none is satisfiable from an empty stock**. Also tested: alternatives are a choice not a sum, a missing group fails the recipe, and a batch of zero is treated as one rather than being a free recipe. Tools and qualities are ported too: `tools_available` follows `tool_comp::has` (`src/requirements.cpp:952-990`) - a count of `-1` is **not minus one charge** but "held and not spent", which is why the encoding is resolved into `ToolNeed` rather than read as a number, and a tool that spends charges needs `count * batch` of them - and `qualities_available` follows `quality_requirement::has` (`:932-940`), where an item at **or above** the level counts and the batch changes nothing because qualities are not spent. `can_make` asks all three, and the corpus gate is that **every one of the 6,291 recipes is makeable holding everything**. **Not ported:** the volume restriction and `check_enough_materials` - the reference's rule that one pile cannot satisfy two overlapping groups, which needs an inventory rather than a lookup |
| Consuming items | **partial, declared** | P3 | `consume_items` from `map::use_amount_square` and `item::use_charges` (`src/map.cpp:5877-5897`, `src/item.cpp:13135-13180`): items are taken **in the order they lie there** - spawn order, so part of the deterministic contract - and the caller learns both how much it got and whether that was everything asked for, because crafting needs to put back what it took when it fails. Seven tests: a partial take, taking exactly what is there, **asking for more than exists (three taken, request unsatisfied, square empty)**, other things untouched, an empty square, several piles taken in order, and asking for nothing. **Not ported:** items inside containers on the square, vehicle cargo, the infinite liquid sources a map can hold, and the filter predicate |
| Item weight | **partial: the base rule, contents included** | P3 | `item_weight` from `item::weight` (`src/item.cpp:7229-7300`): the prototype's weight, times the charges for a stack, **plus everything inside a container**, counted all the way down - a full box is heavier than an empty one, which is the point of the reference's `include_contents` argument. Seven tests: a plain item, a stack, contents, nested containers, a destroyed item weighing nothing, a zero-charge stack counting as one object, and charges that would overflow saturating instead of wrapping. **Not ported:** `integral` (the maximum-damage weight), `NO_DROP` items weighing nothing, corpses, and crafted items weighing their components |
| Containers | **partial, declared; a burnt container spills, except what cannot come out** | P3 | `Container`, `insert_into`, `contents` and `take_out` model a container holding items by volume, with the checks `item::can_contain` makes first: it fits or it does not, and an item cannot be in two places at once. Contents are entities carrying `Inside(parent)`, so a container's contents are a query rather than a parallel list. Six tests now: what fits goes in, a full box refuses and reports the room left and the size wanted, taking something out frees the room, an item cannot be in two containers, a non-container takes nothing, and **a container that burns away spills what was inside it onto the square** - which closes the gap the burning row had been carrying. An item flagged `IRREMOVABLE` does **not** spill: the reference collects only contents without `NO_UNLOAD` into its spill list, so an irremovable item is erased with its container, and that is now a test rather than a guess (46 items in the corpus carry the flag). **Not ported:** pockets and their per-pocket weight and length limits, allow-lists of what a pocket accepts, liquid filling, | `Container`, `insert_into`, `contents` and `take_out` model a container holding items by volume, with the checks `item::can_contain` makes first: it fits or it does not, and an item cannot be in two places at once. Contents are entities carrying `Inside(parent)`, so a container's contents are a query rather than a parallel list - the same shape as the rest of the port. Five tests: what fits goes in, **a full box refuses and reports the room left and the size wanted**, taking something out frees the room, an item cannot be in two containers, and a non-container takes nothing. **Not ported:** pockets and their per-pocket weight and length limits, allow-lists of what a pocket accepts, liquid filling, and the contents of a destroyed container (still named as a gap in the burning row) |
| Things that go off in a fire | **partial, declared** | P3 | `ItemType::will_explode_in_fire` from `item::will_explode_in_fire` (`src/item.cpp:13314-13325`): the type's `explode_in_fire`, or ammunition that cooks off. The **dump came first**: `--dump-items` now carries `explode_in_fire`, `cookoff` and `special_cookoff`, and the reference says **41** items explode in a fire and **387** cook off. The port reads `explode_in_fire` and matches the 41 exactly, and derives the two cookoff fields the way `item_factory.cpp:512-522` does - ammunition whose effects include incendiary or cook-off cooks off, a special cook-off is its own effect, and **hydrocarbon or oil items never cook off** however their effects read. It matches the reference exactly: **387 cook off and 8 do it in their own way.** Ammunition states its effects as `effects`, not `ammo_effects` (`item_factory.cpp:2990`), and the derivation runs only for items that declare the AMMO subtype, which is where the reference derives it - getting either of those wrong gave 4 instead of 387 |
| Item flags that do something | **partial** | P3 | `ItemType::is_irremovable` from `item::is_irremovable` (`src/item.cpp:10356-10359`) - the `IRREMOVABLE` flag and nothing else - and it is wired into the burning loop rather than merely defined. One content test over the whole corpus: **exactly 46** items carry the flag, `efile_lore` among them, and a rock does not. This is the port's first behaviour read from item data rather than from a parameter |
| Item damage levels | **done** | P3 | `damage_level` from `itype::damage_level` (`src/itype.cpp:117-126`): an undamaged item is 0, a type counted in charges is **ruined by any damage at all** (charges have no partial damage, so a leaking bottle reads as ruined rather than scratched), and anything else climbs `1 + 4 * damage / damage_max`, saturating at 5. Five tests, including the saturation past the type's maximum and a type with no damage capacity. Ground items carry a `damage` now, and the stacking rule compares damage levels as the reference does - one of the gaps that row declared is closed, and the test that a dented item never stacks with a sound one is what proves it |
| How much fits | **done** | P3 | `charges_per_volume` from `item::charges_per_volume` (`src/item.cpp:1391-1420`): a charge-counted type fits `wanted * stack_size / volume` charges, anything else fits as many whole items as the space allows, and only a type with **no volume at all** reports `INT_MAX` - a wanted volume of zero holds nothing, because that is what the reference's arithmetic says. Four tests. The first version of this returned "unlimited" for zero space too and a test I wrote tried to justify it; the test was rewritten to match the reference rather than the code |
| Item stacking | **partial, declared** | P3 | `stacks_with` and `drop_and_merge` from `item::stacks_with` (`src/item.cpp:1693-1760`): two things of the same kind stack, charges are ignored for a type counted in charges - which is what makes a pile a pile - and must match exactly otherwise, so **two half-used batteries never merge**, and a burnt item never stacks with a fresh one. Six tests, including dropping onto a matching stack adding to it rather than making a second pile. **Not compared, declared:** relics, variants, contents, degradation, filth and activity (damage **is** compared now), because the port's ground items do not carry them yet; the reference also has `precise` and `check_cat` modes this rule does not model |
| Sound | **partial: distance, reach and falloff** | P2 | `sound_distance` and `heard` transcribed from `src/sounds.cpp:253-275` and the pre-filter at `:484-489`: distance is measured in steps where a diagonal is one, and a noise carries `volume * 2` squares - volume ten reaches nineteen and not twenty-one, with the edge exclusive because the reference's test is strictly greater. `SoundEvent::heard_from` reports the volume a listener hears, falling off with distance. Four tests, including the exact edge and a nearer listener hearing it louder. **Not ported:** the vertical attenuation for sounds across z-levels (5x per level plus the underground bonus), weather attenuation, clustering of simultaneous noises into one centroid, and horde signalling |
| Scent | **partial: the arithmetic and the two terrain rules** | P2 | `diffuse_step` and `decay` transcribed from `scent_map::update` and `scent_map::decay` (`src/scent_map.cpp:60-285`): each turn a square keeps most of its scent and hands a little to its eight neighbours, with `NO_SCENT` terrain emptied and passing nothing and `REDUCE_SCENT` terrain taking part at a fifth of the weight. Three tests: the source keeps more than it gives, a blocking square is emptied **and the square behind it gets nothing**, and decay stops at zero. **Not ported:** the radius around the character, the pause after standing still, scent types, and reading the terrain flags from the map - the caller supplies `ScentCell`s |
| Map memory | **partial, declared** | P2 | `MapMemory` keeps the terrain and a light category per remembered square (`src/map_memory.cpp`, `map::remember` in `src/game.cpp`), and `remember_seen` writes what the cast reaches this turn with `Lit` at or above `AMBIENT_LOW` and `Dim` below it. A square seen once stays on the map after the light goes out, dimmed, with the terrain it had. Four tests: terrain travels with the memory, **the memory survives the light going out and stays dim**, what is behind a wall never enters, and an empty memory is empty. The origin is not lit by its own cast, matching the reference's shadowcasting fixture. **Not ported:** furniture, floor items and submap decoration in the memory; the reference's own decay of remembered submaps; drawing from memory (there is no frame yet) |
| Vision (`seen_cache`) | **partial: cast, light and the rule joining them** | P2 | `Level::seen_at` answers "can the character see this square" over the cast the level already caches, and `vision::shown_at` joins it with light the way the draw path does (`src/game.cpp`): a square is shown when the cast reaches it **and** something lights it. `vision::apparent_light_at` is the brightest quadrant rule. Seven tests across two files: open square and diagonal seen, **wall seen but the square behind it not**, nothing beyond sight, an in-sight-but-dark square not shown (**the gap this round closed**), a lit square behind a wall not shown, and the brightest quadrant being the one that counts. **Still not ported, declared:** the reference's cache holds a transparency *value* rather than a bool and is filled across all z-levels at once; drawing a remembered square dimmed needs map memory; and ambient light is not yet added to the brightest quadrant |
 `Level::seen_at` answers "can the character see this square" over the cast the level already caches, which is the shape of `map::seen` reading `seen_cache` (`src/lightmap.cpp:1039-1100`). Three tests: an open square and a diagonal seen, **the wall seen but the square behind it not**, and nothing beyond the sight range. **Not ported, declared:** the reference's cache holds a per-square *transparency value* rather than a bool, is filled across all z-levels in one pass and can be cumulative between turns, and vision is also limited by light (`apparent_light`), which the port's cast does not consult yet - a lit and an unlit corridor are visible alike here |
| One turn of field processing | **done (assembly); bubble and terrain decide** | P2 | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt (`src/map_field.cpp:160-460`). Two things the world decides rather than the prototype: a field outside the **reality bubble** is untouched - not even aging, which is what the reference means by leaving it unloaded - and the **map** supplies water, sky and passability, so the prototype says how much faster a field ages under water while the terrain says whether it is under water. The bubble is 132 squares on a side, radius 66, matching `MAPSIZE` x `SEEX` in `src/map_scale.h`. Seven tests, including a field outside the bubble left exactly as it was with **nothing drawn for it**, and the edge of the bubble counting while one square past does not | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt - the order `map::process_fields_in_submap` uses (`src/map_field.cpp:160-460`). The **map** now supplies water, sky and passability through `CellFacts`, and those override the prototype's own idea of them: the prototype says how much faster a field ages under water, the terrain says whether it is under water. A field that dies is removed before the later steps; the first version only checked that the entity existed and a test caught it. Five tests, including the same field over water and over dry ground. Per-field data still comes from the caller, so the prototype stays in the content crate | `process_field_turn` walks every field in square order and runs, per field: its own clock, underwater dissipation, the self-upgrade, then the spread attempt - the order `map::process_fields_in_submap` uses (`src/map_field.cpp:160-460`). A field that dies is removed **before** the later steps, which is what the reference's `!is_alive()` check does; the first version of this only checked that the entity existed and a test caught it. Four tests: an ordinary field just ages, the walk is deterministic (no dice for ordinary fields), one spread attempt per turn rather than one per neighbour, and a dead field is never asked to spread. Per-field data comes from the caller, so the prototype stays in the content crate |
| Prototype digest | **done; 15 registries match** | P1 | Verified against the reference digest: `material`, `trap`, `ITEM`, `item_category`, `monster type`, `overmap terrain`, `overmap terrain type`, `overmap special`, `field types`, `terrain`, `furniture`, `vehicle`, `vehicle_part`, `profession`, `scenario`. The last eight matched first time, which is the useful part: the gate now covers ~38,000 prototypes and the only disagreement it has ever found was the terrain/furniture mix-up, so the loaders are in better shape than the count of untested registries suggested. 75 registries remain, most belonging to subsystems that are not ported yet (magic, martial arts, missions, NPC classes) | Registries now verified against the reference digest: `material`, `trap`, `ITEM` (9,912), `monster type` (1,154), `overmap terrain` (24,218), `overmap terrain type`, `field types` and `terrain` (1,109). Checking them found a real defect in the terrain loader: the dump carries terrains **and furniture** in one tagged list, and the port was loading both as terrains, so its "terrain" registry had 1,769 entries against the reference's 1,109. The loader now splits them by `kind` and exposes furniture separately. Eight registries of ninety remain to be wired |
| Prototype digest (start) | **done** | P1 | The harness's digest is FNV-1a over a registry's sorted, unique ids with a zero byte after each, which makes it a gate on the *id set* rather than a count. The port's implementation reproduces **all 90** digests in the artifact from the reference's own id lists, and - the part that matters - the port's **material** and **trap** id sets hash to exactly the reference's digests. That is the first time the port has claimed "same content" with a number instead of an eyeball. The remaining 88 registries need their loaders to expose id sets the same way; each is a one-line addition to that test |
| Mapgen keys | **done; B9 was a bug in my dump** | P1 | The mapgen registry is **fully loaded**: 24,191 of 24,218 overmap terrains have one. The "only 325 built-ins" result that blocked this family for several rounds came from looking keys up by the terrain's **own id** instead of its **mapgen id** - 325 is exactly how many terrains happen to match that way. The dump now keys by `oter.get_mapgen_id()` and the artifact holds **6,306 unique keys**, of which 26 are built in C++ (rivers, subways, forests, lake and ocean shores, ravines) and the rest come from 6,105 JSON mapgen objects. The loader scans the data for `om_terrain` written as a string, a list, or a grid of lists, plus `nested_mapgen_id` and `update_mapgen_id`. Three tests: every reference key is either found in the data or on the built-in list, the built-in list is exactly the 26 expected, and the four keys present in the data but not the registry are the ones the reference itself erases or uses only from C++ |
| Items in a fire (the loop) | **done** | P2 | The loop from `field_processor_fd_fire` (`src/map_field.cpp:980-1031`): a fire finishes at most `intensity * 2` items a turn and stops once it has; each item's contribution comes from `simulate_burn` and its bookkeeping from `burn_item`; anything finished leaves the map and the fire is handed the summed smoke and fuel to round. Ground items gained a `burnt` counter for it. Six tests: a small item consumed, the two-per-intensity cap, a large item surviving but remembering, **no plan means no dice**, a stack losing charges instead of vanishing, and a fireproof item untouched by all of it. `will_explode_in_fire` **is** honoured: items that go off are removed **before** anything burns (`src/map_field.cpp:982-993`), spill what they held, and are counted as `detonated` rather than `consumed` - and a test proves nothing burned and **no dice were drawn** even when the burn plan would have destroyed it. **Not ported, declared:** the explosion itself (blast damage and its own content). Burn products **are** ported: `burn_products` follows `map::create_burnproducts` (`src/map_field.cpp:116-143`), spawning `floor(efficiency * burned / product_weight * portion / portions_total)` of each product of each material, never a copy of the burning item itself. The material dump carries the products now - **64 of 193** materials have them, wood leaving ash at 0.018 - and both the corpus count and the arithmetic are tested. The contents of a destroyed container **are** handled now - they spill onto the square, with the reference's irremovable-item rule still missing |
| Item burning bookkeeping | **done** | P2 | `roll_remainder` (`src/rng.cpp`) - whole values draw nothing, fractions round up with the probability of the fraction - and the charge/accumulation half of `item::burn` (`src/item.cpp:12609-12643`): a stack loses `roll_remainder(burn * 250ml * stack_size / (3 * volume))` charges and a stack with no volume loses them all outright; a solid accumulates `roll_remainder(burn)` and is gone at `volume / 250ml * 3`. Six tests, two of which pin that nothing is drawn when it should not be. **Not ported here:** the corpse branch (needs monster `burn_into` data) and `heat_up` |
| Item burning (`simulate_burn`) | **done** | P2 | How much of an item a fire eats in a turn, transcribed from `src/item.cpp:12543-12592`: a contained fire burns at intensity 3 regardless; an immune material stops the whole item; a material with a `volume_per_turn` scales by that volume against the item's; a liquid giving the fire under 200 **subtracts** `rng(400-1200 per litre)` because wet things smother small fires; several materials are averaged by their declared portions; no materials at all burns at a flat 1; and a stack multiplies by its number of stacks. Nine tests, including that a solid item draws **no dice at all** (stream compared) - the liquid branch is the only one that rolls |
| Material burn data | **done, with a declared gap** | P2 | `--dump-materials` now carries per-intensity `{immune, volume_per_turn, fuel, smoke, burn}` (`src/fire.h:45-56`), and the loader reads `burn_data` from the data files, including volume strings like `"200 ml"`. What the data says: **109 of 193** materials describe burning, wood fuels 1/2/3 across the intensities, and **exactly one material is fireproof at every intensity** (`nomex`). **Declared gap:** the other 84 materials get defaults filled in by the reference at load time, which the port does not reproduce, so a material with no `burn_data` has no burn entry here rather than a zeroed one |
| Monster spawn gate | **partial, declared** | P2 | The field dump now carries per-intensity `spawn_chance`, `spawn_count`, `spawn_radius` and `spawn_group`, and the data says **only two levels in the whole corpus spawn anything**: `fd_tindalos_rift` at 15-in-100 and `fd_fatigue` at 600-in-100. What is ported is the gate (`src/map_field.cpp:763-787`): positive chance and count, one `one_in(chance)`, then the whole count is asked for at once - pinned by a test that three-at-a-time equals three singles over 400 seeds, not three separate rolls. **What is not ported:** turning a group into creatures, which needs the monster-group loader and the deferred spawn list; that is a named gap, not an approximation |
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
| Field prototypes | **done** | P1 | 145 field types read from the reference's dump: id, priority, and every intensity's glyph, colour, danger flag, transparency, move cost and light. Levels are the intensity ceiling, so "intensity 3" now means what the data says it means. Two facts the port had wrong in its head and now has from the data: **thin smoke is see-through and thick smoke is not** (`[true, false, false]`), which is why smoke dims a room instead of blanking it; and **smoke draws over fire** (priority 8 against 4) |
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

