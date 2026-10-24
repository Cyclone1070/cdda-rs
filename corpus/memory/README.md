# Map memory

What the character keeps in their head about a square, recorded from the reference.

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-memory corpus/memory/tiles.json < /dev/null

`memorized_tile` (`src/map_memory.h:22-54`) holds the terrain **and** a decoration, each with its own subtile and rotation, plus the glyph the square was drawn with. Four writes
fill it and each preserves the rest of the tile, because `map_memory` reads the tile out, changes one field and writes it back (`src/map_memory.cpp:218-275`).

The interesting one is the clear, and the scripted sequence exists to exercise every branch of it:

| Square | Written | Cleared with | Recorded |
|---|---|---|---|
| `0,0` | terrain only | — | `t_floor`, rotation 1, subtile 2 |
| `1,0` | terrain, `vp_frame`, symbol `X` | `vp_` | decoration **gone**, symbol **0** |
| `2,0` | terrain, `graffiti_hello`, symbol `Y` | `vp_` | decoration **kept**, symbol **89** (`Y`) |
| `3,0` | `f_bench`, symbol `Z` | `""` | decoration gone — the empty prefix matches everything |
| `4,0` | terrain, `f_chair`, then terrain again | — | `t_grass`, decoration **kept** |

So a clear that matches takes the symbol with it, because the glyph belonged to the decoration; a clear that does not match changes nothing; and an empty prefix matches
every id, which is how the reference forgets every decoration in a region at once.

`prepare_region` has to run before the writes: a memory submap that was never allocated refuses them (`if( !sm.is_valid() ) return`), so a dump that wrote first would record
nothing and look like a rule about writing.
