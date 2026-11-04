# 03 — Decisions and rationale

Scope: a from-scratch design. Migration cost from any existing codebase is
explicitly **not** an input. Where the existing game is cited, it is as evidence
about what works at this scale, not as a constraint.

---

### Colour names are pinned to the 256-colour table

The reference builds two colour tables and chooses on `COLORS >= 256`, which
ncurses only answers once a terminal exists. The tables disagree on names, not
values: in the fallback table `c_dark_gray_red` and `c_black_red` are the same
colour. The port targets the table a player with a modern terminal sees, and the
harness pins it (`ncurses_def.cpp`, under `CATA_HARNESS`), so a dump never
describes a table no player has.

## Decisions

| # | Decision | Rationale |
|---|---|---|
| D0 | **The architecture is absolute** | The port translates the reference's rules into this architecture and never imports its structure. A shape that breaks a principle is wrong even when its output matches, because everything added afterwards builds on whatever shape is already there. Enforced by the gates in `crates/sim/tests/sim_suite/architecture.rs`, not by good intentions. |
| D1 | One entity store is the source of truth | Uniform storage means one way to add a kind of thing, one way to save, one way to query. Two storage classes were the main source of confusion in the reference implementation. |
| D2 | A dense position index is derived, never authoritative | Keeps "what is at this square?" a single step while avoiding two sources of truth. A derived index can always be thrown away and rebuilt. |
| D3 | A square is an ordinary entity, and it holds no contents | A square is a place, not a container. It carries `Position` and the values computed for it, and nothing else: no terrain slot, no furniture slot, no trap slot, no item slot. Everything standing on it is a separate entity with the same position. One storage shape for squares and for the things on them means one way to add a kind of thing, one way to query, one way to save. |
| D4 | Composition over inheritance | A dog and a wall differ by components, not by class. This is what makes new behaviour additive instead of a subclass tree. |
| D5 | System order is data | A readable, diffable schedule instead of subsystem calls buried in one large function. Ordering becomes reviewable and testable. |
| D6 | Structural changes only at sync points | Iteration stays valid; behaviour becomes reproducible. A turn is already atomic, so batching is natural here in a way it is not in a 60 Hz action game. |
| D7 | IDs, never pointers | Removes the ownership graph that makes large C++ simulations hard to refactor. Also the reason Rust is tractable for this design. |
| D8 | Rust, **`bevy_ecs`**, no game engine | Safety and refactorability at this size, plus a future parallel path. Speed is **not** the reason — see the evidence section. |
| D9 | JSON prototypes, immutable at runtime | The content layer stays data, so mods never have to understand storage. This is the one part of the reference implementation to copy as-is. |

---

## What the ECS decision means, in plain words

This section exists because the table above was read wrongly by the person who wrote it. If
you take one thing from this document, take this.

### The framework is `bevy_ecs`

Not "an ECS crate": **`bevy_ecs`**, added to the workspace as `cdda-sim`'s dependency. It is the
first third-party dependency in the project and it is the backbone of everything below - the
entity store, every component, every system, and the schedule itself.

Why this one, in its own terms:

| Decision | What `bevy_ecs` gives |
|---|---|
| D1 one store | a `World` is the store; `Entity` is the id |
| D2 derived index | a component holding the square index, on one entity, rebuilt from the store |
| D5 schedule is data | its `Schedule` is data: built in one place, inspectable, reorderable |
| D6 structural changes at sync points | commands are applied at defined boundaries, not mid-iteration |
| D8 no engine | a plain library: no renderer, no window, no application loop |

`hecs` was the alternative: smaller, but no scheduler, and D5 says the schedule must be
readable data rather than hand-written calls. `shipyard` buys parallelism that the turn loop
does not need yet.

**This is what slipped.** D8 said "an ECS crate" and no phase task said "adopt one", so every
task was satisfiable without a framework and nine rounds of hand-rolled storage were built
instead. The rule that comes out of it: *an abstract decision with no named artefact and no
task that adopts it is a wish, and wishes do not show up in a diff.*

**Things are nouns. Systems are verbs.**

* **A thing** exists in the world: it can be created, moved, damaged, seen, picked up, burnt.
  Smoke, fire, blood, acid, a person, an item, a wall, and the ground of a square. **Every one
  of them is one entity in the one entity store** (D1), and the index says where each stands (D2).
* **A system** is a step that does a job each turn: move things, spread fire, work out light,
  work out what a person can see. Systems are scheduled in a readable order (D5).
* **An answer** is what a system produces: how bright this square is, whether this square is
  visible. Answers are recomputed and cached; they are not inhabitants of the world.

The test for "is it a thing": *does anything act on it, and does it keep an identity over
time?* Nothing burns light, carries it, waits for it or puts it down. A torch is a thing; the
bright patch it throws is an answer produced by the lighting system.

### What this does not mean

* **A square is not a container.** It has no slots for terrain, furniture, traps or items. A
  square carries `Position` and the values computed for it, and nothing else. Everything
  standing on it is a separate entity whose position equals the square, so a square can hold
  any number of things and knows nothing about any of them (D3).
* **Ground is a thing like any other, so the ground is an entity.** It can be looked at,
  walked on, bashed, dug and burnt, and all of those are rules acting on it. It is spawned
  with the level, sits on its square like every other thing, and its terrain, furniture and
  trap values are components on it. It is not exempt and it is not stored off to the side.
* **What may share a square is decided by the entities, not by the square.** An entity
  carries its own rules as data: what it admits on its square, what it needs under it, and
  how much it can support. One placement system asks the entities already there whether a
  newcomer fits. Adding a kind of thing means giving it components, never editing a list of
  kinds or adding a slot to a square.
* **It does not mean separate containers per kind.** A container for creatures, another for
  items, another for fire is exactly the shape this decision rejects (see B below). Adding a
  kind of thing must not mean writing a container for it.
* **It does not mean computed values are things.** Brightness, visibility and "is this square
  under a roof" are components written by one system and overwritten on the next recompute.
  They live on the square's entity because that is the addressable place to put them, but no
  rule reads them as input and they are never saved.

---

## Rejected alternatives

**A. Keep the current object-oriented-plus-grid design.**
Legitimate, and it demonstrably runs this game. Rejected for a greenfield build
because state ends up in one enormous character class (CDDA's is 4,452 lines with
~330 data members and is included by 185 of 462 translation units) and because new
mechanics must be threaded through it rather than added as components.

**B. Grid only, no entity store.**
What the current world data mostly is: flat per-tile arrays plus a handful of
special-cased creature and vehicle containers. Cheap and fast, but every new kind of
thing needs new container code, and "everything is a thing" is lost.

**C. Everything in one entity table, with no position index.**
The uniform version, and it fails on the most frequent question in the game.
Finding what is at a square means scanning entities. A position index is required
either way; the only question is whether it is the primary structure or a derived
one. This design makes it derived, which was the last unresolved point and is
recorded in D2/D3.

**D. A full game engine (Bevy, Godot, Unity).**
An engine brings rendering, audio, windowing and an application loop — and takes
over the part of this game that is largest and most distinctive: a text and tile
interface built around dense menus. Only the ECS and the scheduler are wanted, so
only those are taken.

**E. C++ instead of Rust.**
Would work. Rejected because the win here is not code generation — the two
languages measure within roughly ±10% — but the ability to change a 500k-line
simulation without breaking it silently. That is precisely what borrow checking and
index-based ownership buy.

**F. Push all rules into a scripting language.**
Attractive for modding, but the reference implementation already shows the ceiling:
thousands of data-driven behaviour definitions coexist with large amounts of
hardcoded logic for the parts that need world access. Systems in the host language
with a data-driven content layer is the balance chosen here.

---

## Prior art: this was argued before

The reference project debated ECS in February 2017 and dropped it. That history is
worth recording, because the objections were specific and this design answers them
in specific ways.

Thread: [Evaluating the Entity Component System model for DDA](https://discourse.cataclysmdda.org/t/evaluating-the-entity-component-system-model-for-dda/13405)
(opened 2017-02-15, moved from [#20069](https://github.com/CleverRaven/Cataclysm-DDA/issues/20069)).

**The bubble question was asked then, exactly as it came up here.** A contributor
wrote:

> "If done correctly, though, it could allow expanding the reality bubble."

and the maintainer answered:

> "The thing keeping us from making the reality bubble larger is scaling of various
> algorithms that act on it, such as field of view and dynamic lighting
> calculation, I don't see how an ECS would have any impact. On the contrary, if
> you look at the key data structures for the reality bubble, they're fully
> decomposed into multidimensional arrays of values for efficiency. An ECS would
> have a lot [of] trouble beating that."

That is correct for the 2017 proposal, and it is answered here by **not replacing
the arrays** — the position index and the per-square value arrays remain. ECS
becomes the owner of the entities, and the arrays become its index, rather than two
competing structures.

**The structural-change objection** was also raised, by the maintainer himself, when
he prototyped an archetype allocator in the same thread:

> "adding or removing a component from an entity would require allocating a new id
> and copying all components to a new location… it might make it totally unusable."

This design answers it with sync points (D6) and with the fact that the workload is
turn-based, so structural changes batch naturally rather than needing to be visible
mid-frame.

**The scope objection** — *"MASSIVELY invasive… tantamount to a total redesign"* —
is not answered at all, and is not meant to be. It is the reason this is a
greenfield design and not a proposal to change an existing codebase.

**Adoption signal:** nine years later the project tracker has **0 issues with "ECS"
in the title** and 4 incidental mentions of "entity component" in unrelated work;
the fork with real maintenance budget (Cataclysm: Bright Nights) stayed in C++ and
added a CI performance regression gate instead.

---

## Evidence: what is measured, what is judgment

| Claim | Status |
|---|---|
| Rust and C++ are within ~±10% on comparable computation | **Measured.** Benchmarks Game ratios; the closest real C→Rust port (`rav1d`) was 3.8% slower transpiled, <6% slower after optimisation |
| ECS gives no measurable benefit below ~1,000 entities | **Measured.** Archetype vs sparse-set indistinguishable at 100 and 1,000 entities (p = .49, .376); 1.7–1.9× at 10k–50k homogeneous entities |
| ECS retrofit can make some systems slower | **Measured.** In a shipped commercial retrofit, the bulk system was 4.5× faster and the engine-coupled one 1.6× slower; the studio declined a full conversion |
| Per-turn cost scales with world area | **Verified in code.** Every turn sweeps the bubble; light and vision are skipped or diffed when clean |
| The reference implementation is already data-oriented where it matters | **Verified in code.** Struct-of-arrays tiles, enum bit-set flags, flat A* arrays, spatial zone cache, active-item and dirty-submap caches, 1.5 MB-class per-square caches |
| An entity store with a derived index is fast enough at this scale | **Judgment.** No prototype exists. This is the measurement to run first |
| Rust will reduce defect rate in a simulation of this size | **Judgment**, consistent with the safety rationale but not measured here |

The honest summary: **the performance case for this design is unproven, and it is
not the reason for it.** The case is uniformity, composability and refactorability.
Speed comes from the schedule and the dirty-tracking discipline, which are
independent of ECS.

---

## Reference defects the port must decide about

Found mechanically while extracting the options table. Each is reproduced by the
reference today, so each is a choice: copy the behaviour, or diverge and say so.

| Defect | Evidence | Decision needed |
|---|---|---|
| A default outside its own option's value list | `MORALE_STYLE` items are lower case, default is `Vertical`; reference silently falls back to the first item (`options.cpp:2151`) | Copy the fallback, or fix the data |
| Same, second case | `ASTERISK_POSITION` items are `prefix`/`suffix`, default `right` (`options.cpp:2216`) | Same |
| A setting that is silently dropped | `DEFAULT_REGION` is a `string_input`, but regions are added with `add_value`, which only extends `string_select` (`regional_settings.cpp:1189`) | Copy the dead setting, or wire it up |
| One option registered twice | `RENDERER`, once per build profile (`options.cpp:2673`, `:2687`) | Not a defect in a single build; note it for the port's table |

The port's default is to reproduce observable behaviour, so the first three are
copied unless a decision says otherwise; each gets a test that pins the fallback.

## Settled since the plan was written

| Decision | Choice | Consequence |
|---|---|---|
| Reference save compatibility | **Out of scope.** The port writes its own save format | Session start moves to the state dump, which becomes an input format as well as a comparison output. No legacy fix-up paths to transcribe. See [05-roadmap.md](05-roadmap.md) B2 |
| Platform scope | **Desktop only for now**: macOS, Linux, Windows from one codebase. Android and web deferred, not cancelled | Measured cost per platform, and the seven constraints that keep the deferred targets reachable, in [12-platforms.md](12-platforms.md) |

## Open questions

| Question | Why it matters | How to settle it |
|---|---|---|
| Is a chunk an entity, or just a range in the store? | Affects streaming code and whether systems can address chunks directly | Prototype both; measure load and wake cost |
| Do square passes write into per-square component arrays inside the store, or into separate arrays the store owns? | Determines whether there is one source of truth or two | Design the visibility pass both ways and count the indirections |
| What is the save format, and how are component migrations versioned? | Save compatibility is this genre's hardest long-term constraint | Start with a text format and a migration table; never change it without one |
| How much behaviour becomes data (script) versus systems (host code)? | Determines modding power and how much C++/Rust contributors must touch | Track the ratio of new mechanics implemented as data per release |
| When is parallelism allowed, and which systems are actually independent? | The read/write declarations exist for this | Record declarations from day one; defer any threading until a profile demands it |
| What entity count is the design targeting? | It is the number that decides whether ECS earns its keep | Instrument turn time vs entity count in the first playable build |
| What ordering does a tie get when the reference has none? | Pointer-order tie-breaks and hash-order iteration exist today; the port cannot reproduce them and should not try | Pick one canonical order per case, list them as declared divergences |
| Do the 188 C++ option declarations become data (276 total with the JSON ones)? Mods can only ship three option files today; moving options into data is a modding gain and a compatibility risk | Decide with the registry phase; measure how many options mods actually override |
| Which reference warts are deliberately not reproduced? | Two are known: a shared static object handed out by uniform map regions, and a graveyard that keeps dangling creature references alive | Add each to the equivalence spec as an explicit, reviewed exception |

---

## What to keep from the reference implementation

Honest answer: most of its performance engineering is independent of its
architecture and should be copied.

- Prototype registries loaded from JSON, immutable at runtime.
- Struct-of-arrays per-square storage with a fast path for uniform regions
  ([submap.h:62](../references/Cataclysm-DDA/src/submap.h#L62)).
- Bit-set flags with an enum fast path instead of string keys
  ([mapdata.h:228](../references/Cataclysm-DDA/src/mapdata.h#L228)).
- Active-only and dirty-only processing: per-square and per-level dirty bits decide
  what gets rebuilt ([level_cache.h:26](../references/Cataclysm-DDA/src/level_cache.h#L26),
  [map_field.cpp:167](../references/Cataclysm-DDA/src/map_field.cpp#L167)).
- Pool storage with stable references for numerous small objects — a vendored copy of
  `plf::colony` ([colony.h:74](../references/Cataclysm-DDA/src/colony.h#L74)).
- JSON parsed once and cached as flat binary buffers
  ([flexbuffer_cache.h](../references/Cataclysm-DDA/src/flexbuffer_cache.h)).
- A CI performance regression gate on a fixed scenario and seed.

## What not to keep

- A single character class carrying hundreds of members.
- A turn function that hardcodes the order of twenty subsystems.
- String-keyed behaviour dispatch in per-turn paths.
- Virtual-heavy interfaces on types that only ever have two or three implementations.

### Armour materials are derived, and four items still differ

An armour that lists no materials takes them from its per-part entries. The port
does this with the body-part and sub-location registries loaded, and the reference
now dumps its own consolidated rows (`--dump-items`, field `armor_data`) so the
model can be checked rather than inferred.

What the rows settled:

* One row per body part. Entries covering the same part merge into it: a material
  both name keeps a thickness averaged by how much of the part each entry covers,
  a material only the new entry names is appended unchanged, and the row's own
  coverage only grows *after* an entry has been merged into it.
* The first entry makes the row, and its materials are copied **whole** - so a
  helmet that pads the same material twice lists two shares, not one average.
* Thicknesses add up before being turned into portions, in single precision, and
  are truncated once. Truncating per body part gives 208 where the reference says
  209 on a 1.05 mm layer over two arms.
* `replace_materials` for a material the item does not have leaves that material
  behind with a share of zero, because the reference reads the old share through
  `materials[ old ]`, which inserts it (`item_factory.cpp:4007`).

That closed the last four differences. They were not float boundaries as first
recorded: two were the per-row truncation above, one was `extend` reaching the
material list, and one was the order in which allergen flags are read.

## Combining two halves of a split coordinate

The reference refuses to combine two three-dimensional points because it cannot
say which `z` wins (`coordinates.h:837`). The port keeps one three-dimensional
point type instead of separate point and tripoint types, so `combine` takes the
`z` of the coarse cell. That is the only reading that cannot disagree with the
reference: `z` is never scaled, so the coarse and fine halves carry the same
number anyway.

## The error function is the platform's, not ours

`normal_roll_chance` is arithmetic rather than a draw, and the reference takes
`erf` from the platform's libm. The port cannot borrow it, so it carries its own
approximation, which agrees with the reference to within 4e-8 in single precision
over the rows that were measured. The centre case is exact: a difficulty on the
centre of the curve is exactly one half, so it never reaches the approximation.

That is a declared divergence rather than a hidden one. It changes an outcome
with probability 3e-8 per call, which is far below the granularity of anything
the game rolls against, but it is a difference and the ledger says so.

## The dice follow the oracle's standard library

The standard library's distributions are implementation-defined: the same seeds produce different draws under libc++ (macOS) and libstdc++ (gcc), and `corpus/rng/` holds the two side by side — 64 of 98 draws apart. The port follows **libstdc++**, because the oracle every recording comes from is a gcc build. The consequence to state plainly: a port held to these numbers reproduces the Linux oracle, not a macOS build of the original. CDDA itself does not promise cross-platform dice, so there is no third option to match; choosing the oracle is choosing the only reference we can record.

## A loader is fed the shape it was written for

**Decision.** Every content loader names the source it reads and the shape that source has. A loader written against a **dump** (`cdda_content::monsters`, which reads one number per
damage type and calls the dice faces `melee_sides`) is fed the reference's dump; a loader written against the **game's own JSON** (`cdda_content::items`, which reads `melee_damage`
objects and the nested `armor` form) is fed the game's JSON. Nothing is fed a source it was not written for, and nothing quietly falls back to a default when a field is missing in a
shape it did not expect.

**Why.** Round 1100 found the CLI feeding the monster loader the game's own JSON. The two shapes disagree on field names - `monster.h`'s member is `melee_sides`, the game's JSON
member is `melee_dice_sides` (`monstergenerator.cpp:813`) - and on structure (armour is a nested `armor` object in the JSON, one flat number per type in the dump). **The loader did
what any tolerant loader does: it took the fields it recognised and left the rest at zero.** Every monster in every playtest therefore swung **`2d0`**, carried no stab armour and
looked entirely healthy: a full set of type records, a full set of flags, a full set of hit points, and no fight.

**What this rejects.** *"Be liberal in what you accept."* A tolerant reader turns a wrong file into a wrong world with no error anywhere, and this port's whole method is that a zero
must be a **fact from the data**, not the absence of one. It also rejects a single generic loader that sniffs the shape of its input: the reference has two shapes because two
different things write them, and a loader that guesses which it has cannot say which rules it is applying.

**How it is kept.** Three defences, each found by a round that paid for it:

| Defence | Catches |
|---|---|
`content_suite::item_melee_parity` | a loader that disagrees with the dump on **9,912** items |
`creature_fight::a_monster_that_reaches_the_survivor_hurts_it_in_the_turn` | a scheduled blow that cannot hurt anyone |
`cli::every_table_a_scenario_installs_is_filled_from_the_source_its_loader_speaks` | a table installed from the wrong shape: a zombie's dice faces, some monster's stab armour, some monster's bashing armour |
Playtest report lines | a monster's **dice and accuracy**, printed where a reader can see `2d0` |
