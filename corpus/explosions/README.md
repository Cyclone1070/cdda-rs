# Blasts

Two recordings of the reference's explosion code, both with the dice handed in rather than drawn.

| File | What it is |
|---|---|
| `blast.json` | Three blasts at the centre of a sealed room, with every square the wavefront bashed on the room's own level, in order, and the force it hit it with |
| `heap.json` | The reference's own `std::priority_queue` draining a fixed list, which is how it breaks ties between squares the same distance away |

## Why the room is sealed with bulkhead

A blast does not stop at a wall it can break: it bashes the wall, the wall becomes open air, and the wavefront continues through the hole. That would make the recording a
function of the bash rule as well as the blast rule. `t_wall_bulkhead` is one of the seventeen impassable terrains the data gives **no bash block at all**, so it cannot be
broken — which is what keeps the blast inside the room and the recording about the blast.

The grid in the dump runs **two squares past the room**, because a square's shaped-charge bonus counts how many of its eight neighbours are open, and the neighbours of a wall
square are outside the room.

## Why the heap is recorded separately

The queue's comparator is `pair_greater_cmp_first` (`src/cata_utility.h:35-41`), which compares the distance and **nothing else**. Squares at the same distance therefore come
out in whatever order the standard library's binary heap holds them — and that differs between libraries: libstdc++ picks the right child on a tie, libc++ picks the left. The
oracle is built with clang and libc++, so `heap.json` is libc++'s answer, and the port's queue is checked against it directly rather than through a blast.

## Regenerate

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-blast corpus/explosions/blast.json < /dev/null
    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --dump-heap-probe corpus/explosions/heap.json < /dev/null
