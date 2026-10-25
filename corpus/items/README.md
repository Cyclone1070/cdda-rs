# The item corpora

`measures.json` — `--dump-item-measures`: for each of the reference's 9,912 item types it constructs an item and asks
its own functions what the **rules** make of the data:

| field | reference |
|---|---|
`weight`, `integral_weight` | `item::weight`, `item::weight( integral )` |
`weight_mg`, `integral_weight_mg` | the same at milligram precision |
`volume`, `integral_volume`, `volume_no_contents` | `item::volume` and its variants |
`length` | `item::length` (`src/item.cpp:7343-7398`) |

`length` is there because the rule behind it is not the data: a liquid has none, a soft thing has none, an explicit
`longest_side` wins over any derivation, and otherwise it is the cube root of the **effective** volume — which for
anything counted by charges is the volume of one charge, so a battery's 100 ml became 1 ml and its length 10 mm, not
50. The port reproduces all of it, with two declared exceptions: guns (their length comes from their mods) and
`belt40mm`, whose recorded length is that of the 25 grenades the reference's default construction puts inside it.

`pockets.json` — `--dump-pockets`: every type's pockets with their volume, weight, watertightness, and the three
compatibility limits (`max_contains_length`, `min_contains_length`, `min_contains_volume`).

`pocket-session.json` — `--dump-pocket-session`: the same payloads offered to the same containers step by step.

Reproduce with `--dump-item-measures`, `--dump-pockets` and `--dump-pocket-session` (see the oracle README).
