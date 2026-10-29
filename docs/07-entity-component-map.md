# 07 — Entity and component map

What in the reference is an entity, what is a component, what is a prototype, and
what is just a cache. This is the bridge between [appendix-file-map.md](appendix-file-map.md)
(which files) and the design in [01-architecture.md](01-architecture.md) (what we build).

Full per-class tables:

| Part | Covers | Rows |
|---|---|---|
| [entity-map-world-items.md](parts/entity-map-world-items.md) | Map, world, environment, items, crafting | 136 |
| [entity-map-actors-world.md](parts/entity-map-actors-world.md) | Creatures, vehicles, overmap, factions, scripting | 182 |

Read [03-decisions.md](03-decisions.md) first, section "What the ECS decision means, in
plain words". The three sentences that matter: **things are nouns, systems are verbs, answers
are neither.** Smoke, fire, blood, acid, people, items, walls and the ground of a square are
things. Brightness and visibility are answers a system computes and stores on the square's
entity, never as truth. A cache that disagrees with the store is wrong.

Method: for each stateful class, read its data members and classify. Verdicts are
per class, and mixed classes are split rather than forced into one bucket.

---

## 1. Verdicts

| Verdict | Meaning | Port rule |
|---|---|---|
| Entity | An individually addressable thing that exists in the world | Lives in the one entity store, addressed by id |
| Prototype | An immutable definition loaded from data | Shared, read-only, referenced by id, never copied per instance |
| Component | Plain data describing one aspect of an entity | A column in the store; behaviour comes from systems |
| Cache / index | Derived, rebuildable, never authoritative | Rebuilt freely; if it disagrees with the store, it is wrong |
| System | Logic with no game state | A query-read-write step in the schedule |
| Scaffolding | Build, IO or helper code with no simulation meaning | Port as plumbing or drop |

| Layer | Entity | Prototype | Component | Cache | System | Scaffolding |
|---|---|---|---|---|---|---|
| World, environment, items | 9 | 22 | 24 | 16 | 5 | 27 |
| Actors, vehicles, world-scale | 12 | 29 | 19 | 11 | 5 | 6 |

Prototypes are the largest bucket, which matches the design's assumption: most of
what looks like "game data" is definitions, not state.

---

## 2. Identity — the reference mostly does not have it

The single biggest structural finding. Verified cases:

| Thing | Identity today | Problem |
|---|---|---|
| Monster | **none** | No id member at all; addressed by pointer or by position in a vector, which the code documents as invalidated when monsters are added or removed |
| Vehicle | `om_id`, assigned as container size + 1 with a collision loop ([overmapbuffer.cpp:783-791](../references/Cataclysm-DDA/src/overmapbuffer.cpp#L783-L791)) | Per-overmap, derived from a size, reused after removals; the code comments "this *should* be unique but just in case" |
| Basecamp | position only | Stored by value in a reallocating vector, found by radius scan |
| NPC | counter, reset to 1 on a new game | Lookup is a linear scan; a second competing id exists on the same object |
| Cities, camps, hordes, monster groups | none | Embedded by value in containers, so any pointer into them dangles on reallocation |
| Bionic | `bionic_uid` ([bionics.h:197](../references/Cataclysm-DDA/src/bionics.h#L197)) | **The only clean instance id in the codebase — the model to copy** |

Design consequence: "everything is an id" is not a stylistic preference here, it is
the fix for a defect class the reference lives with.

---

## 3. Ownership — one owner, many borrowers

| Owner | Owns | Evidence |
|---|---|---|
| World submap store | every loaded submap | `map<tripoint_abs_sm, unique_ptr<submap>>` |
| Submap | terrain, furniture, items, fields, traps, its vehicles, its camp | tile arrays plus `vector<unique_ptr<vehicle>>` |
| Prototype factories | every definition | one factory per content type |
| Item | its contents, recursively | pockets own items own pockets |
| Creature tracker | monsters and NPCs, through several containers | see the parts for the full list |

And the borrowers, which are where the hazards live:

- The reality bubble **borrows** submaps through raw pointers: it owns nothing. <!-- verified -->
- Per-level caches **borrow** vehicles through raw pointers, so evicting a submap can
  leave them dangling.
- Items **borrow** their prototypes, and several places treat prototype data as if
  it were per-instance state.

---

## 4. Duplicated state — the list to delete

Each of these is the same fact stored twice. In the port each gets exactly one home.

| Fact | Authoritative | Duplicate / derived |
|---|---|---|
| Trap at a square | tile array | a second full index rebuilt on every change |
| Field at a square | tile array | a hand-maintained counter with consistency assertions sprinkled through the map code |
| Terrain flags | bit set | the same facts again as a string set |
| Item type flags | prototype | per-instance set plus three derived caches |
| Where a specific item is | the container chain itself | a re-resolvable handle type that behaves like a weak pointer |
| Which vehicles are on a level | submap | per-level raw-pointer list, plus a square bitset, plus a square→part map |
| Light at a square | **nothing** | the per-level light array is the only copy, and it is a cache by construction |

The light case is the sharpest: the reference has no authoritative lighting store,
only a rebuildable one. It is settled here: light is a component on the square's
entity, written by the lighting system and overwritten on the next recompute, never
saved and never read as input by a rule.

---

## 5. Prototype leakage

Two verified cases where a prototype carries per-instance state:

- Item type variables: the prototype holds a template that is copied into each
  instance. If any code path writes through the prototype instead of the copy,
  every item of that type changes at once.
- Pocket definitions: instance state mirrored back onto the shared definition, with
  a lint suppression rather than a fix.

The port's rule (prototypes immutable, instances hold only differences) removes
this class of bug, but only if the copy semantics are tested: a test that mutates
one instance and asserts the other instances are untouched.

---

## 6. Open questions this map raises

| # | Question | How to settle |
|---|---|---|
| Q1 | Is light a component or a cache? | **Settled:** a component on the square's entity, recomputed by one system, never saved |
| Q2 | Is the reality bubble an entity, or a query window over the store? | Prototype both; measure the cost of the window's rebuild |
| Q3 | Is scent authoritative (it survives shifting) or per-turn state? | Compare against how the reference discards it on map shift |
| Q4 | Do embedded-by-value collections (cities, hordes, camps) become entities? | Count how often each is addressed individually during a turn |
| Q5 | Do item prototypes need instance variables at all, or is that a missing component? | Audit every write site of the prototype template |

---

## 7. Known limits of this map

- The two largest files (the map implementation and the item implementation) were
  searched and spot-read rather than read end to end.
- Nine ambiguity items remain across the two parts, listed there with what extra
  reading would settle each.
- Class extraction measured member counts mechanically, but "what a member means"
  is still a reading judgement.
- Nothing here was compiled or run: the map describes structure, not behaviour.
  Behaviour is what the gates in [09-testing-plan.md](09-testing-plan.md) measure.
