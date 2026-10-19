# 02 — The turn loop

## Two shapes of system

A system is always the same five steps (query, read, write, queue, sync). What
varies is the shape of the query:

| Shape | Selects | Used when | Example |
|---|---|---|---|
| **Entity query** | entities having a component set | the work is per thing | creatures act, items tick |
| **Square pass** | squares in fixed order | the *result* is a value per square | light, vision, scent, floors |
| **Global** | nothing; whole-world state | the work is one thing | clock, weather, save |

The square pass exists because some results — brightness, visibility, scent
concentration — are properties of a *place*, not of a thing. Those passes write into
per-square component arrays. Crucially they are still systems: same schedule, same
read/write declaration, same sync rules. The scheduler does not care which shape a
system has.

---

## The turn as an ordered list

One pass of the schedule is one tick. The player may act several times inside it
until their action points run out, which is what makes a "turn" feel like a turn.

| # | System | Shape | Selects | Reads | Writes |
|---|---|---|---|---|---|
| 1 | Clock | global | — | time | time |
| 2 | Weather | global | — | time, climate | weather |
| 3 | Timers | entity | entities with a due timer | due time, time | events fired |
| 4 | Missions | entity | entities with a mission | mission state | mission state |
| 5 | Autosave check | global | — | time, settings | save request |
| 6 | Input | entity | actors with `PlayerTag` + `ActionPoints` | action points | queued actions |
| 7 | Action resolution | entity | entities with queued actions | action queue | targets, tiles; queues changes |
| 8 | Scent deposit | entity | actors with `Scent` | position, scent | scent cell |
| 9 | Scent diffusion | square pass | squares with scent above zero | scent | scent |
| 10 | Floor cache | square pass | squares flagged dirty | tile components | floor and outdoor cache |
| 11 | Physics | entity | `Position` + `Motion` | position, velocity | position; queues collisions |
| 12 | Collision resolution | entity | queued collisions | positions, stats | damage, movement stops |
| 13 | Fire and fields | square pass | squares with active fields | field state | field state; queues damage and spawns |
| 14 | Active items | entity | items with `Active` + timer | item state | item state; queues transforms and destroys |
| 15 | Explosions | entity | entities with `Explosion` | radius, damage | queues damage and tile changes |
| 16 | Field effects on occupants | entity | actors standing in a field | stats, effects | effects, damage |
| 17 | Sound delivery | entity | sound events × listeners | hearing, distance | awareness |
| 18 | Vision cache | square pass | squares with dirty transparency | sight blockers | per-square visibility and light |
| 19 | Creature planning | entity | actors with `Brain` + `ActionPoints` | visible world, goals | chosen step or action |
| 20 | Creature stepping | entity | actors with a chosen step | position, walk cost | position, action points; queues collisions |
| 21 | Off-map agents | entity | entities with `OffMapAgent` | coarse position | coarse position; queues arrivals |
| 22 | Emitters | entity | tiles with `Emitter` | position, tile | spawns fields |
| 23 | Threat display | entity | observers with perception | what they can see | interface state |
| 24 | Player upkeep | entity | actors with `Needs` | needs, temperature | needs, action point refill |
| 25 | Wrap-up | global | — | all pending queues | flush and sync |

This ordering is derived from CDDA's actual turn
([do_turn.cpp:455](../references/Cataclysm-DDA/src/do_turn.cpp#L455)) so
that the sequence is known to be workable. The design change is that it becomes a
list of systems instead of one function.

---

## Storage example

Four squares in a row. Every square is an entity — no exceptions, no separate
storage for scenery:

| Entity | Position | Tile | Field | Container | Items |
|---|---|---|---|---|---|
| #11 | 11,14 | floor | — | — | — |
| #12 | 12,14 | floor | — | — | sock, sock |
| #13 | 13,14 | wall | — | — | — |
| #14 | 14,14 | floor | fire, 3 turns left | — | — |

Actors, in the same store:

| Entity | Position | Actor | Stats | ActionPoints |
|---|---|---|---|---|
| #3 (dog) | 11,14 | yes | 20 hp, bleeding | 100 |
| #7 (player) | 12,14 | yes | 80 hp | 0 |

And the derived index, which no system writes directly:

| Square | Tickets |
|---|---|
| 11,14 | #11, #3 |
| 12,14 | #12, #7 |
| 13,14 | #13 |
| 14,14 | #14 |

Note that squares and actors are indistinguishable in the store: same table, same
component mechanism. The distinction exists only in the index, where a square's own
entity (#12) and the entities standing on it (#7) both appear.

---

## Trace 1 — an entity query (creature stepping, system 20)

1. Query: actors with a chosen step and action points → #3, #7.
2. #7 has no action points — skipped.
3. Read #3's position, 11,14. Ask the index what is at 12,14 → #12, #7.
4. #12 carries `BlocksMovement` (table) → the step is rejected. Cost read from
   `WalkCost`; nothing charged.
5. Write #3's position to 11,15; update the index: remove #3 from 11,14, add to
   11,15.
6. Queue: none.

Every "what is at this square?" was one index read. No entity was scanned.

## Trace 2 — a square pass (fire and fields, system 13)

1. Query: squares whose entity has an active `Field` → #14.
2. Read remaining duration, 3 turns. Write 2.
3. Queue: damage to entities at 14,14 (none this turn).
4. At the sync point, the queued damage is applied; if duration had hit zero, a
   queued destroy would remove the `Field` component and update the index.

Note the shape difference: the system never enumerates all 365,904 squares. It
walks the ordered list of squares that have fields, which is maintained by the
storage layer exactly like the index.

## Trace 3 — a square pass over dirty squares (vision, system 18)

1. Query: squares whose transparency flag was set dirty by a preceding change
   (a wall mined, a door opened) → the two affected squares.
2. Write their transparency into the visibility array.
3. Queue: plain writes only.

This is the technique that keeps a 132 × 132 × 21 world affordable: passes are
proportional to *change*, not to world size. It is copied deliberately from CDDA's
dirty-submap bitsets and per-level light diffing
([map.cpp:9549](../references/Cataclysm-DDA/src/map.cpp#L9549)).

---

## Sync points

Structural changes are queued and applied at sync points. Proposed policy:

| Point | Why |
|---|---|
| End of turn | The default; matches the atomicity of a turn |
| After action resolution (#7) | An item picked up must be visible to the next action in the same turn |
| After collisions (#12) | Destroyed entities must not be stepped on later in the same pass |
| After explosions (#15) | Same reason |

Anything else defers to the end of the turn. If a system needs an immediate sync, it
declares it — it does not reach into the store and mutate during iteration.

---

## Determinism rules for the loop

- One random stream, advanced only by systems, in schedule order.
- Query results iterate in stable order (index order, then insertion order).
- No wall-clock time inside systems; all timing is from game time.
- Systems never read state another system wrote earlier in the same pass unless
  that dependency is declared in the schedule.
