# 10 — Task breakdown

Every phase expanded into ordered tasks. A task is done when its **proven by**
column is satisfied, not when the code exists. Task IDs are stable and are what
[parity.md](parity.md) refers to.

Rule for the whole document: a task that cannot name how it will be proven is not
ready to start.

---

## P0 — Foundation

| ID | Task | Produces | Proven by | Status |
|---|---|---|---|---|
| P0-T1 | Instrumented reference build | Oracle binary with harness modes behind a flag | Builds; modes documented in [11-oracle-harness.md](11-oracle-harness.md) | done |
| P0-T2 | State fingerprint + full dump | `--dump-state` in the reference | Two runs of the same session agree | done |
| P0-T3 | Prototype digest | `--dump-prototype-digest` | Digest is stable across runs and rebuilds | done |
| P0-T4 | Coordinate and scale types | Origin/scale-safe point types | Illegal arithmetic fails to compile; exhaustive conversion tests | done |
| P0-T5 | Units and quantities | Typed physical quantities | Round-trip and overflow tests | done |
| P0-T6 | Calendar and time | Dates, durations, turn time | Reference date arithmetic re-expressed | done |
| P0-T7 | Random numbers | One stream, reference-identical draws | 10M-draw comparison across every wrapper | done |
| P0-T8 | Identifiers | Typed string/int ids, caches only | Reload-safety tests | done |
| P0-T9 | JSON reader | Strict reader matching acceptance rules | Accept/reject corpus | done |
| P0-T10 | JSON writer | Byte-stable writer | Golden output tests | done |
| P0-T11 | Parse cache | Cached parse artefacts | Cache-hit and staleness tests | done (5 tests; all 6,220 files identical cold vs warm) |
| P0-T12 | Options table | All 276 options declared once (188 in C++, 88 in JSON) | Reference option list compared item by item | done |
| P0-T13 | Paths and roots | Seven logical roots, platform rules | Table-driven path tests | done |
| P0-T14 | Text lookup and formatting | Message lookup and string formatting | Message-id and formatting parity corpus | done |
| P0-T15 | Colours | Colour definitions and themes | Rendered-value parity | done |
| P0-T16 | Containers and hashing | Stable pools, dense bit sets, hashing | Behavioural tests against reference container semantics | done |
| P0-T17 | Diagnostics | Debug levels, assertions, error reporting | Error-path tests | done |
| P0-T18 | CI skeleton | Jobs for T0–T2 wired to the port | Runs green on an empty port | done (foundation.yml; every command run by hand first) |

**Gate M0:** RNG, text and JSON parity; harness modes exist; acceptance corpus passes.

---

## P1 — Content and persistence

| ID | Task | Produces | Proven by | Status |
|---|---|---|---|---|
| P1-T1 | Registry machinery | Load, inherit, finalise, consistency-check | Fixture types load in both builds | in progress (machinery done and exercised on the whole corpus; per-type loaders next) |
| P1-T2 | Inheritance and merge operators | `copy-from`, `extend`, `delete`, `relative`, `proportional` | Operator-by-operator parity corpus | in progress: `copy-from` and its deferral rule done; the four operators are per-loader (`mapdata.cpp:1072`, `assign.cpp:60-75`) and arrive with the loaders |
| P1-T3 | Loader families (items, monsters, terrain, vehicles, recipes) | The five largest content families | Per-family digest | not started |
| P1-T4 | Loader families (remaining 155 types) | Everything else | Full digest equality (M1) | not started |
| P1-T5b | Loader family: monsters | Monster types, first slice | Monster parity corpus | done: 1,154 of 1,154 on the dumped fields |
| P1-T5 | Mod discovery and ordering | Discovery, duplicate detection, user order | 42 in-repo mods load in the same order | not started |
| P1-T6 | Dependency graph and cycles | Tarjan-style cycle detection | Synthetic cycle fixtures | not started |
| P1-T7 | Save format decision | Written decision (blocker B2) | Recorded in [03-decisions.md](03-decisions.md) | not started |
| P1-T8 | Save reading | Load reference saves or refuse explicitly | Round-trip digest | not started |
| P1-T9 | Save writing | Write the port's saves | Round-trip identity (I9) | not started |
| P1-T10 | Migrations | Content and per-field legacy paths | Old-format fixtures | not started |
| P1-T11 | Compression and archives | Container format, dictionaries | Byte-compatible archives | not started |
| P1-T12 | Map memory and seen tiles | Persistence for map memory | Digest parity | not started |
| P1-T13 | Gate 0 runner | Self-consistency, allocator and stdlib checks | Report published | not started |

**Gate M1:** prototype digest equal over 6,220 files and 42 mods; Gate 0 report clean.

---

## P2 — World and environment

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P2-T0a | Adopt `bevy_ecs` | The framework itself: a `World` as the store, `Entity` as the id, components as columns, resources for shared state, and a schedule built as data (D1, D2, D5, D6, D8) | Spawn, query, despawn and schedule tests; a resource rebuilt from the World agrees with it |
| P2-T0b | The entity store | Every thing that exists - people, items, smoke, fire, blood, walls, ground - is an entity in the World, reached by id and never by pointer (D1, D7). A square is an entity too, and it holds no contents (D3). Adding a kind of thing must not mean writing a container for it | Add, find and remove tests; the index rebuilt from the store agrees with it; a fixture proves a new kind of thing needs no new container |
| P2-T0c | Port the map layer onto the World | Smoke, fire and the ground become components on entities; light, transparency and seen become resources written by systems; the private containers are deleted | The existing behaviour tests keep passing unchanged - that is what proves the storage moved and the behaviour did not |
| P2-T1 | Ground of a square | The ground is a thing that never moves; storing it per square is a storage choice, not an architectural one. Uniform-region fast path stays | Storage tests + uniform-region fixtures |
| P2-T2 | Positions and the index | Where each thing stands comes from the store, and the index is rebuilt from it (D2). Nothing outside the store is authoritative about position | Index rebuild tests; no second source of truth |
| P2-T3 | Answers and their caches | Light, transparency, seen, vision, floor, outside: computed by systems, cached, thrown away when a thing they depend on changes | Rebuild tests; dirty-flag invariant I8 |
| P2-T3a | The turn reads the world | `run_environment_turn_from_store` builds the fire rules' question sheet from the store (`MapFacts::from_world`) and takes no map; a world of entities alone takes a turn | **done** - a fire on a square whose facts say its ground feeds ages over a store-only turn; `FireMap` stays as the rule-level seam a replay or unit test uses |
| P2-T3b | The turn runs as a schedule | `crates/sim/src/turn.rs` declares the environment order as chained system sets - scent, falling, fields, light - and `run_turn` executes them; the stages' inputs are components | **done** - two tests: the executed order equals the declared order (`TurnTrace`), and the declared order is written out by hand so a swap breaks it. Mutation-checked: swapping two stages in the chain fails the first test with the swapped order |
| P2-T2a | Radiation as a component on the square | `Radiation` is the last per-square array in the simulation that is not the index - a `BTreeMap` keyed by position. It becomes a value on the square, so `singletons::radiation` reads the store | **done** - `radiation::dose_at` reads the square, `adjust` writes it and takes the component off at zero; the per-square ledger lost its `Radiation` line; the nuke-gas and wandering-field tests read the dose back from the square |
| P2-T4a | The headless game | A binary that runs the turn loop with no window, and a surface to look at the world with: `play`, `store`, `square`, `index` | **done** - `crates/cli` builds and runs; `cdda_sim::inspect` names components from its own table, fingerprints a square by its entities and their values, and `play` reports which squares each turn changed (the demo world's fire ages on every turn) |
| P2-T4b | Scripted headless playtests | A scenario file describes a world and a sequence of turns with assertions, and every committed scenario runs as a test | **done** - `scenarios/*.txt` with seven verbs, `cdda-cli playtest <file>`, and a test that runs every `.txt` in the directory and fails if any assertion does not hold (mutation-checked by changing an expected count) |
| P2-T4c | A capture of the view | The frame the simulation computes, drawn to a PNG that can be looked at and diffed | **done** - `cdda-cli shot FILE --radius N` writes 8-bit RGBA, one cell per square, tinted by glyph and shaded by light, remembered squares notched. The encoder is the port's own (`crates/cli/src/png.rs`), so there is no new dependency and the bytes are reproducible. Two tests: a picture read back out of the file it went into, and the same picture twice is the same file |
| P2-T3 | The turn as a schedule | The turn's inputs stop being parameters: the ground's fire answers are components on the square (`FireFacts`, done for the falling step), the fire and gas processors ask the store instead of a `FireMap` trait, and `run_environment_turn` becomes a `bevy_ecs::Schedule` with declared stages - scent, falling, fields, light | A trace test proves the stages run in the declared order; the fire, smoke and gas replays pass unchanged after each processor moves; no `impl FireMap` survives in `crates/`. **Three findings from two failed attempts, so the next pass starts from them:** (1) two fixtures ask for `adjacent_fires: 7` and `: 9`, but there are only eight neighbours - the store caps the answer at 8, so those two tests have to be re-pinned rather than translated; (2) `levels.rs`'s `Watched` fixture records which squares the map was asked about - a question that stops existing, so it is replaced by the observable outcome (smoke rises to the floor above); (3) `reference_session` feeds `set_items` from the dump's `burns` column while the store derives it from `GroundItem::burn`, so the replay is the place that shows whether the two agree |
| P2-T4 | Smoke, fire and the rest | Smoke, fire, blood and acid are entities in the store, each with a square, a strength and an age; 21 processors decide what they do each turn; decay and spread | Fire/gas/smoke session replays |
| P2-T5 | Illumination | Four-quadrant light, 21 levels | Light session replays |
| P2-T6 | Vision and visibility | Line of sight, memory | Vision session replays |
| P2-T7 | Scent | A smell field is an answer computed per square and diffused by a system, not a thing | Diffusion replay |
| P2-T8 | Sound events | A sound is a short-lived thing - it exists, it travels, it stops - so it goes in the store like anything else | Sound-affected session replays |
| P2-T9 | Temperature | Per-square temperature is an answer; body temperature belongs to the thing that has a body | Cold/heat sessions |
| P2-T10 | Weather | Weather generator and effects | Multi-day weather replay |
| P2-T11 | Traps | Trap placement and triggering | Trap sessions |
| P2-T12 | Zones | Zone data and queries | Zone sessions |
| P2-T13 | Falling and explosions | Falling objects, explosion resolution | Explosion parity (includes balance-sensitive numbers) |
| P2-T14 | Environment replay corpus | 30+ environment sessions | Per-turn hash equality (M2) |

**Gate M2:** environment-only sessions replay equal.

---

## P3 — Items

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P3-T1 | Item state | Item data model | Reference item tests re-expressed |
| P3-T2 | Prototype lookup | Item prototypes and variants | Digest |
| P3-T3 | Containment and pockets | Nested containers, capacity rules | Pocket test parity (largest test file) |
| P3-T4 | Item references | Generational handles replacing locations | Locate-after-move tests |
| P3-T5 | Item groups | Spawn tables and recursion guard | Spawn distribution parity |
| P3-T6 | Inventory | Character inventory operations | Inventory sessions |
| P3-T7 | Crafting | Recipe selection, tools, batches, failure | Crafting sessions + recipe tests |
| P3-T8 | Requirements and qualities | Requirement resolution | Requirement tests |
| P3-T9 | Constructions | Construction data and special functions | Construction sessions |
| P3-T10 | Use actions | 195 use-action functions | Use-action coverage report + sessions |
| P3-T11 | Examine actions | 108 examine functions | Examine coverage + sessions |
| P3-T12 | Consumption, butchery, liquids | The messy interaction paths | Sessions |
| P3-T13 | Auto-pickup | Rules engine | Rule parity tests |
| P3-T14 | Item replay corpus | 70+ item/crafting sessions | Per-turn hash equality (M3a) |

**Gate M3a:** item, crafting and construction sessions replay equal.

---

## P4 — Creatures

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P4-T1 | Creature base | Shared creature state as components | Creature tests re-expressed |
| P4-T2 | Character | Character state, split into components | Character sessions |
| P4-T3 | Monster | Monster state and type lookup | Monster sessions |
| P4-T4 | NPC state | NPC state (behaviour comes in P7) | NPC state tests |
| P4-T5 | Creature index | One position index, no graveyards | Invariants I1–I3 |
| P4-T6 | Effects | Effect storage, stacking, expiry | Effect tests |
| P4-T7 | Body and anatomy | Body parts, hit locations | Anatomy tests |
| P4-T8 | Mutations | Mutation acquisition and effects | Mutation sessions |
| P4-T9 | Bionics | Bionic state and activation | Bionic sessions |
| P4-T10 | Morale, needs, wounds, disease | The survival loops | Survival sessions |
| P4-T11 | Melee and ranged | Attack resolution | Ranged balance tests (tolerance decided) |
| P4-T12 | Projectiles and dispersion | Flight, hits, misses | Ballistics tests |
| P4-T13 | Martial arts, weakpoints, harvest, death | Remaining combat surface | Sessions |
| P4-T14 | Combat replay corpus | 60+ combat sessions | Per-turn hash equality (M3b) |

**Gate M3b:** combat, wound and mutation sessions replay equal.

---

## P5 — Spells and enchantments

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P5-T1 | Spell types and costs | Five cost kinds, learning, levels | Spell tests re-expressed |
| P5-T2 | Casting path | One cast path for creatures, items, traps, bionics | Cast sessions across every cost kind |
| P5-T3 | Shapes, targeting, effects | Lines, cones, blasts, beams | Per-shape scenario tests |
| P5-T4 | Enchantments | Enchantment values on creatures and items | Stat parity tests |
| P5-T5 | Terrain and furniture transforms | Transform effects | Transform sessions |
| P5-T6 | Teleporter lists | Persistent teleport network | Teleport sessions |
| P5-T7 | Relic generation | Procedural relics | Seeded relic digest |
| P5-T8 | Spell replay corpus | 30+ spell sessions | Per-turn hash equality (M3c) |

**Gate M3c:** spell and enchantment sessions replay equal.

---

## P6 — Activities

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P6-T1 | Activity actors | 69 actor types with typed payloads | Activity test parity |
| P6-T2 | Activity lifecycle | Start, resume, suspend, cancel, finish | Lifecycle tests |
| P6-T3 | Activity handlers | Handler registry and dispatch | Handler coverage |
| P6-T4 | Pickup and movement modes | Interaction paths | Sessions |
| P6-T5 | Action pipeline | Keys to intents to actions | Input parity for scripted sequences |
| P6-T6 | Activity replay corpus | 30+ activity sessions | Per-turn hash equality (M3d) |

**Gate M3d:** activity sessions replay equal.

---

## P7 — AI and scripting

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P7-T1 | Monster planning | Target choice, anger, morale | Decision traces |
| P7-T2 | Monster movement | Step choice, bashing, shoving | Movement traces + sessions |
| P7-T3 | Behaviour trees | Node graph, predicates, strategies | Tree fixtures |
| P7-T4 | NPC decisions | 28 actions, scoring, danger assessment | Decision traces |
| P7-T5 | NPC movement and paths | Path following, door handling | Movement traces |
| P7-T6 | Pathfinding | Cost model matching the reference exactly | Route parity over generated maps |
| P7-T7 | Dialogue | Conversation trees and responses | Dialogue tests + sessions |
| P7-T8 | Conditions and effect-on-condition | The scripting language | EOC test parity |
| P7-T9 | Missions | Mission lifecycle | Mission sessions |
| P7-T10 | Factions and trading | Opinion, trade, camp interactions | Trade sessions |
| P7-T11 | AI replay corpus | 40+ AI sessions with traces | Trace equality (M4a) |

**Gate M4a:** per-decision traces and session hashes equal.

---

## P8 — Vehicles

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P8-T1 | Vehicle identity | Stable ids where none exist today | Lookup tests |
| P8-T2 | Parts model | Parts without renumbering hazards | Part tests |
| P8-T3 | Derived indexes | Rebuilt, never stale | Rebuild tests |
| P8-T4 | Movement and collision | Movement order and tie-breaks declared | Driving sessions |
| P8-T5 | Boarding and interaction | Enter, exit, controls | Sessions |
| P8-T6 | Turrets and groups | Turret AI, vehicle spawn groups | Sessions |
| P8-T7 | Vehicle replay corpus | 30+ vehicle sessions | Per-turn hash equality (M4b) |

**Gate M4b:** vehicle sessions replay equal.

---

## P9 — World generation

| ID | Task | Produces | Proven by |
|---|---|---|---|
| P9-T1 | Overmap storage | Dense terrain grid, side tables | Storage tests |
| P9-T2 | Overmap generation | Cities, roads, rivers, specials | Seed-corpus digest |
| P9-T3 | Hordes | Horde storage, movement, absorption | Horde digest over time |
| P9-T4 | Map generation interpreter | The JSON mapgen language | Per-file generation digest |
| P9-T5 | Built-in generators | 12 C++ generators | Parity per generator |
| P9-T6 | Regional settings | Region definitions | Digest |
| P9-T7 | World factory and options | World creation, options | World-creation parity |
| P9-T8 | Basecamps and faction camps | Camp storage and expansion | Camp sessions |
| P9-T9 | Seed corpus | Fixed seed list with expected digests | Digest equality (M5) |

**Gate M5:** same seed produces the same world.

---

## P10 — Interface

| ID | Task | Produces | Proven by | Status |
|---|---|---|---|---|
| P10-T1 | Text layout | Wrapping, folding, markup | Layout parity tests |
| P10-T2 | Window abstraction | Cells, colours, frames | Rendering primitives tests |
| P10-T3 | UI stack and redraw | Frame boundaries and invalidation | Frame-count parity |
| P10-T4 | Menus | Shared menu widget reproducing 254 sites | Per-menu frame scenarios |
| P10-T5 | Prompts | Text and number prompts | Prompt frame scenarios |
| P10-T6 | HUD widgets and panels | Data-driven HUD | Widget value parity |
| P10-T7 | Input and keybindings | 620 actions, contexts, rebinding | Input parity tests |
| P10-T8 | Fonts and themes | Font loading, colour themes | Rendering scenarios |
| P10-T9 | Terminal renderer | Cell output | Semantic frame gate |
| P10-T10 | Graphics renderer | Tiles, effects, overlays | Semantic + pixel gates |
| P10-T11 | Screen ports | Every screen from the inventory | One scenario per screen (M6) |
| P10-T12 | Sound and music | Audio path and packing | Not state-compared; manual checklist |

**Gate M6:** zero differing cells across all screen scenarios.

---

## P11 — Runtime, platform, tooling

| ID | Task | Produces | Proven by | Status |
|---|---|---|---|---|
| P11-T1 | Schedule | Ordered system list as data | Order parity test |
| P11-T2 | Main and command line | Flags, including harness modes | CLI parity test |
| P11-T3 | Action handling | Key to action to simulation | Input sessions |
| P11-T4 | Event bus | Event publication and subscriptions | Event-order tests |
| P11-T5 | UI state | Persistent interface state | Round-trip tests |
| P11-T6 | Debug tools | Debug menu and editors | Smoke scenarios |
| P11-T7 | Capture | Screenshots and dumps | Frame files produced |
| P11-T8 | Diagnostics and crash handling | Error reporting, crash dumps | Fault-injection tests |
| P11-T9 | Platform ports | Targets agreed under blocker B6 | Build + smoke run per target |
| P11-T10 | Packaging | Artifacts per platform | Reproducible builds |
| P11-T11 | Performance gate | Benchmarks with budgets | Report (M7) |

**Gate M7:** shipping artifacts build; performance within budget.
