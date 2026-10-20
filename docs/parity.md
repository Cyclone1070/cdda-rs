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
