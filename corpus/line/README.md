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
