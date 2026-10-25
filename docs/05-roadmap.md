# 05 — End-to-end porting roadmap

Baseline: `0.I-1`, commit `7b2efa5`, 840 files / 565,083 LOC in `src/` (third-party excluded).
Every file is assigned to exactly one phase in [appendix-file-map.md](appendix-file-map.md) — 100% coverage, no orphans.

This roadmap answers one question: **what gets ported when, and what proves it was ported correctly.**

---

## 1. Ordering principles

| # | Rule | Why | Evidence |
|---|---|---|---|
| R1 | Leaves first | Foundation headers are depended on by nearly everything; porting a mid-layer file before its dependencies means porting it twice | `type_id.h` 293 includers, `debug.h` 269, `point.h` 221, `coordinates.h` 195 | <!-- verified: include fan-in counts -->
| R2 | Build the measuring instrument before the thing it measures | Without a state dump, a replay driver and a digest, a ported system cannot be proven equal — only "looks fine" | No `--dump-*` flag exists at baseline; the harness is new code we write | <!-- verified: grep '"--dump' src/ = 0 hits -->
| R3 | Vertical slices, not horizontal layers | Every phase must end with something that runs and diffs; a half-ported layer has no gate | — |
| R4 | Data before behaviour | 160 JSON types and 6,220 files are the shared input of every later phase; a digest mismatch found here costs minutes, found in P7 it costs weeks | `init.cpp:263-471` registrations | <!-- verified -->
| R5 | Interface last | The UI has the weakest oracle (zero presentation tests) and cannot be validated until the simulation under it is correct | 0 test files include `uilist.h`, `input_context.h`, `sdltiles.h` | <!-- verified -->
| R6 | Packaging last | Build files are throwaway until the binary is worth shipping | 9 release artifacts across 5 targets (`release.yml` matrix) | <!-- verified -->

---

## 2. Phase map

```mermaid
graph TD
  A[Foundation types and diagnostics] --> B[Content registry and save format]
  B --> C[World grid and environment]
  C --> D[Items and crafting]
  D --> E[Creatures and combat]
  E --> F[Spells and enchantments]
  F --> G[Activities and actions]
  G --> H[AI and pathfinding]
  H --> I[Vehicles]
  B --> J[World generation and overmap]
  I --> J
  E --> K[Interface and rendering]
  F --> K
  K --> L[Platforms and packaging]
```

| Phase | Files | LOC | Share | Depends on | Exit gate | Effort class |
|---|---|---|---|---|---|---|
| P0 Foundation | 189 | 62,866 | 11.1% | — | RNG + text + JSON parity vs oracle | mechanical, RNG redesign |
| P1 Registry & saves | 131 | 61,911 | 11.0% | P0 | prototype digest equal; oracle self-consistent | mechanical ×92 types, save format redesign |
| P2 World grid | 60 | 40,200 | 7.1% | P1 | environment-only replay equal | moderate |
| P3 Items | 86 | 91,446 | 16.2% | P2 | item/inventory/crafting replay equal | moderate, `item_location` redesign |
| P4 Creatures | 83 | 74,569 | 13.2% | P3 | creature/combat replay equal | redesign |
| P5 Spells & enchantments | 10 | 9,444 | 1.7% | P2–P4 | spell/enchantment replay equal | moderate |
| P6 Activities | 22 | 26,317 | 4.7% | P5 | activity replay equal | moderate |
| P7 AI | 57 | 38,090 | 6.7% | P6 | combat/NPC session replay equal | redesign |
| P8 Vehicles | 21 | 25,752 | 4.6% | P7 | vehicle session replay equal | redesign |
| P9 World generation | 43 | 47,547 | 8.4% | P1, P8 | same seed → same world digest | moderate, mapgen redesign |
| P10 Interface | 113 | 59,329 | 10.5% | P4, P5 | scripted frame parity, 0 cell diffs | redesign |
| P11 Platform | 25 | 27,612 | 4.9% | P10 | packaging + perf within 10% | throwaway + platform work |
| **total** | **840** | **565,083** | **100%** | | | |

Sizes are the C++ lines being replaced, not the Rust lines to be written. They are a proxy for
surface area, not effort: P0 is 11% of the code and the highest-risk 11%, because everything
inherits its mistakes.

---

## 3. Milestones

| # | After | Meaning | Proof |
|---|---|---|---|
| M0 | P0 | Harness exists; no gameplay yet | RNG draw-for-draw equality; JSON accept/reject parity — **passed**; see [parity.md](parity.md) |
| M1 | P1 | The game's *content* loads identically | Prototype digest equal over 6,220 files, 42 mods, 160 types |
| M2 | P2 | The world ticks identically for environment-only sessions | Replay hash per turn over weather/light/field sessions |
| M3 | P3–P6 | A character can act, craft, fight and cast identically | Replay hash over recorded gameplay sessions |
| M4 | P7–P8 | Full simulation parity including NPC/monster AI and vehicles | Replay hash on late-game combat and driving sessions |
| M5 | P9 | Worlds generate identically from a seed | Same seed → same overmap, city layout, hordes and map chunks |
| M6 | P10 | Playable: visually equivalent | 0 cell differences on the semantic frame gate |
| M7 | P11 | Shipped on the agreed platforms within the perf budget | Artifacts built; perf gate green |

M1 is the first milestone that proves anything. Everything before it is scaffolding.

**Where the work stands.** M0 is passed. The id half of M1 is **finished**: all 90
registries the reference reports are now reproduced exactly, with nothing invented
- terrain and its types, overmap specials, effect-on-conditions, items, monsters,
materials, fields, traps and the rest. Seven loader families
carry field-level corpora from the reference's own dumps (terrain and furniture,
traps, materials, field types, item types, monster types, and the generated
prototypes those loaders create), the registry gate is **90 of 90 id sets
exactly, with no named gaps at all**, and JSON reading and writing match over all
6,220 data files. The last eight ids closed because a plain `flags` member *replaces*
inherited flags while `extend.flags` adds to them - treating the two the same way is
what kept two overmap terrain types wrong.

**P9 has a world on both sides now.** The reference's own overmap, noise field and final terrain can be
dumped in one run from one seed, and the port's rules are checked against that terrain tile for tile: **1,681 of
1,681 tiles accounted for**, with every disagreement being a square the town wrote over. The world layer itself has
gone from a blank grid to woodland from the reference's own noise (**matched to a part in a million on 1,681
points**), cities, streets, **shaped road tiles** in the reference's box-drawing vocabulary, and buildings placed
from the reference's own **258 specials, 35 locations, 178 houses, 223 shops and 51 parks** - the whole specials family, loaded, placed and gated against itself. Two bugs the comparison found and reading would not have: the
regional file's forest thresholds (**0.2 / 0.25**) override the header's, and the noise is shifted by a
**forest growth adjustment** that only appears away from the origin.

**The map level is open, and scored.** A second reference dump captures one 24x24 chunk the generator actually built,
with the overmap terrain it belongs to and the terrain id of all **576 squares**. The port reproduces that chunk's whole
rule: corner depths hashed from absolute positions, the curve between them, the six-ticket raffle, the layer chain, the
region's palettes and their chains, furniture that does not replace the ground, and the four neighbours read from the
world rather than assumed. Measured against the dump it matches **291 squares of 576** - and the way that number moved
matters more than the number: seven separate bugs came out of watching it, each invisible from reading the code. A
neighbour that owns no biome must not be divided by its depth; the ceiling is the region's **densest** forest, not the
square's own; the ground gets its **own** raffle, stepped back by the margin, rather than reusing a failed feature's
ticket; palette names chain, so resolving once leaves placeholders on the map; and the raffle's die is a raw 32-bit
value, not a real one. Two consecutive rounds took the score **down** on purpose - 316 to 302, 305 to 291 - because the
assumption behind the higher number was the wrong one, and the ledger says so beside each figure.

What is left is a single square. The pass lays grass at the chunk's corner where the reference lays forest floor, and
two cheap explanations are already dead: the corner's arithmetic checks out by hand (the empty ticket wins 76% there),
and the sample is not mirrored (identity 291 against a best mirror of 307). So the next step is not another inference
but an instrument: a per-square trace saying which ticket the reference drew, whether a layer fired, and what came out.
**That instrument is built and the trace has been taken** (`CDDA_MAP_TRACE`, one line per square, 31 chunks). It says
the corner draws the same ticket the port's arithmetic predicts, so the disagreement is **downstream of the ticket** -
in what plain ground, own biome and neighbour each turn into - rather than in the raffle itself.

The suite now also runs **on a second machine and a second platform**: `tools/remote_test.py` syncs the workspace to
a Linux desktop and runs every gate there in about thirty seconds, so the same 1,217 cases are verified on macOS and
Linux on each run.

**P2 stands here.** Read this against `03-decisions.md`: things are nouns, systems are verbs,
and answers are neither. T1 (the ground of a square), T2 (positions come from the register, the
grid is rebuilt from it) and T3 (answers and their caches) are done.

**`bevy_ecs` is now the backbone** (P2-T0a): the World is the store, entities are the ids,
components are the columns, and the schedule is data. The register (P2-T0b) is done and
smoke has begun to move onto it (P2-T0c): a field is an entity with a square, a strength and an
age, and the field system walks it. The old per-square container is **deleted**: the map
layer no longer stores fields, and the lighting system reads smoke from the store instead of a
copy. Two sources of truth became one.

The rest of P2-T0c is done too: the sun writes into the same `Light` answer as the torches
(`cast_sunlight`), so daylight and lamps meet in one place instead of two that a reader has to
reconcile. The ground is still a per-square store, which the decision explicitly allows. Every
map-shaped structure that exists today was written before that task existed, which is how the
plan and the decision drifted apart: the register is what makes "adding a kind of thing" not
mean "writing a container for it".

T4 has its first processor: smoke and the rest decay, and gas now spreads to a neighbour with
the reference's thinning rule - the fraction of age that moves is the age divided by the
intensity, and a square already holding the gas gets thicker rather than a second field. What is
missing from the processor is the *direction*: the reference picks it with wind power and a
dice roll. The wind half of that now exists - shelter, forest, height and an upwind wall, in the
reference's order - and it is now wired: `spread_gas` rolls for the chance, picks a starting neighbour, walks
clockwise from it and moves one unit, with wind-facing squares resisted by a dice roll. The first fire processors are in as well: a flame burst burns down and becomes a fire vent,
and the vent thins one turn in three and becomes a burst, carrying the age across the change.
Hot air is ported too, so both fire processors now leave the wisps the reference leaves. Vertical movement is **ported and gated** as of round 517-519: the reference tries the square
below first, then a neighbour, then the square above, and two recorded gas sessions replay it
die for die, one of them dropping through a stairwell. Of the twenty-one processors, ten are in
with gates (`fd_fire` and the fire vents, smoke and the hot air kinds, gas spread, the
extinguisher, the shock and acid vents, the self-upgrade, underwater dissipation, and now
`fd_incendiary`); the rest need subsystems the port does not have yet - monsters, items, the
fungal and radiation grids - and the ledger says which is which rather than leaving them
unnamed. T5 illumination is complete end to end, including point
sources and smoke dimming what is behind it.

step.

**T6 vision is well past "not started" and T7 scent is ported to the turn.** What exists and is gated:

| Layer | What it does | Gate |
|---|---|---|
the cast | `map::sees`'s Bresenham walk, stopping before the target so a wall is visible as a wall | 8 tests |
the level | `lit_level`, the chain from `apparent_light_at`, the draw path's answer | 6 tests |
what is shown | `shown_at` = "drawn at all"; `lit_level_at_square` = "at which level" | 5 tests, plus a pin that the two agree |
the wall's face | the eight-neighbour quadrant rule for opaque squares | 2 tests |
scent | marks from `NO_SCENT`/`REDUCE_SCENT`, the `ScentWindow` arithmetic, decay, and the deposit the turn starts with | 11 tests |

**Weather is end to end.** The reference was rebuilt on this machine as a tracked background job (no install was needed - the build tree was configured), which produced the
first oracle dump of the phase: **20 weather types** with their derived numbers. The loader is gated on it, the `Weather` resource takes a loaded type rather than a hand-set
penalty, and a test proves fog dims a lamp by exactly the absorption it adds. The weather *system* - generators, transitions, precipitation over time, temperature - is still a
declared gap, and it is the next thing after the frame gate.

**The frame gate has both halves, and its terms are measured.** The reference's own frame is committed beside the prototype corpora (1,681 squares with glyph, terrain,
furniture and light), and its glyph column has been read rather than assumed: 1,638 of the 1,681 squares print the terrain prototype's own symbol and 43 print something drawn on
top of it. So the port can be diffed square by square over 1,638 squares today, and the remaining 43 are a named gap that closes when items and creatures exist. What is left is
the join: build the port's world for that fixture and diff.

**A frame can be drawn, and it is text.** `sim::frame::frame` asks the three questions a cell needs - what is there, how lit, how seen - once per view, and `Frame::levels`
prints one character per square (`B L l * .`). That string is the semantic frame gate's unit of comparison, and the tests already assert against it square by square. What it
still needs is an oracle: the reference's own frame for the same fixture, which is the same outstanding build as the weather dump.

**"Hazy" is answered now.** The sight cast carries the reference's own fraction (`cast_seen_light`), so a square seen only through smoke is `BRIGHT_ONLY` - a glow with nothing in
it - and a wall in plain sight is `BRIGHT`. What T6 still lacks is the *grid*: the port asks "how clearly is this square seen" one square at a time and rebuilds the cast for each
answer, where the reference keeps a `seen_cache` per level and rebuilds it once. That is a shape problem, not a rule problem, and it is the next step before a frame is drawn.

Fields come before vision for a reason: smoke is both a field and a thing that blocks sight,
and the light model is now good enough to notice the difference.

Every porting task follows [14-test-porting.md](14-test-porting.md): dump first, write
the failing gate, port the smallest thing that clears it, then tighten the gate. The
fast loop is `python3 tools/test_fast.py`, which runs the test binaries concurrently
(43 s for the whole suite against 156 s serial) and reports what each one cost.

P2 has begun with its first task: tile storage, the uniform page that costs nothing
until it diverges ([parity.md](parity.md)). **A gate that is piped is not a gate.** Clippy has been run every round and twice committed over
since the output went through `grep` for the word "error": the exit status was never read, and a
`-D warnings` failure lists `error:` lines that a grep for something else does not show. The gate is
`cargo clippy ... | grep -cE "^error"` returning **0**, not "I looked and saw nothing".

Run long oracle commands in the
foreground, never backgrounded - a cancelled background dump keeps parsing for
ever, and two of them once held this machine above a load average of eleven. Killing the job
does not kill the process: last round a cancelled dump's child kept generating a map for minutes
after its shell was killed. The check after every long run is `pgrep -fl cataclysm`.

Resume at the mapgen loader: start from `load_and_add_mapgen_function`
(`src/mapgen.cpp:1400-1480`), reproduce the terrain ids it derives from
`om_terrain` arrays - chunked per position and per rotation - and hold them to
`corpus/prototypes/ids.json`, which already names every one of them.

Two smaller gaps are recorded rather than forgotten: the chi-squared roll, which
needs libc++'s gamma sampler and drives NPC mutation timing (`src/npc.cpp:663`),
and `normal_roll_chance`'s error function, which agrees to 4e-8 rather than
exactly. Neither is dead code; both are named where they are used.

---


### The mapgen family's groundwork is measured and pinned (round 493)

The generator is not written, but everything it has to satisfy is now a number or a test rather than a reading of the schema:

| Fact | Value | Where |
|---|---|---|
registry | the reference's **6,306**, both directions | `content_suite::mapgen_keys` |
keys in the data | 6,284, plus 26 in code, minus 4 selectors | same |
entry shape | the map is under `object` in **all 6,216**; never at the top level | same |
terrain declaration | eight combinations, each pinned (2,411 palette-only, 150 grid-with-no-terrain, 1,302 placement-only, ...) | same |
palette graph | **481** palettes, 5,304 references, 278 names, **0** dangling, **0** cycles | same |
first step written | `mapgen::resolve_rows` turns a grid into terrain ids through its own map | `content_suite` |

Two mechanisms remain to be written before a chunk can be produced: **transitive palette resolution** (the graph is proven acyclic, so a plain walk suffices) and
**predecessor mapgens** (one chunk in the data is drawn over another). The thick forest - the chunk we dumped - is generated in code, so the first end-to-end comparison will need a
chunk whose mapgen the data describes rather than that one.

### The mapgen family has its first gate (round 461)

The registry is an equality now: the keys written in the data, plus the twenty-six the game registers in code, minus the four `om_terrain` ids that are selectors rather than chunks,
comes to the reference's **6,306** exactly, and the test names what a port would miss and what it would wrongly offer as two separate failures.

That was the precondition for generating anything. What follows is the generator itself, and the two things it needs are already in the corpus: the palette a chunk is drawn from
(`corpus/maps/reference_map.json`, 27 terrain kinds over 576 squares) and the shape a generated chunk must take (24 by 24, every id known to the terrain loader).

### The oracle now covers all three questions (round 435)

The instrumented reference answers what the port has to match, and as of this round it can be asked all three kinds of question:

| Question | Flag | Corpus |
|---|---|---|
what the data says | `--dump-weather`, `--dump-fields`, `--dump-terrain`, `--dump-items`, `--dump-prototype-ids` | `corpus/prototypes/` |
what one square looks like | `--dump-frame` | `corpus/frames/` |
what generation produced | `--dump-map` | `corpus/maps/` |

The third needed a guard rather than a fix: a read past the loaded chunk waits for overmap generation that a headless run never performs, so the dump now asks only for squares the map
holds. With that, the reference's answer to "what does this chunk look like" is reproducible on demand, which is what a mapgen gate needs.

### M2 is closed: every clause of the exit gate now has a session and a receipt (round 556)

The gate reads: *per-turn state hash equality on environment-only sessions covering weather transitions, fire and gas spread, light changes across z-levels, scent
diffusion, and vision.* All five clauses are now met, each by a replayed reference recording rather than by a hand-written case:

| Clause | Fixture | What is compared | Result |
|---|---|---|---|
weather transitions | `corpus/fields/weather-session.json` | 27 turns: the sky recomputed from the generator's inputs, the manager's decisions replayed from the reference's dice, sector included | receipt `0x4c221d74263f52f4`, equal to the record's |
fire and gas spread | `gas-60`, `gas-fall`, `gas-wind`, the fire sessions | every field on every square, turn by turn, die for die | receipts `0xbb4513a5ce73269a`, `0x033342d5a7845618`, `0x12ca5c3c692106d4` |
light across z-levels | `corpus/fields/light-session.json` | `map::light_at` on every square of the fire's level, seven turns | **175 of 175 squares agree** (round 555) |
scent diffusion | `corpus/fields/scent-session.json` | the diffusing window, turn by turn | receipt `0xaac13754071041c0`, equal to the record's |
vision | `corpus/vision/start-location.json` | the whole `seen_cache` over three levels | **867 of 867 squares agree** (round 544) |

Two of the five clauses had to be *built* before they could be gated - the light session had no light in it until round 552, and its first recording was wrong in a way that
took an instrument fix to see (round 555: the dump built every level before reading any). That is the milestone's real lesson: **an oracle that records the wrong column is
worse than no oracle**, because it fails a correct port and passes an incorrect one.

What is *not* in M2 and stays declared: the field processors that need subsystems (monsters, items, the fungal and radiation grids), and the weather beyond the manager -
pressure systems moving across the overmap, which needs an overmap.

**Next: P3.** The roadmap's P3 is items, inventory, crafting and interaction, and the ledger already carries declared partials to close in order: eating and its data,
crafting activity and time, skills, container handling, item weight and volume, and consuming.

### What the next round should pick up

The gate reads: *per-turn state hash equality on environment-only sessions covering weather transitions, fire and gas spread, light changes across z-levels, scent
diffusion, and vision.* All five clauses are now met, each by a replayed reference recording rather than by a hand-written case:

The weather is a **system** now: the generator's numbers, the condition loop, the sun it needs, and the manager that holds them between turns are all ported and gated, the last
three against the reference's own recorded day. Two things finish it and one thing starts the next piece of M2.

1. ~~Join the pieces into one call~~ - done in round 511: `weather::System::tick` computes the point, decides the sky and books the next change in one call, and reproduces the
   recorded day from the recorded dice alone.
2. ~~The transparency flush, for real~~ - done in round 512: `Map::on_weather_change` marks the level's light stale exactly when the sight penalty changes, and the next read rebuilds
   once. The reference's loop over the overmap's depth is a single mark until the port has more than one level.
3. **A ten-day weather recording** is now the gate (round 514): `--dump-weather-manager` covers the manager's own path, the port replays 240 hours with fourteen absorption
   changes, and it found three real bugs (a data function that could not see the point, a missing requirement chain, and sight penalties that were all one). One temperature sits a
   moment's temperature is now bit-exact: the hook `harness_weather_temperature_terms` records the pre-narrowing double and its terms, which showed the residual was a
   fixture written to ten digits and a `float` product the port was computing in double. One humidity hour remains one ulp out, which is macOS `cos` against Linux `cos`.
4. **Weather in the world** is wired (round 513): `Climate` is a resource, `tick_climate` runs a turn of it, and `run_environment_turn` takes the world's wind over its own.
   The recorded session with a transition now exists (round 531: `--turn-minutes` and the sky on every turn) and both halves of it are gated - the point is recomputed from the
   generator's own inputs (round 532) and the manager's decisions are replayed from the reference's dice, sector included (round 533).
5. **Vision across z-levels** now has its oracle (round 536): `--dump-visibility` writes each level's `seen_cache` and `transparency_cache`, and the first fixture shows
   the character seeing squares of the level below through an opening and nothing above it. `cast_zlight` is now transcribed whole in `cdda_core_types::zlight`, and it
   **reproduces the reference's own seen cache on 867 of 867 squares** over three levels (round 544). Two port-wide bugs fell out on the way: `MAX_VIEW_DISTANCE` was 72
   where the reference computes 60, and `rl_dist` was Chebyshev where `CIRCLEDIST` makes it the truncated Euclidean length.
6. **The per-turn state hash exists** (rounds 547-549): fields, scent and the sky are three layers of one digest, and every environment session has a receipt number. The sight cast is now the reference's (round 545): `Level::seen_cast` runs `cast_zlight`, and ~~one red gate stays as the next thing to fix - smoke's transparency is not folded into the span before the square behind it is written~~ **closed in round 570**: the frame's transparency table now folds the furniture layer and the fields in, and the fields **multiply** the way `calc_transp` says rather than winning a `min` - which makes smoke at its second intensity ten times as absorbing as air and smoke at its third intensity a wall. Two gates pin it (`frame_smoke`): a square behind smoke is seen less, and behind dense smoke not at all.

## 4. Phase detail

### P0 — Foundation types, IDs, RNG, text, diagnostics (189 files, 62,866 LOC)

**Contents.** Coordinates and geometry (`coordinates.h` 1,172; `point.h` 514; 38 origin/scale aliases in `coords_fwd.h:44-103`), units, calendar/time, RNG wrapper, `string_id`/`int_id`/`type_id`,
`generic_factory` scaffolding, JSON read/write types, flexbuffer wrapper, translation lookup, colours,
options table, path handling, assertions and debug reporting, containers (`colony`, `list`, `mdarray`,
`cata_bitset`), hashing, formatting.

**Why first.** `coordinates.h` is included by 195 files and `point.h` by 221. The coordinate layer is
pure value types with no state and no RNG — the cheapest place to be exactly right, and the most
expensive place to be approximately right.

**Hard parts, not mechanical.**
- **Origin/scale legality.** The C++ type system refuses illegal cross-scale arithmetic by only
  defining operators where legal (`coordinates.h:97-196`). A naive `Point(i32,i32,i32)` in Rust
  compiles and silently reintroduces the bug class the types exist to prevent. Rust newtypes must
  reproduce the same legality matrix, not just the arithmetic. <!-- verified -->
- **RNG equivalence.** The engine is `std::minstd_rand0` (`rng.h:30`) — trivially portable. The
  *distributions* are not: 1,441 wrapper call sites go through `std::uniform_int_distribution`,
  `uniform_real`, `normal`, `exponential`, `chi_squared` (`rng.cpp:16-97`), whose algorithms are
  library-specific. See blocker B1. <!-- verified -->
- **JSON semantic strictness.** Duplicate keys are a hard error (`flexbuffer_cache.cpp:74`), unvisited
  members are reported globally (`flexbuffer_json.cpp:14`). A conventional Rust deserialiser accepts
  files the reference rejects, which weakens content validation for every later phase. <!-- verified -->
- **Diagnostics.** Debug levels, error formatting and the test hooks that later gates depend on.

**Not in this phase:** anything that mutates the world.

**Exit gate.** Byte-identical text/formatting output on a fixture corpus; 10M-draw RNG comparison
across every wrapper; JSON accept/reject parity on a corpus of valid, duplicate-key, and malformed
files; the CI pipeline from [port-validation.yml](../.github/workflows/port-validation.yml) runs green.

---

### P1 — Content registry, mods, save format, oracle harness (131 files, 61,911 LOC)

**Contents.** The 160 JSON type loaders, `generic_factory` instantiation for ~92 prototype types
across 68 files / 64,883 LOC, mod manager and dependency graph, options and keybindings, the save/load
graph, compression and archives, plus the harness modes the oracle needs (`--replay`, `--dump-state`,
`--dump-frame`, `--dump-prototype-digest`).

**Why here.** Two independent reasons converge. First, all later phases read prototypes through this
layer, so a wrong inheritance rule corrupts every system at once. Second, the differential harness needs
(a) deterministic RNG from P0, (b) save loading and (c) a state dump — so the harness cannot start
before this phase and everything after it depends on the harness. <!-- inference: ordering rationale -->

**Hard parts.**
- **Inheritance and merge semantics are the content language.** `copy-from` appears 16,444 times,
  `extend` 4,240, `proportional` 2,151, `relative` 790, `delete` 781, `abstract` 465 across 6,220 files.
  The merge operators are *not* centralised — each type implements its own (`assign.cpp:60-398`,
  `mapdata.cpp:1072-1088`, `monstergenerator.cpp:823`). <!-- verified -->
- **Provenance is load-bearing.** Every prototype carries the `src` string of the mod that defined it
  (`generic_factory.h:305,317,352`), used for duplicate detection and origin display. <!-- verified -->
- **Load order is filesystem order** (BFS with per-level sort, `filesystem.cpp:332,372-374`) while mod
  order is a user-ordered list replayed verbatim (`mod_manager.cpp:427-444`). <!-- verified -->
- **`int_id` is explicitly unstable across reloads** (`string_id.h:59-73`) — which confirms the design's
  rule that entities reference prototypes by string id and that any integer index is a rebuildable cache.
- **Save format** — see blocker B2.
- **Options are C++, not data**: 188 `add(...)` declarations (`options.cpp`) plus 88 JSON options. Decide once
  whether they become a data table or stay code; either way it is 4,206 lines to move. <!-- verified -->

**Exit gate (M1).** Prototype digest equality across the full data set and all 42 in-repo mods
(the digest is now ported and **88 of the 90 registries pass**; the two left, `city` and `magic_type`, hold no ids at all); mod load-order
and duplicate-detection parity; oracle self-consistency gate green (see B1); a reference state dump loads
into the port and reproduces the same digest.

---

### P1 status at round 341

**M1 is green.** Both halves of the exit gate are met and evidenced:

- **prototype digest equality — 88 of the 90 registries**, the other two (`city`, `magic_type`) holding no ids at all. Fifteen are checked through a real loader and
  the rest through the id-set rule in [digest_ids.rs](../crates/content/tests/content_suite/digest_ids.rs), which carries the reference's own counterexamples as
  comments: a registry named unlike its JSON type (41 of them), a nested object that is not an entry, a null prototype made in code, an id not called `id`.
- **mod load order and duplicate detection — both parity.** `--dump-mods` gives the reference's own resolved order for all 43 mods and the port reproduces it
  exactly; `Factory::insert` now rejects two definitions from one source with the reference's own sentence and lets another source override in place.

What P1 does **not** claim: the loaders are gated on id sets and, for fifteen registries, on full prototype digests. Field-level equality for the other seventy
registries comes with whatever system first reads each of those fields — that is what the rest of the plan is.

### P2 — World grid and environment (60 files, 40,200 LOC)

**Contents.** Submap storage and the tile SoA, the world's submap owner, the reality bubble view,
per-level caches, fields, light and vision, scent, sound events, temperature and weather, terrain and
furniture records, traps, zones, map memory, the map half of the turn schedule.

**Why here.** Environment runs without items or creatures, so it is the first phase where a full turn
can be executed and compared end to end on a loaded map.

**What already matches the target design.** The tile layer is *already* structure-of-arrays
(`maptile_soa`, `submap.h:62-70`) and the scent grid is already a dense 132×132 array
(`scent_map.h:50-52`), as is the pathfinding flag grid (`pathfinding.h:134`) and the 13 per-level arrays in
`level_cache.h:26-98`. **The dense-index shape is not new — what is missing is a single one for
entities.** <!-- verified -->

**Hard parts.**
- **Ownership.** One global owns all submaps (`mapbuffer.h:91`); the bubble is a non-owning raw-pointer
  vector (`map.h:2114`), and the code itself flags the ownership semantics as unsound
  (`mapbuffer.cpp:99`). The port decides ownership once. <!-- verified -->
- **Light.** 21 levels × 4 quadrants per square × 13 caches with five dirty bits, and a separate
  vehicle pass that exists because vehicle caching mutates neighbouring levels (`map.cpp:9549-9571`). <!-- verified -->
- **Fields.** Behaviour is a C++ processor list selected by 21 data-driven predicates
  (`map_field.h:20-26`); field processing also writes into the scent grid mid-iteration
  (`map_field.cpp:428,500`). <!-- verified -->
- **Hidden cross-call state** to eliminate: `static tripoint_abs_ms player_prev_pos` inside the cache
  builder (`map.cpp:9584-9585`), and a shared `static` object handed out by uniform submaps
  (`submap.h:202-203,219-220`). <!-- verified -->

**Progress, round 316.** Fields, fire and gases now run a whole turn in the reference's order (`run_environment_turn`), the oracle can record a
session turn by turn (fields, scent, terrain, furniture) **and** label its own dice, and one five-turn fire session is replayed by the port to the
same age and intensity. Not yet: weather as a system, light across z-levels, and vision.
Since then: the outside cache and the weather's sight penalty are wired into the cast, the sun runs the transcribed cascade over three rungs, `map::sees` and the field half of
`sight_transparency` are ported, and the scent marks now come from the loaded terrain flags. The store carries `z`, gases use it (fall first, rise last), fires answer in whole
squares, smoke rises through an open floor, and every terrain answer - including the fuel rule's own dice - comes from loaded prototypes (`MapFacts`
over `TerrainFacts`) rather than from a test double. **Every session in `corpus/fields/replayable.txt` replays die for die, including one whose only catchable square is a plank on concrete** - the item
branch of the spread rule now has an end-to-end receipt, and the replay of it corrected two further rules (the flashpoint's fuel gate, smoke coming from
the fuel block). (`corpus_sessions`, machinery in `reference_session`), the first
end-to-end parity receipt for an environment session and now a mechanism rather than a test: a new rule needs a new fixture, not a new test.

**Exit gate (M2).** Per-turn state hash equality on environment-only sessions covering weather
transitions, fire and gas spread, light changes across z-levels, scent diffusion, and vision.

---

### P3 — Items, inventory, crafting, interaction (86 files, 91,446 LOC)

**Contents.** `item` (16,206 LOC) and its contents/pockets, item groups, inventory and item location,
the item factory, materials, crafting and recipes, requirements and qualities, constructions, use
actions (187 functions), examine actions (107 functions), consumption, butchery, liquids, auto-pickup.

**Largest single phase by line count (16.2%).**

**Hard parts.**
- **`item_location` is a hand-rolled weak-reference scheme** — a shared implementation object plus a
  weak aliasing anchor embedded in every item (`item_location.h:184`, `safe_reference.h:68`,
  `item.h:3279`) — existing solely to survive items moving between map, vehicle, pockets and inventory.
  A single entity store with generational handles deletes this problem rather than porting it. <!-- verified -->
- **Raw prototype pointers on instances**: `item::type`, `item::corpse`, `craft_data::making`
  (`item.h:3262,3277`). Under the locked design these become IDs resolved at use — a pervasive but
  mechanical change. <!-- verified -->
- **Every item consumes RNG at construction** (`int seed = rng(0, INT_MAX)`, `item.h:3347`), so item
  creation order is part of the RNG contract. <!-- verified -->
- **Crafting touches the map positionally** — the recipe check inventories a radius around the crafter
  (`crafting.cpp:645`). <!-- verified -->
- **Behaviour hidden behind function pointers**: construction specials are C function pointers named by
  string from JSON (`construction.h:83`, four static tables at `construction.cpp:2286+`). <!-- verified -->

**Exit gate (M3a).** Replay equality on item, pocket, crafting, construction and use-action sessions;
the corresponding reference tests (iteminfo 3,322 lines, item_pocket 2,995, crafting 2,476) re-expressed
as port-side tests.

### P3 status at round 600

Read this with `docs/parity.md`, which carries a row per behaviour; what follows is the shape of the phase.

**In and gated.** Item types and their prototypes (the digest half of M1). Item volume and weight with the
charge rule, contents and the integral volume. Pockets and containers: the compatibility chain, insertion and
removal, and the per-charge volume cap. Item groups: the collection and distribution dice, the holiday skip,
the fixed count that draws no die, the recursion set that stops a cycle at its second visit, the container
overflow behaviours (**spill, discard, and the `none` that loses the item too**), the corpse that takes what
does not fit, and the leading `std::shuffle` — reproduced against **36 recorded permutations**, because that
shuffle runs on the standard library's engine and no `rng()` hook ever sees its dice. Consuming: the charges
walk (spawn order, the filter asked before the type, the liquid gate before both, contents **pre-order** after
their container) and the amounts walk (whole items, contents **first and bottom up**) — two functions that look
alike and are not. Crafting: requirement satisfaction, the batch time rule and its floor, the work turns, and
the unfinished craft that holds its own parts. The line family a use action needs: the 2D and 3D walks,
`squares_closer_to`, `move_along_line`/`continue_line`, and `clear_path` with its three habits (the last square
never checked, the inclusive flat comparison against the strictly-climbing one, and support in either column).

**Next, in order.** ~~The pockets' **length** limits~~ (in at round 602: `ItemType::longest_side`, gated over
all 9,912 types), ~~the pocket loader and its accept-lists~~ (rounds 603-605), ~~the accept rules in the
reference's order~~ (round 606: `check`, 4,752 real verdicts), ~~**corpses**' volume~~ (round 608: 8,078
measurements over 1,154 creatures). What is left of this sentence: the item's own `volume` **variable** override,
which nothing in the content sets yet, and ~~**crafts**~~ (round 609: the volume is the sum of the parts floored at
one millilitre, the weight the same sum with no floor). Then the layer the
phase is named for: an inventory that is a set of entity handles rather than an `item_location` scheme, so
that wielding, wearing and magazine loading can be ported once instead of twice. Use and examine actions come
after that, because every one of them starts by asking an inventory something.

**Not started, and named.** Hunger and thirst themselves (the eating *data* is loaded: nutrition, quench and
charges-per-use), butchery, liquids as a system, auto-pickup, and the 107 examine actions.

---

### P4 — Creatures, body, effects, combat (83 files, 74,569 LOC)

**Contents.** `Creature`/`Character`/`Monster`/`Npc` state, effect maps, body parts and anatomy,
mutations, bionics, morale, needs, wounds and disease, melee and ranged resolution, projectiles,
dispersion, martial arts, weakpoints, harvest, death.

**Hard parts.**
- **Position plus four hand-maintained indexes.** A creature stores a bare absolute coordinate
  (`creature.h:803`) and invariants are repaired by a virtual `on_move` hook (`creature.cpp:316`); the
  tracker holds a vector, an unordered position map, a "removed this turn" graveyard that exists to keep
  dangling references alive, and a lazily rebuilt zone index (`creature_tracker.h:148,150,156,165`).
  Moving one monster linearly scans all monsters. <!-- verified -->
- **`Character` is a 1,269-member object** with five mutation containers that must stay consistent,
  index-addressed bionics, and three concurrent activity slots (`character.h:4022-4041,2794,2780`). <!-- verified -->
- **Effects are a nested map keyed by effect type then body part** (`effect.h:259`), i.e. per-creature
  allocation rather than a dense component. <!-- verified -->
- **~20 flat additive stat channels** on every creature (speed, dodge, hit, bash, armour bonuses) that
  every source — prototype, mutation, bionic, effect, enchantment, item — writes into. <!-- verified -->

**Exit gate (M3b).** Replay equality on combat, wound, mutation, bionic and morale sessions; vision and
ranged-balance test parity.

---

### P5 — Spells and enchantments (10 files, 9,444 LOC)

**Contents.** Spell types and casting, spell effects, enchantments, terrain and furniture transforms,
teleporter lists, relic generation.

**Why not late: magic is core content, not a mod feature.** <!-- verified -->
- `SPELL` is a registered core type (`init.cpp:453`), finalised and consistency-checked alongside every
  other type (`init.cpp:797,903`).
- Core data casts spells in 20 files: monster special attacks, mutations, bionics, artifacts, traps,
  guns and scenario scripting. Mods add far more (169 spell files in Magiclysm, MindOverMatter and
  Xedra Evolved).
- `magic.h` is included by 42 files and `magic_enchantment.h` by 38 — creature stats, item stats, use
  actions, traps, projectiles and NPC attacks all read enchantments.
- Enchantments feed the same flat stat channels that mutations and bionics write to, so the creature
  phase cannot be called complete without them.

**Where it sits.** After creatures and items, before activities: a character that cannot cast is not a
character the replay corpus can exercise, and monster spell attacks are part of the AI phase.

**Deferred to a later phase:** relic procedural generation and the spellbook interface, which are
content-generation and presentation concerns rather than simulation.

**Exit gate (M3c).** Replay equality on spell, enchantment, mutation-granted spell and artifact
sessions; the reference's spell and enchantment tests re-expressed.

---

### P6 — Activities and the action pipeline (22 files, 26,317 LOC)

**Contents.** The activity actor hierarchy (~67 classes), the player activity record, activity handlers,
pickup, movement mode, the key-to-action pipeline.

**Hard parts.** The actor pattern itself is clean and translates directly (`clone_ptr<activity_actor>`
plus a dispatch table keyed by activity id, `activity_actor.h:135-137`). What must be *replaced* is the
generic argument soup that actors coerce — parallel arrays of items, ints, strings, coordinates, weak
monster references (`player_activity.h:62-72`) — with real per-actor payloads. <!-- verified -->

**Exit gate (M3d).** Replay equality on activity sessions; the 2,138-line activity test file
re-expressed.

---

### P7 — AI, pathfinding, dialogue and scripting (57 files, 38,090 LOC)

**Contents.** Monster planning and movement, NPC movement and decision making, pathfinding, behaviour
trees, NPC classes, trading and opinions, dialogue, conditions and effect-on-condition, speech, factions'
runtime side, missions.

**Hard parts.**
- **Two unrelated architectures for the same job.** Monsters run data-driven behaviour trees whose nodes
  are raw-pointer graphs with 13 string-keyed predicates and a `// TODO: make into an ID?` on the goal
  field (`behavior.h:86`, `behavior_oracle.cpp:34-47`); NPCs run a 28-value enum with a 28-way switch
  over ~30 hand-tuned float scorers and a cache holding weak references to every nearby creature
  (`npcmove.cpp:198-227,1608`, `npc.h:552`). <!-- verified -->
- **Pathfinding is self-contained but load-bearing**, including a tuning constant whose comment admits
  the search window is too small (`pathfinding.cpp:440`). <!-- verified -->
- **Highest risk of silent divergence in the whole port**: float scoring, tie-breaks and iteration order
  all decide which action is chosen, and one different choice changes every later turn. This phase needs
  trace-level gates, not just end-state hashes.

**Exit gate (M4a).** Replay equality on combat-heavy and NPC-interaction sessions; per-decision trace
hash equality for monster and NPC action selection; dialogue, mission and effect-on-condition test
parity.

---

### P8 — Vehicles (21 files, 25,752 LOC)

**Contents.** Vehicle model and prototypes, part definitions, movement and collision, vehicle
interaction, turrets, vehicle groups, appliance forms.

**Hard parts.**
- **No identity at all.** A vehicle is addressed by its owning submap plus a submap-local 2-D point with
  no z (`vehicle.h:2392`); world lookup falls back to a linear scan of a submap's vehicle vector. The
  port must invent the identity the reference never had. <!-- verified -->
- **Parts are a renumbering vector** with ~30 derived index vectors rebuilt by a single `refresh()`
  (`vehicle.h:2243,2305-2332`). Removing a part renumbers every later part. <!-- verified -->
- **Pointer-order tie-breaks in movement**: vehicles are held in a pointer-ordered set
  (`level_cache.h:81`) and the mover picks the maximum-moves vehicle with a strict comparison, so ties
  resolve by heap address (`map.cpp:729-737`). This is blocker B1 in its purest form. <!-- verified -->
- Raw two-way towing pointers, raw passenger pointers inside parts, and a vehicle whose turret brain is
  a heap-allocated NPC. <!-- verified -->

**Exit gate (M4b).** Replay equality on driving, collision, boarding and turret sessions.

---

### P9 — Overmap, world generation, factions, missions (43 files, 47,547 LOC)

**Contents.** Overmap storage and generation, map generation (JSON interpreter plus 11 builtin
functions), regional settings, world factory, hordes, cities, map extras, basecamps and faction camps,
missions.

**Why after P1 and P8.** Existing saves carry already-generated worlds, so the port can replay real
sessions long before it can *create* a world. Generation is deferred deliberately: it is the largest
pure-redesign surface and nothing before it needs it. <!-- inference: ordering rationale -->

**Hard parts.**
- **Map generation is a program, not a format**: 845 JSON files / 239,715 lines / 12 MB in a ~15-node DSL
  (palette, piece, constraint, place, setmap, nested, update, phase) interpreted by C++
  (`mapgen.h:194-568`). Porting it means writing an equivalent interpreter. <!-- verified -->
- **Generated worlds are order-dependent**: overmap noise draws from the single global engine, and
  shuffles during generation (`overmap.cpp:4921,5547,5623`) mean generating region A before B changes B.
  <!-- verified -->
- Overmaps live in an unordered map (`overmapbuffer.h:581`), so iteration order over loaded overmaps is
  unspecified — another B1 case. <!-- verified -->
- Basecamps have two different ownership models for the same type (by value in the overmap, by pointer
  in a submap). <!-- verified -->
- `faction_camp.cpp` is 6,354 lines — the largest behaviour file after items and map. <!-- verified -->

**Exit gate (M5).** Same seed → identical overmap digest, city layout, horde distribution and map
chunks over a corpus of seeds; basecamp and mission test parity.

---

### P9 status at round 271

Three more rounds of replaying the reference's own dice changed the picture again, and each change is measured rather than
argued:

| Finding | Consequence |
|---|---|
A biome layer can yield **furniture**; the reference treats that as no terrain and still runs the ground raffle | the port was writing a furniture id where terrain belongs - fixed; placing the furniture itself is a declared gap |
Forest items are placed **once per chunk**, not per square (`place_items` over the whole chunk, `item_spawn_iterations` times) | ported: the count comes from `roll_remainder(chance * spawn_rate / 100)`, then three draws per square attempt with up to twenty retries |
Expanding the item group costs **589-601 dice per chunk** | the placement rule accounts for a handful, so the bulk is item construction, which the port does not do - a sized gap, not a vague one |

The replay itself is the instrument: it walks the reference's 3.3-million-draw recording square by square, checks every
bounded draw's range, and stops exactly where the port stops agreeing. It replayed **31 squares exactly** before the first
draw the port cannot make.

### P9 status at round 264

The forest half of map generation is now measured against a traced reference rather than against a reading of the source, and
the measurements changed the design three times:

| Finding | Effect on the port |
|---|---|
| The corner rule never runs (it compares a biome copy's address) | not called; kept and documented as intent |
| Generation leaves **placeholders**, resolution is a later sweep | pass split into a placeholder form and `resolve_chunk_placeholders` |
| The reference's chunk as generated is **574 of 576** placeholders | the port now produces placeholders too, and only then resolves |

Remaining for P9's exit gate: replaying the reference's dice step by step so a chunk can be compared **square by square**
rather than by shares, then the same treatment for cities, roads, specials and map extras.

### P10 — Interface, rendering, audio (113 files, 59,329 LOC)

**Contents.** Output and text layout, the UI stack and window abstraction, menus, HUD widgets and panels,
input and keybindings runtime, colour, both rendering backends, tilesets, fonts, ASCII art, sound and
music, help, and the screens (inventory, crafting, overmap, character, medical, safemode, trade, diary).

**Why last.** Zero of the 218 test files exercise the UI, and there is no screen-by-screen behaviour
specification. The oracle for this phase is the one we build ourselves, and it only becomes meaningful
once the simulation beneath it is proven.

**Hard parts.**
- **The terminal build renders through ImGui into ncurses** rather than raw curses, and the two backends
  are mutually exclusive at build time (`CMakeLists.txt:154-155`, `cata_imgui.cpp:100-157`). The port must
  decide whether to preserve that dual-build model or to render cells directly. <!-- verified -->
- **Global mutable UI state**: a static UI stack, a static frame flag, file-scope clients
  (`ui_manager.cpp:26`, `sdltiles.cpp:79`). <!-- verified -->
- **The draw path is 5,616 lines of cache, occlusion, memory and rotation logic** (`cata_tiles.cpp`). <!-- verified -->
- HUD widgets and panels are already data-driven JSON (116 files under `data/json/ui`), so the *spec* is
  reusable even though the renderer is not. <!-- verified -->

**Exit gate (M6).** Semantic frame gate: for scripted input sessions, every cell's glyph, foreground and
background colour identical, zero tolerance; pixel diff as a nightly secondary signal; no UI-driven
divergence in the simulation hash.

---

### P11 — Runtime shell, platform, tooling (25 files, 27,612 LOC)

**Contents.** The game object and its turn loop, main entry and command line, the action handler,
gamemode, uistate, event bus, debug menu (~309 actions), map editor, overmap editor, screenshot support,
memory and difficulty instrumentation, crash handling, platform shims.

**Why last.** Nothing here is needed to prove correctness; all of it is needed to ship.

**Decisions to make before starting.** Which platforms are in scope (see B6), whether the debug menu
ships in release as it does today, and whether screenshots stay the only capture mechanism or become the
frame-dump path the visual gate needs.

**Exit gate (M7).** Artifacts build for the agreed platforms; the performance gate (no worse than 10%
on the frame and turn benchmarks) is green.

---

## 5. Critical path and parallelism

- **Critical path:** P0 → P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8 → P9 → P10 → P11. It passes through items,
  creatures and AI because those are the systems with the most cross-dependencies.
- **Off the critical path, start early and run in parallel:** the harness modes and CI (P1), the
  prototype digest tooling (P1), port-side test-runner infrastructure and the semantic frame dump (P0/P1
  even though the UI is ported in P10), the tileset and soundpack parsers (P10 code, spec is
  documentation-only), and the platform evaluation for B6.
- **Never parallelise:** the coordinate/RNG/JSON foundation (P0) and the save format decision (B2) —
  both are inherited by everything downstream.

---

## 5b. Open questions found while porting

**Vehicle parts accessor - closed.** `vehicles::parts::get_all()` and
`vehicles::get_all_prototypes()`; 701 parts and 249 prototypes dumped and loaded.

**Items on the ground - unblocked.** The pile is now part of the store: items are entities with a
square, a type and charges, returned in drop order, which is what the fire processor needs to
consume a pile front to back the way the reference does. Next: terrain flammability, then `fd_fire`.

**Mapgen: explicit directory load - tried, no effect.** Loading `jsondir/mapgen` by path and then
finalising still leaves 325 keys, so the files either were already loaded or the registration is
deferred further. Combined with the game-order result, the picture is now: mapgens register per
overmap terrain, on demand, when a chunk is generated - not during data load at all.

**NPC classes - open, and it now falls through into a normal run.** Three attempts: ids plus
`is_common()` hung; ids only hung; ids only on the *game's own* load path (`load_core_data` then
`finalize_loaded_data`) hung, and its log shows the game's title art - so that branch does not dump
at all, it falls through and starts the game waiting for input. The dump was removed rather than
left in the tree. Next attempt: prove the branch is reached (print one line first), before asking
anything of `npc_class::get_all()`.

**Mapgen registry - promoted to B9 below.** Three explanations were tried and ruled out, and the
harness has no world-initialisation path to fall back on, so this is no longer an open question
worth spending rounds on. It is a blocker with a work item behind it.

## 6. Hard blockers

These are the findings that need a decision or an experiment before the phases that depend on them.
None of them stops the port; each one changes the plan if it resolves badly.

### B1 — The reference is not fully deterministic, so "identical output" needs a definition

**Verified evidence.**
- Vehicles are held in a pointer-ordered set and the mover breaks ties by pointer value
  (`level_cache.h:81`, `map.cpp:729-737`). <!-- verified -->
- Loaded overmaps are iterated in hash order (`overmapbuffer.h:581`). <!-- verified -->
- A single global RNG engine is drawn from by overmap-generation shuffles and by *every item at
  construction* (`rng.h:32`, `item.h:3347`). <!-- verified -->
- The RNG engine is portable, but its distributions are library-specific (1,441 call sites through
  `std::uniform_int_distribution` and friends, `rng.cpp:16-97`). <!-- verified -->
- The reference's own test suite pins no distribution outputs (`tests/rng_test.cpp` asserts statistical
  thresholds only) and CI seeds tests from the clock. <!-- verified -->

**Consequence.** Byte-exact equivalence cannot be asserted until we know how much of the oracle's output
depends on memory addresses and standard-library internals. A port using stable IDs will be *more*
deterministic than the reference, which shows up as a mismatch — a divergence in the right direction,
but still a red gate.

**Required before P1's gate.**
1. Oracle self-consistency: run the same binary twice on the same session, compare state hashes. Any
   session that fails is not usable as a fixture.
2. Configuration sensitivity: same session under the sanitizer build and under a different allocator.
   If hashes differ, the responsible systems are exactly the pointer-order ones (B1's list above).
3. Adopt a policy: the port picks one canonical order for those tie cases, and the harness either
   patches the oracle to match or records the session as order-sensitive and excludes it.

**Status:** blocking for any claim of exactness; mitigation path is defined and cheap (hours, not weeks).

### B2 — Save compatibility: **resolved, out of scope**

**Verified evidence.** The format is not pure JSON: a text `# version` header addressed by byte offset
(`savegame.cpp:185`), per-submap version members inside arrays (`mapbuffer.cpp:321,440-443`), a bespoke
zstd-frame archive container (`zzip.h:19-60`), three shipped zstd dictionaries that must match
bit-for-bit, two coexisting on-disk layouts selected by a world option, and 388 hand-written serialise
methods of which roughly 40% of the load code is conditional fix-up (`savegame_json.cpp`). There are
**no save or migration tests anywhere in the test suite**. <!-- verified -->

**Decision (settled): the port does not read reference saves.** It writes its own format.

**What that removes.** No transcription of the archive container, the dictionaries, the
byte-offset header, the two coexisting layouts, or the legacy fix-up paths.

**What it adds, and this is the part that matters for testing.** Sessions can no longer
start from a shared save file, so the reference must *hand over* its starting state
through the harness: the state dump becomes an input format the port can load, not
just an output for comparison. That format is ours, is debug-only, and only needs to
carry the fields the gates compare — far smaller than the save graph.

**What still has to work:** the port's own save and load, with round-trip identity
(invariant I9) and long-play sessions (T8), because players save.

### B3 — JSON acceptance semantics are load-bearing

Duplicate keys are a hard error and unvisited members are reported globally
(`flexbuffer_cache.cpp:74`, `flexbuffer_json.cpp:14`). A conventional Rust JSON reader would accept files
the reference rejects, quietly weakening content validation. Either reuse the vendored reader through FFI
or transcribe its error behaviour, and gate it with an accept/reject corpus. <!-- verified -->

### B4 — Translations are bound to the C++ message set

691 MB of catalogues across 28 locales, 11,484 `_()` call sites in C++ and 13,527 comment keys in data,
plus a custom plural-rule evaluator (`translation_plural_evaluator.cpp`, 466 lines). Any rewrite that
changes message ids invalidates all existing translator work. **Decision needed:** freeze the message-id
set and keep the gettext pipeline, or accept the loss. <!-- verified -->

### B5 — There is no presentation oracle; we have to build one

The reference has no state or frame dump flags (`grep '"--dump' src/` = 0), no replay capture, and zero
tests over the UI, input or rendering layers. The semantic frame gate in
[04-validation.md](04-validation.md) therefore requires new C++ code in the oracle *before* the port's UI
phase can be validated at all. This is the largest piece of infrastructure work in the plan that does not
appear in any phase's file list. <!-- verified -->

### B6 — Platform scope: **resolved, desktop only for now**

Windows (MSVC, MinGW, vcpkg), macOS (universal, dmg), Linux (tar, snap, two divergent Flatpak manifests),
Android (NDK, GLES, 33 platform-specific code sites) and WebAssembly (built in CI, commented out of the
release). Terminal and graphics builds are mutually exclusive, so the dual-build model is itself a
decision. <!-- verified -->

**Decision (settled): macOS, Linux and Windows, from one codebase.** Android and
web are deferred, not cancelled, and [12-platforms.md](12-platforms.md) lists the
seven constraints that keep them reachable — one window/input layer, one filesystem
layer, no required threads, one content loader, renderer separate from simulation,
input as events, audio behind an interface. All seven are already how the reference
behaves, so this is the default rather than extra work. Measured platform cost per
target is in the same document.

**Consequence for the plan.** P11's gate becomes "artifacts build and smoke-run on
the three desktop targets"; CI gains one build job per desktop target; the visual
gate keeps one pixel baseline per platform while the semantic frame gate stays
identical across them.

### B7 — Some behaviour can only be transcribed, never derived

Field processors are a C++ list selected by data (`map_field.h:20-26`), construction specials are named
function pointers (`construction.h:83`), map generation is a 239,715-line DSL over 11 builtin functions,
and tuning constants include values with in-source admissions that they are wrong
(`pathfinding.cpp:440`), loop bounds justified by anecdote (`map.cpp:683`), and empirically fitted
thresholds (`weather.h:59-74`). These must be copied exactly and marked as such; "improving" any of them
silently breaks replay equality. <!-- verified -->

### B9 — resolved, and it was my bug

The blocker was never the reference. `mapgen::has_mapgen_for` is a plain lookup, the data loads, and the registry is full: **24,191 of 24,218** overmap terrains have a map. My dump asked for `oter.id.str()`; the game asks for `oter.get_mapgen_id()`. Those agree for exactly 325 terrains, which is the number I mistook for "only the built-ins".

The lesson is the same one the vehicle-parts round taught: when a registry "looks empty", check the key before the loader. A dump that cannot see the data is not evidence that the data is missing.

### Original notes, kept

## B9 — The mapgen registry cannot be reached from the harness

Three explanations ruled out by experiment: forcing `calculate_mapgen_weights()` leaves the count
at 325; forcing `finalize_loaded_data()` aborts because `check_mod_data` already finalised; and it
is not a key-space mismatch, since `house` and `house_north` are both absent while `LIXA_*`,
`field` and `forest` are present - the signature of the built-in C++ mapgens alone. Every dump
path in the harness runs through `check_mod_data`, and there is no world-initialisation path to
fall back on.

**Work item:** give the harness a path that initialises a world, or a loader hook that registers
JSON mapgens without finalising twice, then gate the mapgen loader on the dump.

**What it does not block:** the rest of P1 and all of P2. The mapgen loader family simply has no
gate yet, which is written here so it cannot later be mistaken for done.

### B8 — Divergences we intend to accept

Two reference behaviours should *not* be reproduced, and each needs a written exception in the
validation spec: the shared `static` object returned by uniform submaps (writes through it mutate a
process-wide object, `submap.h:202-203,219-220`), and the "removed this turn" graveyard that exists to
keep dangling creature references alive (`creature_tracker.h:151`). Both are artefacts of the ownership
problem the port removes. <!-- verified -->

---

## 7. What "ported" means

The port is complete when all of the following hold, in the order the gates in
[04-validation.md](04-validation.md) define:

1. Every prototype loads to the same digest from the same 6,220 data files and 42 mods.
2. Recorded sessions replay to identical per-turn state hashes within the declared tolerance classes.
3. The invariant suite holds on every session, including the ones the reference fails.
4. The fuzzer runs a night of sessions without a crash, a hang, or a divergence it cannot minimise.
5. Every scripted visual session produces zero differing cells.
6. Frame and turn times are within 10% of the reference on the same hardware and data.
7. The agreed platform artifacts build from a clean checkout in CI.

Anything less is a system that "seems to work", which is the failure mode this whole document exists to
prevent.

---

## 8. First ten tasks

1. Pin the baseline (done) and record its digest inputs: data files, mods, dictionaries, RNG seeds.
2. Write the oracle self-consistency gate and run it (B1 step 1).
3. Run the allocator and sanitizer sensitivity experiment (B1 step 2); publish the outcome as the
   tolerance policy.
4. Add `--dump-state`, `--dump-prototype-digest` and `--replay` to the oracle — nothing else can be
   validated without them.
5. Build the prototype digest and run it against the reference; freeze the expected value.
6. Build the JSON accept/reject corpus (valid, duplicate-key, malformed, unknown-member) (B3).
7. Port the coordinate layer with its origin/scale legality matrix, plus compile-time tests that illegal
   arithmetic does not compile.
8. Port the RNG wrapper and prove draw-for-draw equality against the pinned oracle standard library.
9. Make the state dump loadable in both directions (B2 is settled: no reference
   saves, so this is how a session starts in the port).
10. Start the semantic frame dump in the oracle (B5) — it is the longest-lead infrastructure item and it
    gates P10.
