# 15 — How overmap terrain ids are made

Working note for the last P1 gap. Everything here was measured against the pinned
reference and `corpus/prototypes/ids.json`, not read off a diagram.

## The two registries are different things

| Registry | Size | What it is |
|---|---:|---|
`overmap terrain type` | 6,269 | one per distinct `"id"` in a `type: overmap_terrain` entry |
`overmap terrain` | 24,218 | the *variants* a type spawns: rotations, line-drawing shapes, and the empty id |

## Verified: the type registry is just the JSON ids

Scanning `data/core` + `data/json` for `type: overmap_terrain` and collecting every
string in `"id"` (a string or an array of them) yields **6,269 distinct ids** -
exactly the reference's count, with no additions and no omissions. That registry
needs no rule beyond "read the ids", which is why it is worth closing first.

The earlier 7,788 figure was the same scan over *all* data including mods, which
the dump does not load. The load set matters more than the traversal here.

## Measured: the terrain registry is types plus variants

Of the 6,269 types:

* **5,903** have all four `_north`, `_east`, `_south`, `_west` variants.
* **365** have none at all - never one, two or three, which rules out a
  per-direction decision and points at a per-type one.
* **256** further ids in the terrain registry are neither a type nor a type plus a
  direction: `acid_ants_end_east`, `acid_ants_es`, `acid_ants_isolated` and the
  like. These are the line-drawing shapes `om_lines::mapgen_suffixes` generates for
  terrains carrying `LINEAR` (`src/overmap.cpp:943-960`), not rotations.

Check: 5,903 × 4 + 365 + 256 + 1 (the empty id) = 24,234 against the reference's
24,218, so 16 of those shapes overlap counts that are already included - close
enough to say the shape of the rule is right, and specific enough that the id gate
will name the remainder rather than hide it.

## Open: which 202 types refuse to rotate

`NO_ROTATE` covers 149 of the non-rotating types and `LINEAR` another 14, which
leaves **202** with no flag at all: `basement`, `crater_core_large`,
`faction_base_blacksmith_0` and friends. Something else decides for them. The
candidates are inheritance (`copy-from` reaching a `NO_ROTATE` parent) and a rule
in `oter_type_t` itself; that is the next thing to read, and it is a small enough
question to answer exactly rather than approximate.

This is the state of the last P1 blocker: one registry solved by counting, one
registry with its shape known and 202 types still to explain.
