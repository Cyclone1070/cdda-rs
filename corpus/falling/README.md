# Falling

What leaves a square when the ground under it is destroyed, recorded from the reference.

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-falling corpus/falling/fall.json < /dev/null

`map::process_falling` (`src/map.cpp:3018-3035`) is a queue of squares whose support went away, and `map::drop_everything` (`src/map.cpp:2693-2705`) is four rules in a row on
each: creature, furniture, items, the vehicle, fields. The dump runs the same pair the game does — `support_dirty` then `process_falling` — over five columns.

| Column | What is on it | What happened |
|---|---|---|
| `dx -6` | a rock and a log | both fell one level, onto the floor below |
| `dx -2` | a rock | fell **three** levels: items fall to the lowest floor, not one level down |
| `dx  2` | thick smoke | fell one level at the same intensity, and nothing was left behind |
| `dx  6` | a bench | fell two levels, bashed the floor it landed on, and the bash raised dust |
| `dx 10` | a rock, on a square with its own floor | nothing moved |

Every column is cleared first — terrain, furniture, items and fields — because the first version of this fixture only replaced the terrain and recorded the locker and the
junk items that mapgen had left there, which is a recording of a map with a hole punched in it.

## Reading the record

`before` is the fixture in one line per column: `i:` an item, `f:` a field with its intensity, `u:` furniture. `columns` is every level of every column afterwards, with its
terrain, its furniture, its items in stack order, and its fields with intensity and age.

Two things about the recording that are easy to get wrong:

- **A dead field entry is not a field.** `remove_field` zeroes an intensity rather than erasing the slot, so an unfiltered reading of the map shows a removed field with an
  intensity of one — which is how the first recording of the smoke column came to claim "three moved and one stayed".
- **Blood has `accelerated_decay` and still never lands on a hole**, because `map::add_field` refuses to place a non-gaseous field on a square with no floor
  (`src/map.cpp:6531-6534`). Both halves are data: `corpus/prototypes/fields.json` version four carries `accelerated_decay` and `phase` for all 145 fields.
