# How hard everything is to break

`info.json` is `--dump-bash-info`: every terrain and every piece of furniture that carries a `bash` block, printed whole and **after inheritance** — 958 terrains and 612
pieces of furniture, 1,570 blocks.

| Fact | Count |
|---|---|
| Bashable terrains | 958 |
| Bashable furniture | 612 |
| Named drop groups | 189 |
| Inline (anonymous) drop groups | 1,278 |
| Blocks whose `sound_fail_vol` is the loader's `-1` | most |
| Terrains whose from-above result differs from their ordinary one | 200+ |

## Why this is a dump rather than a transcription

Half of the block is **absence**, and the reference's absence has three different meanings that are all written the same way:

| Field | `-1` / absent means |
|---|---|
| `str_min`, `str_max` | not given — the loader's default, and a `str_min` of `-1` on a block means unbashable |
| `sound_vol`, `sound_fail_vol` | silent |
| `str_min_blocked` … `str_max_supported` | "use the ordinary strength" |

And the defaults are not uniformly "empty": `ter_set_bashed_from_above` defaults to `ter_set` **only when the object is fresh**, so a carpet that renames its `ter_set`
still bashes from above into the `t_floor` its parent resolved; furniture's `furn_set` defaults to the real id `f_null`; and a block that names no items gets the real group
`EMPTY_GROUP` rather than nothing.

Regenerate with:

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-bash-info corpus/bash/info.json < /dev/null
