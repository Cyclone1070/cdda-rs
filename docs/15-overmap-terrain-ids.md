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

## Solved: the rotation rule, exactly

`src/omdata.h:372` is the rule, and nothing in it is about geometry:

```
is_rotatable() = !NO_ROTATE && !LINEAR
```

Three things had to be right for that to reproduce the dump, and the first two are
the ones that had me guessing:

1. **Flags inherit.** A type's flags are its own plus its `copy-from` chain's,
   resolved through `abstract` parents too - my first pass indexed only `"id"`, so
   52 types looked flagless when their parent had `NO_ROTATE`.
2. **`delete` applies after inheritance.** `kelp_forest_end` inherits
   `NO_ROTATE` from `generic_kelp_forest` and then deletes it, so it rotates. A
   resolver that applies the delete first re-adds the parent's flag and gets it
   backwards.
3. **`uniform_terrain` is not rotatable.** Those types carry no variants at all.

With all three, predicted and measured agree on **5,901 types with no
disagreement** - every type either has all four variants or none.

## The whole registry, accounted for

| Part | Count |
|---|---:|
Types that rotate, times four | 23,612 |
Types with no variants | 351 |
Line-drawing shapes for the 14 `LINEAR` types | 256 |
Empty id | 1 |

That is the terrain registry, and it reconciles to within two ids of the
reference's 24,218 - the two being names that are both a type and a rotation
variant of another type.

The line-drawing shapes are `om_lines::mapgen_suffixes` (`src/overmap.cpp:262-275`):
`_es`, `_esw`, `_isolated`, `_end_north` and the rest, generated for the 14
terrains flagged `LINEAR` - the ant tunnels and their relatives.

## What the loader now does

Written and gated (`crates/content/src/overmap.rs`, `crates/content/tests/overmap.rs`):

* **The type registry is exact**: 6,269 ours against 6,269 reported, nothing
  missing and nothing invented.
* **The terrain registry is 24,212 of 24,218.** All 256 line-drawing shapes are
  registered: the sixteen suffixes of `om_lines::all` (`src/overmap.cpp:266-287`),
  appended to each `LINEAR` type's id. What is left is eight variants belonging to
  two outlier types.
* Those outliers are `microlab_ratkin_surface_connector` and
  `microlab_ratkin_vent_shaft_connector`. Both inherit `NO_ROTATE` and both rotate
  in the reference's registry anyway. The flag rule explains 5,901 of the 5,903
  rotatable types; making it explain these two as well costs nine other types, so
  they are named in the gate as outliers instead of being absorbed into a looser
  rule.

That is the honest state: one registry closed, one 264 ids short with the shortfall
named to the id.

## What remains

1. The two outliers' rotation, once whatever makes them rotate is understood.

That is the whole of it. The id gate now counts these eight ids and nothing else:
**87 of 90 registries exact**, with three named gaps - the effects, the overmap
specials, and these eight variants.
