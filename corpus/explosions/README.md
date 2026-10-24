# Blasts

Two recordings of the reference's explosion code, both with the dice handed in rather than drawn.

| File | What it is |
|---|---|
| `blast.json` | Three blasts at the centre of a sealed room, with every square the wavefront bashed on the room's own level, in order, and the force it hit it with |
| `heap.json` | The reference's own `std::priority_queue` draining a fixed list, which is how it breaks ties between squares the same distance away |

## Why the room is sealed — floor, walls **and** sky

A blast does not stop at a wall it can break: it bashes the wall, the wall becomes open air, and the wavefront continues through the hole. It also **climbs** — it bashes the
square above, and in a room open to the sky it then travels over the wall and comes back down outside, which made the room's own level impossible to reproduce from a model of
that level alone (the first version of this dump got 22 of 360 squares wrong that way, and it took a recorded pop order to see why).

So all three levels are sealed. `t_wall_bulkhead` is one of the seventeen impassable terrains the data gives **no bash block at all**, so it cannot be broken; the interior is
`t_LIXA_thconc_floor`, passable and equally unbreakable, so the wavefront crosses it without changing it. The dump records all three levels, and the grid runs **two squares
past the room** because a square's shaped-charge bonus counts how many of its eight neighbours are open.

## What the dump holds

| Field | What it is |
|---|---|
| `pops` | every square the wavefront popped, in order, with the distance travelled and the force arrived with |
| `bashes` | every square bashed on the room's level, in order, with the force |
| `room`, `above`, `below` | the three levels as they were **before** the blast |
| `other_levels` | how many bashes happened on the two other levels |

## Why the heap is recorded separately

The queue's comparator is `pair_greater_cmp_first` (`src/cata_utility.h:35-41`), which compares the distance and **nothing else**. Squares at the same distance therefore come
out in whatever order the standard library's binary heap holds them — and that differs between libraries: libstdc++ picks the right child on a tie, libc++ picks the left. The
oracle is built with clang and libc++, so `heap.json` is libc++'s answer, and the port's queue is checked against it directly rather than through a blast.

## Regenerate

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-blast corpus/explosions/blast.json < /dev/null
    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --dump-heap-probe corpus/explosions/heap.json < /dev/null
