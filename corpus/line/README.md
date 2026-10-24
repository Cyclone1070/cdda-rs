# The three-dimensional line walk

`walks.json` is twenty `line_to` walks recorded with `--dump-line`: one endpoint pair per branch of the three-dimensional
`bresenham` (`src/line.cpp:73-215`), then several of them repeated with starting error terms of both signs.

The walk has ten branches — three for a flat line (split by which level axis is longer) and seven for one that climbs
(split by which axis dominates and which two are equal) — and each steps differently, so a sample that only walks a
diagonal says nothing about the other nine. Every case carries the name of the branch it is meant to exercise, and the
test checks that all ten names appear.

The error terms are in the file because they are not decoration: `flat-x` with `t = 3` takes its first step north-east
and with `t = -3` east, which is exactly the tie-breaking the walk relies on.

Reproduce with:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-line "$PWD/corpus/line/walks.json"

## `closer.json`

Fourteen `squares_closer_to` answers (`src/line.cpp:700-733`), which is what monster movement walks when it picks a step
(`src/monmove.cpp:1128`): the four shapes (x-dominant, y-dominant, diagonal, and a climb), each walking backwards,
the degenerate straight runs, and a mover that is already at its target.

The order in the file is the order the reference offers the squares in, and that is the point: a creature takes the
first square it can enter, so the same squares in a different order is a different move. Swapping the two flanking
squares in the x-dominant branch fails the first case.

Reproduce with:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-squares-closer "$PWD/corpus/line/closer.json"

## `clear_path.json`

Twenty answers from `map::clear_path` (`src/map.cpp:7723-7800`) — ten shapes under two cost windows — on a map built
square by square: a 31-square floor at z=0, a wall column at `x = 5` from `y = -4` to `y = 4`, and a floor above it
except at `x = 8`, which is open air.

Three habits of the rule are the reason the map is shaped that way:

- the **last** square is never checked, so a target standing *on* the wall is a clear shot;
- the flat branch compares the move cost **inclusively** and the climbing branch **strictly**, so a window of a single
  value can carry a flat shot and never a climb — which is exactly what the two windows, `2..=2` and `1..=3`, show;
- a climb is allowed if either column has support at the higher level, which is why one column above is floor and
  another is open air.

Reproduce with:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-clear-path "$PWD/corpus/line/clear_path.json"
