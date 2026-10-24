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

`hits.json` is `--dump-bash`: eight fixtures bashed with eight or nine hits each — 97 hits in all — recording what the square became, what damage it kept, and whether the hit
touched anything. Each fixture's square below is pinned to `t_rock_floor`, because a floor's strength depends on whether the tile under it supports a roof and a dump that
left that to the world would be a recording of the seed.

| Fact | Count |
|---|---|
| Hits | 97 |
| Hits that succeeded | 11 |
| Hits that touched something | 86 |
| Rows that end as a roof | 6 |
| Rows whose square below is not the pinned `t_rock_floor` | 0 |

**Pass `--seed`.** The first recording of `hits.json` was made without one, and two rows came back reading `t_rock_floor_no_roof` where the fixture had written `t_rock_floor`: the
world under the fixture is mapgen, and without a seed it is a different world each run. The dump now records the square below **before** and after the hit as well, and the gate
asserts that the fixture held, so an unseeded recording fails instead of quietly changing the answer.

Regenerate with:

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-bash-info corpus/bash/info.json < /dev/null
    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-bash corpus/bash/hits.json < /dev/null
