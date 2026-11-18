# Handoff

Updated every round. Tracked since round 196; the long record that ships is [parity.md](parity.md).

## Where the port stands

- Gate green at `18a4d106`: **1,609 sim / 547 content / 32 cli / 40 json**, clippy quiet, docs 4,300 citations, oracle patch verifies byte for byte, CI 6m46s.
- The store is one `bevy_ecs::World`; the index is derived; no globals; no per-square arrays. Architecture gates in `sim_suite/architecture.rs`.
- Vehicle stores: `corpus/vehicles.json` (definition parts) is correct, and now carries each prototype's **spawn list** as well.

## The current edge: walking and the vehicles it happens on (P8)

Ported and gated in rounds 174-195:

- **Driving**: top speed, rolling drag, acceleration, the two-hull impact along centres of mass.
- **Boarding**: a step from a vehicle, the driver's exit skid, doors opened from the outside.
- **The price of a step**: `map::combined_movecost` → `Character::run_cost` — the limb blend `(footing + speed·2)/3`, the flat-ground multiplier, the diagonal's √2 round trip, and
  **stamina** (`2/(ratio+1)`, which is above one while any breath remains).
- **The tank**: a step burns `moves × 15/100 × stamina_move_cost_mod`; a turn returns `ceil(20 × turns)`, capped.
- **The body**: a part's score rebuilt from its type figure, health, encumbrance and threshold; the twelve parts sum to the two scores; the legs differ by load, not anatomy.
- **Vehicle mass**: parts + carried items (by the part's `cargo_weight_modifier`) + riders; a spawned car weighs 1,029,567 g with its loot, exactly the reference's figure.
- **Vehicle loot**: the spawn list is data; `place_spawn_items` finds the cargo part, rolls batches (`roll_remainder(chance × max(rate,1) / 100)`), halves a broken part's batch on one
  die in two, makes listed items at `rng_float(0,1) < rate`, draws groups **at the rate**, damages what a broken part lets through to its ceiling, and stows it.

## Named gaps, in the order they should be closed

1. **Turrets** — no turret brain, no mount logic; the part data carries `TURRET` but nothing aims.
2. **Towing** — no vehicle-to-vehicle link.
3. **z-movement, ramps and rails** — vehicles live on one z level.
4. **Perpendicular damage per part pair** — two hulls colliding at an angle damage along their centres of mass only.
5. **Pivot displacement** — `vehicle::pivot`'s mount shift while turning.
6. **Item damage has no consequence** — a marked rock is still a rock: no damage levels in rules, no repair, no wear.
7. **Carried weight and body parts** — `creature::Weight` is a seam; the walk price takes `LimbCost` instead of computing it from parts, because nothing carries anything yet.
8. **Enchantment cache** — the flat-ground multiplier arrives as an input rather than being gathered from traits.

## Next round

1. Refresh `docs/05-roadmap.md`'s P8 status with the list above, so the roadmap and this file agree.
2. **Turrets** are the largest remaining P8 hole and the only one whose data is fully loaded (`PartSpec` carries the mount, the gun is the part's `item`).
3. Failing that, **item damage levels** — `armour::damage_level` exists; what is missing is a consumer. The first honest one is repair, or the melee penalty for a ruined weapon.
