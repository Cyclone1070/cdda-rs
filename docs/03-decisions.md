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
| D1 | One entity store is the source of truth | Uniform storage means one way to add a kind of thing, one way to save, one way to query. Two storage classes were the main source of confusion in the reference implementation. |
| D2 | A dense position index is derived, never authoritative | Keeps "what is at this square?" a single step while avoiding two sources of truth. A derived index can always be thrown away and rebuilt. |
| D3 | Empty squares are not entities | Uniformity is about mechanism, not about creating 366,000 objects to represent air. The index has a third state, "unloaded", which makes streaming honest too. |
| D4 | Composition over inheritance | A dog and a wall differ by components, not by class. This is what makes new behaviour additive instead of a subclass tree. |
| D5 | System order is data | A readable, diffable schedule instead of subsystem calls buried in one large function. Ordering becomes reviewable and testable. |
| D6 | Structural changes only at sync points | Iteration stays valid; behaviour becomes reproducible. A turn is already atomic, so batching is natural here in a way it is not in a 60 Hz action game. |
| D7 | IDs, never pointers | Removes the ownership graph that makes large C++ simulations hard to refactor. Also the reason Rust is tractable for this design. |
| D8 | Rust, ECS crate, no game engine | Safety and refactorability at this size, plus a future parallel path. Speed is **not** the reason — see the evidence section. |
| D9 | JSON prototypes, immutable at runtime | The content layer stays data, so mods never have to understand storage. This is the one part of the reference implementation to copy as-is. |

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

### Armour materials are derived, and the derivation is open

An armour that lists no materials takes them from its per-part entries: each
entry names materials with a thickness, and the reference merges entries that
cover the same body part by averaging thickness weighted by how much of the part
each entry's sub-locations cover (`item_factory.cpp:1124-1150`), then turns each
thickness into a hundredth of a unit per covered body part
(`item_factory.cpp:1452-1463`).

Most of this is settled against the data: for a pair of boots, leather at 2.5 mm
over everything but the sole and 0.1 mm over the sole merges to 1.9 mm per foot
and 380 hundredths for the pair, which is exactly what the reference reports, and
it only comes out that way in single precision.

Two things are still open, and the port therefore leaves 69 of 9,912 item types
unverified rather than guessing:

* The weighting needs each body part's sub-location table (`body_part` and
  `sub_body_part` registries, the latter carrying `max_coverage` and `parent`),
  which the port does not load yet.
* One family reports a value the weighted average does not explain: a 1.05 mm
  layer over two arms comes out as 209 hundredths, where every ordering of the
  arithmetic the port has tried gives 208 or 210. Until that is explained the
  rule is not ported.
