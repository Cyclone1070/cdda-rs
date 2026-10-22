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
