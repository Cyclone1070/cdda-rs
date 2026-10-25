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

`pockets.json` — `--dump-pockets`: every pocket a constructed item reports, with its kind, volume, weight, watertightness, rigidity
the three compatibility limits (`max_length`, `min_length`, `min_volume`), the one-item rules (`holster`, `ablative`, `sealed`,
`max_item_volume`) and the three accept-lists (`items`, `flags`, `materials`, in the reference's own set order). The dump walks `get_all_contained_pockets`, which is
`get_pockets( is_type( CONTAINER ) )` (`src/item_contents.cpp:2142-2147`), so it records the **CONTAINER** pockets only — 1,146 types,
2,528 pockets — and a gun whose pockets are all magazine wells has none. Pockets contributed by installed mods are in there too, because
the reference records what a constructed item holds.

`pocket-session.json` — `--dump-pocket-session`: the same payloads offered to the same containers step by step.

`accepts.json` — `--dump-accepts`: twelve containers against every twenty-fifth item type (4,752 cases), with
`item_pocket::can_contain`'s answer and its own message for **each pocket of the container, in order**. The messages are the
point: `holster does not accept this item type or form factor` is the accept-list's, `item is too long`/`too short`/`too big`/
`too heavy`/`too small` are the limits', `item is not ammunition` is the ammo pocket's, and `can't contain liquid` is the phase
rule's — so one file gates five rules and says which one refused.

Reproduce with `--dump-item-measures`, `--dump-pockets` and `--dump-pocket-session` (see the oracle README).

### `damage.json` — every type's damage numbers

`--dump-damage` writes 9,912 entries: the category, whether the type is counted in charges, `damage_max`, `degrade_increments`, and the damage level the reference
reports at six probe values (0, 1, a quarter, a half, one short of the maximum, the maximum).

Reproduce:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-damage "$PWD/corpus/items/damage.json"

Two of the three rules are not data at all. The maximum is a constant (4,000, or 0 for anything counted in charges — 1,261 types). And `degrade_increments` is **0 for
all but 230 types**, because the loader throws away the stated `degradation_multiplier` unless the type is a vehicle part: the data says 1.0 everywhere and the rule
says no. The damage level multiplies before it divides, which is what makes its four bands equal — dividing first fails 25,953 of the 79,296 measurements.

### `faults.json` — every fault, whole

`--dump-faults` writes all 54 faults: name, description, fault type, item prefix and suffix, price modifier, degradation modifier, `affected_by_degradation`, both
encumbrance numbers, the blocked faults and both damage-modifier lists. Small enough that the gate compares **every field of every fault** rather than a sample.

Reproduce:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-faults "$PWD/corpus/items/faults.json"

Three rules the dump taught that the header file did not: a fault's `name` is a translation object and its `description` a plain string; `copy-from` inherits every
scalar the child omits; and `melee_damage_mod`/`armor_mod` are **appended** to the parent's list rather than replacing it — which is six of the 54 faults, and a
mutation of that one line fails three of them.

### `item-faults.json` — which faults each type can catch

`--dump-item-faults` writes the **resolved** weighted fault list for all 915 types that can acquire one. Resolved, not stated: a type may point at a `fault_group`,
and the expansion is a finalisation pass (`item_factory.cpp:919-936`) that either replaces a fault's weight with `( weight + weight_add ) * weight_mult` or adds it at
one `weight_override`. Replacing keeps the fault where it stood; adding appends — so the order of the type's own entries decides the result, and turning that replace
into an append fails 14 of the 915 types.

### `transforms.json` — what an item turns into

`--dump-transforms` writes every `transform` use action resolved: 459 of them over 459 types, 431 `transform` and 28 `delayed_transform` (the same loader with a
`not_ready_msg` on top). Keyed by the item type that declares it, so the gate can put a transform that landed on the wrong type on the failure line instead of hiding
it behind "found somewhere in the content".

### `consume-drugs.json` — what taking something does

`--dump-consume-drugs` writes all 108 `consume_drug` actions resolved: effects with durations and body parts, fields produced, stat adjustments, charges and tools
needed, vitamins as a range, damage over time, and the item left behind. Keyed by item type for the same reason the transforms are.

### `link-ups.json` — what a cable plugs into

`--dump-link-ups` writes all 108 `link_up` actions resolved. The defaults carry the behaviour: efficiency 0.85, move cost 5, and a target set that is
`{ no_link, vehicle_port }` rather than empty.

### `firestarters.json` — how long a light takes

`--dump-firestarters` writes all 42 `firestarter` actions. Its second field is derived rather than defaulted: `moves_slow` falls back to `moves * 10`, so
`"moves": 50` alone means a slow cost of 500, not the member's 1,000.
