# Environment-only field sessions

A **field session** is the smallest world that can still behave: no creatures, no items, one or more field entries, and the map's
own field sweep running a turn at a time. It is the fixture behind the P2 exit gate's "per-turn state hash on environment-only
sessions", and it was built because a hand-computed table cannot tell whether a fire's *spread* matches - only a recording can.

## Regenerating a session

```sh
oracle/build/src/cataclysm \
    --userdir oracle/userdir \
    --datadir "$PWD/oracle/src/data/" \
    --world harness-fixed --seed 12345 \
    --dump-field-series corpus/fields/fire-floor.json \
    --place-fire 0 0 2 --field-turns 5
```

* `--place-fire X Y I` is an offset **from the player**, not an absolute square, so a fixture does not move when the player's start
  square does.
* `--seed` is mandatory in practice. Without it the run takes `time( nullptr )`, the fixed world is generated differently every time,
  and two dumps that should be identical are not. The first attempt at this fixture was unseeded, which is why its numbers initially
  looked impossible.
* `--field-turns N` runs `map::process_fields()` N times and writes the state after each one, plus the state before the first.
* Every row carries the **terrain and furniture** under the field. That is not decoration: the fire processor's fuel accounting branches
  on the terrain's flags, and a dump without them cannot distinguish "the fire gained fuel" from "the fire burned the ground".

## `fire-floor.json`, and what each number is

`t_floor` is `FLAMMABLE_HARD`. Intensity two, placed on the player's own square, five turns:

| Turn | Age | Why |
|---|---|---|
0 | 0 | placed with `add_field( ..., 0_turns )` |
1 | 1 | the field's own `do_decay()` adds one turn per turn |
2 | 22 | no ground fuel this turn: `mod_field_age( 10_seconds x intensity )` adds 20, decay adds 1 |
3 | 43 | same again |
4 | 42 | the ground fed this turn: `FLAMMABLE_HARD` and `one_in( 3 )` give `time_added = 1_turn x ( 4 - intensity ) = 2`, which is **subtracted**, and decay adds 1 - so the age goes **down** by one |
5 | 123 | it spread (paying `1_minutes` = 60) and a **new fire** appeared at age **120** |

The new fire's 120 is `add_field( dst, fd_fire, 1, 2_minutes )`: a field's age is time **burned**, so the newborn arrives already two
minutes old, which is what makes it weak. The port had that sign inverted until this fixture was read.

That single downward step is the whole reason this file exists. An age that goes **down** while nothing is being extinguished looks like a
bug in a port; it is the reference feeding on the ground. A test that only checked "the fire ages" would have passed with the wrong rule.

## `fire-floor.rolls.txt`: the dice that made it

Run the same command with `CDDA_RNG_TRACE` set and the reference writes every die it rolls:

```sh
CDDA_RNG_TRACE=/tmp/rolls.txt oracle/build/src/cataclysm ... (same arguments as above)
```

The session JSON carries `draw_start`: the reference's draw counter when the session began. Everything after that line in the trace is the
session's own dice, and it is checked in here as `fire-floor.rolls.txt`. The file also contains **marks** - `mark fire.before_fuel`,
`mark fire.fuelled`, and so on - written by the fire processor itself (`rng_mark`), so the dice can be attributed to a rule rather than guessed at.

That is what makes dice parity testable. `core_types_suite/fire_ground.rs` reads the four `fire.before_fuel` phases and asserts that the port's
`ground_fuel` asks for **exactly those dice, in exactly that order**, and reaches the reference's answer:

| Turn | Recorded draws | What they are | Result |
|---|---|---|---|
1 | `int 0 2 1` | `one_in( 3 )` failed | no fuel |
2 | `int 0 2 1` | failed | no fuel |
3 | `int 0 2 0`, `int 0 99 11` | `one_in( 3 )` fed, then `one_in( 100 )` for bashing | two turns of fuel, no bash |
4 | `int 0 2 1` | failed | no fuel |

The order is the point: a port that rolled the bash die first would shift every later draw in the turn.

## The grid, added in round 324

The dump now carries a `"grid"` array: the terrain and furniture of **every** square in the radius, not only of the squares that happen to hold a field.
Without it a replay has to guess the ground under the fire's neighbours, and the guess is invisible - the fuel branch, the spread rule and whether smoke may
rise all read terrain, and a port that reads "nothing" everywhere looks plausible and spreads differently.

The dump is 49 squares (`radius 3`), seven by seven, and the session's fire sits at `(69, 60)`: the room is `t_floor`, the walls are `t_wall_w`, and the
fixture is regenerated with the command at the top of this file.

## Two fixtures, and why (round 327)

`fire-floor.json` puts the fire on the **player's own square** (`--place-fire 0 0 2`). That is a legitimate scene - a person standing in a fire - but it is
not a fixture about fields: every `add_field` the hot-air rule makes lands under them, and the reference spends a hundred draws rolling their body parts.
Those draws belong to the creature layer, which the port has no entities for in this session.

`fire-floor-offset.json` moves the fire one square east (`--place-fire 1 0 2`). Nothing changes but the scene: same room, same fire, same five turns, and now
the session's dice are the fields' own from the first draw to the last, which is what `fire_session_parity` replays with nothing carved out.

Keep both. The first is the better fixture for the creature layer the day it is ported; the second is the one that proves the field rules.

## Placing the scene, not just the fire (round 329)

The dump can now be told what to stand the fire in and what to put around it, because a fixture about one branch of the rules needs the other branches out of
the way:

| Option | What it does |
|---|---|
`--place-items <x> <y>` | puts a plank on a square - the spread rule's last disjunct is `one_in( 5 ) && get_item_count() > 0 && flammable_items_at( ... )`, and without an item no fixture can reach it |
`--place-terrain <x> <y> <ter>` | sets the ground on a square; repeatable, so a fixture can bury a fire in `t_concrete` and leave exactly one square catchable |
`--place-furniture <x> <y> <furn>` | sets the furniture; `f_null` clears it - a bench is `FLAMMABLE_ASH`, and clearing the ground while leaving the bench standing isolates nothing at all |

Every grid square in the dump now carries `"items"` and `"items_burn"`, so a replay knows what a fire could have caught besides the ground.

## The isolated plank (`fire-plank`)

The item branch only shows itself when **nothing else** on the scene could catch, so this fixture removes everything else: the fire stands on `t_floor` and
all eight of its neighbours are `t_concrete` with `f_null`, except one square that holds a plank. The plank square is the only square a fire can reach, and
`t_concrete` has no flammable flags of any kind (`prototype` row: `FLAT`, `MINEABLE`, `ROAD`, `TRANSPARENT`).

Confirmed by mutation: with the item branch disabled the replay fails on turn 6 - `[(70, 59, 2, 84)]` against the reference's
`[(69, 59, 1, 120), (70, 59, 2, 144)]` - and with it enabled it passes. The bench fixture this replaced (`fire-items`) did **not** fail under the same
mutation, because a bench is `FLAMMABLE_ASH` and the ground rule caught the square without any help; it has been deleted rather than kept as a test that
cannot fail.

The session stops at the spread on purpose. From the next turn the newborn starts **eating the plank**: over sixty turns the reference's own state shows the
newborn's age going 120 then 119 then 120 - one turn of fuel from the item, then nothing - and the port has no `item::burn` and no `fire_data` fuel to
produce that. That is a rule to port, not a fixture to cut: the cut is recorded here so the next session can extend this same file.

## What the dice stream leaves out (`CDDA_RNG_TRACE_REALS`)

`rng_float` draws are **not** in these files. The trace records them only when `CDDA_RNG_TRACE_REALS` is set, because a replay can only spend a real if the
port makes every real the reference makes, in the same order - and today it does not: bash and rubble draw reals the port has no rule for. With the switch on,
`fire-plank` fails at the first real inside the spread loop; with it off, the same file replays to its last integer die. The opt-in exists so that the day the
remaining float rules land, the corpus can be recorded strictly without touching the harness again.

## `gas-60.json`: where a gas goes

```sh
oracle/build/src/cataclysm \
    --userdir oracle/userdir \
    --datadir "$PWD/oracle/src/data/" \
    --seed gaswatch \
    --place-fire 0 0 0 --place-gas fd_toxic_gas 3 \
    --field-turns 60 \
    --dump-field-series corpus/fields/gas-60.json
```

A fire intensity of **zero** means the fixture is about the gas alone: a fire would add smoke of its own and there would be no telling the two apart. The
sixty turns are what it takes for `fd_toxic_gas` to beat its own spread chance - ten percent under a light wind - and the split at turn 48 is the thing worth
recording: the source drops from intensity three to two and its age falls from 47 to 33, while the destination arrives at intensity one and age 16. Those
numbers are the spread rule's arithmetic, not a rounding, and a port that gets them wrong looks plausible for forty-seven turns and then is visibly wrong.

Each turn carries the **wind** (`windspeed`, `winddirection`, and the local `windpower` at the centre), because the spread chance is `rng( 1, 100 - windpower )`
and the destination is resisted when it blows into that wind: the dump's dice say the ceiling moves between 100 and 94, and without the wind beside them there is
no way to say why. The field dump reads **three levels**, so a gas that falls or rises is still recorded; none of the levels is touched in this fixture, which is
itself the fact worth knowing - a gas on a floored square drifts rather than falls.
