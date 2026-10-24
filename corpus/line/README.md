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
