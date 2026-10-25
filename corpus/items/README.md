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
`not_ready_msg` on top). Every member the loader reads is in the dump, including the two sentences the **actor** supplies when the data is silent, and the three
clamps it applies to numbers. Keyed by the item type that declares it, so the gate can put a transform that landed on the wrong type on the failure line instead of
hiding it behind "found somewhere in the content".

### `consume-drugs.json` — what taking something does

`--dump-consume-drugs` writes all 108 `consume_drug` actions resolved: effects with durations and body parts, fields produced, stat adjustments, charges and tools
needed, vitamins as a range, damage over time, and the item left behind. Keyed by item type for the same reason the transforms are.

### `link-ups.json` — what a cable plugs into

`--dump-link-ups` writes all 108 `link_up` actions resolved. The defaults carry the behaviour: efficiency 0.85, move cost 5, and a target set that is
`{ no_link, vehicle_port }` rather than empty.

### `firestarters.json` — how long a light takes

`--dump-firestarters` writes all 42 `firestarter` actions. Its second field is derived rather than defaulted: `moves_slow` falls back to `moves * 10`, so
`"moves": 50` alone means a slow cost of 500, not the member's 1,000.

### `holsters.json` — the two messages, and nothing else

`--dump-holsters` writes all 76 `holster` actions. The actor stores nothing — the pocket does — so its data is a prompt and a message; its behaviour is the one line
that says an active item cannot be holstered even when it fits.

### `deploy-furn.json` and `place-traps.json` — putting something into the world

`--dump-deploy-furn` writes 39 actors with one mandatory member. `--dump-place-traps` writes 28, and those have **two halves at two levels**: the unburied half
reads the actor's own object, the buried half reads a nested `"bury"` object — and only when there is a `bury_question`, so the same keys mean different things at
the two levels.

### `use-methods.json` and `ammo-scales.json` — what an item can do, and what it costs

`--dump-use-methods` writes every item type's action keys **in the map's own order** with the actor behind each, and `--dump-ammo-scales` writes the per-action charge
scale. Three rules are visible only here: the keys are **sorted** (they live in a `std::map`), a repeated name **collapses**, and a `repair_item` actor answers to its
`item_action_type`, so a repairkit has `repair_metal` rather than `repair_item`.

### `ammo-effects.json` and `ammo-effect-registry.json` — what a round does, and what an effect is

`--dump-ammo-effects` writes all 736 ammunition types' resolved effects; `--dump-ammo-effects-registry` writes the 82 `ammo_effect` definitions. The pair exists
because the reference **does not validate** an effect id at load — four ids in this content are really flags — so a gate has to record both sides rather than
assume the registry is a filter.

### `transform-applies.json` — what a transform *does*, not what it says

`--dump-transform-applies` builds each of the 459 items that carry a `transform`, hands it to the reference's own `do_transform`, and writes what it became: type,
charges, count-by-charges, active, countdown, item counter. The companion of `transforms.json`, which holds the actor's **data**; this one holds its **effect**, and
the two disagree in the ways that matter — `active` is a union of three conditions, and a liquid result switches the item on for a reason the actor never states.

### `transform-can-use.json` — whether the action is allowed, and in whose words

`--dump-transform-can-use` asks the reference's own `iuse_transform::can_use` about every transform in three standings (bare, wielded, wielded with no charges) and
writes the verdict plus the sentence it gave. The sentence is the useful half: the conditions are asked in a fixed order, so the answer says *which* one refused. It
carries the item's name formatted in, so a port comparing it has to compare the template around the name — and there are two placeholder spellings to handle, because
three sentences are `can_use`'s own (`%1$s`) and the fourth is the actor's `need_charges_msg` (`%s`).

## `comestibles.json` — every comestible slot, resolved

`--dump-comestibles` writes one entry per item type with a comestible slot (`src/itype.h:147-248`, 1,686 of them), with `calories` already multiplied by 1,000 and `nutr`
already derived:

```sh
tools/oracle_sync.sh --dump-comestibles '\$PWD/oracle/comestibles.json'
```

The build and the dump both run on `cyc@elitedesk` — `tools/oracle_sync.sh` rsyncs the already-patched source tree (the patch is proved to reproduce it by
`apply_harness_patches.py --verify`) and builds there. Nothing heavy runs on the Mac.

## `material-vitamins.json` — what each material carries

`--dump-material-vitamins` writes the nonzero vitamins of all 193 materials, read through `material_type::vitamin` (`src/material.h:163-166`). A comestible that states
no vitamins derives them from its materials (`src/item_factory.cpp:762-776`), so a disagreement about a food has to be traced to the material data before it can be
called a derivation bug — which is exactly what happened on the first run of the comestible gate.

## `comestibles.json` also carries `materials` and `contamination`

The comestible dump was extended twice: it now writes each type's resolved **material list with portions** (because a stated `primary_material` replaces it, and the
derived vitamins are computed from it), `contamination`, `freezing_point`, `primary_material`, `rot_spawn` and `consumption_eocs`.

## `fun-for.json` — what every comestible is worth to every eater

`--dump-fun-for` writes, for each of the 1,686 comestibles, the pair `Character::fun_for` returns in **22 eater states** (`src/consumption.cpp:483`): plain, sick
(cold and flu), cold and frozen items, rotten, rotten for a saprophage and a saprovore, a gourmand, the seven species thresholds, sick-and-gourmand, a faulty
gross-food bionic, and a taste blocker at 10 and 20 kJ.

Two fields of each row are inputs rather than answers, and both had to be recorded because neither can be recovered from the results:

* `item_fun` — `item::get_comestible_fun` (`src/item.cpp:8041-8056`), which adds each flag's `taste_mod` (`BAD_TASTE` is −5) and floors a `MUSHY` item at −5;
* `rot_set` — where `get_relative_rot()` actually landed after `set_relative_rot( 1.5 )`, which is **0** for every food that does not spoil.

## `food-summary.json` — what a bite puts in the stomach

`--dump-food-summary` writes, for all 1,686 comestibles, the four numbers `Character::consume_effects` computes before it hands the food to the stomach
(`src/consumption.cpp:1689-1714`): `kcal` after the body's own rules, the vitamins converted from RDA percentages, the water `quench · 5 mL` carries, and the room the
dry bulk takes up after its density and its calories per gram. `stomach::ingest` (`src/stomach.cpp:392-398`) adds all four unchanged, so this recording is the
stomach's state after one bite of an empty stomach.

Each row also carries the inputs the port has to reach from its own item model — `weight_g`, `type_weight_g`, `charges`, `volume_ml`, `count`, `quench`,
`relative_rot` — and the intermediate `ratio`, because a disagreement has to be placeable: on the type's data, on the item's arithmetic, or on the food rule itself.

## `digest.json` — the stomach emptying itself

`--dump-digest` puts a bite of each of the 1,686 comestibles into a 1,500 mL stomach and a 1,500 mL set of guts, then digests **six** half-hour steps of each at a fixed
metabolic pressure (`kcal` 2,500, `hunger` 1) and records, per step, what left (water, solids, calories in thousandths of a kilocalorie) and what the compartment still
holds (total volume, water, kilocalories). The vitamins that left in the first step and over all six are recorded too, so a rounding rule can be told from a clamping
one.

The ingested summary is recorded with its own water, solids, calories and vitamins, so the trace can be replayed without the food-summary recording — a disagreement is
then placeable: on what went in, on the rates, or on the release rule itself.

## `update-stomach.json` — the body's side of a meal

`--dump-update-stomach` runs the **real** `Character::update_stomach` (`src/character_body.cpp:935-1019`) on a fresh pair of compartments holding one bite of each of
the 1,686 comestibles, in six character profiles: at a 1,500 mL capacity and at the **250 mL a real character has**, fed and just bitten, fed and bitten forty
minutes ago, engorged and thirsty, and famished. Each profile records the metabolic rates the rule recomputed for itself, the calorie burn per turn, the healthy
weight's calorie figure, the two dice it rolled, and the body before and after — hunger, thirst, stored **calories** (not kilocalories: the burn is sub-kilocalorie and
the displayed figure rounds it away), vitamin levels, and both compartments.

The dice are the only randomness in the rule. The harness seeds the engine, rolls `roll_remainder( rates.hunger * five_mins )` and the thirst one, writes both down,
seeds the engine again to the same place, and calls the rule — which then rolls the same two numbers in the same order.
