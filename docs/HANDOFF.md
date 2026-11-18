# Handoff

Updated every round. Untracked by design; the record that ships is [parity.md](parity.md).

## Where the port stands

- Gate green: **1,596 sim / 547 content / 32 cli / 40 json**, clippy quiet, docs 3,690+ citations, oracle patch verifies byte for byte, CI green.
- The store is one `bevy_ecs::World`; the index is derived; no globals; no per-square arrays. Architecture gates in `sim_suite/architecture.rs`.
- Vehicle stores: `corpus/vehicles.json` (definition parts) is correct - `get_all_parts()` and the definition agree for all 249 prototypes.

## The current edge

- **Vehicles (P8).** Driving replay, two-hull impact along centres of mass, boarding (part + mass), the driver's exit skid, and a step taken from a vehicle are ported and gated.
- **Named gaps.** Vehicle spawn items (loot) → the mass difference in the impact test; carried weight behind `creature::Weight`; autodrive target; turrets; towing; z/ramps/rails.
- **No walk loop.** `creature::try_step` exists and `vehicles::step_from_vehicle` now calls it, but nothing turns a pressed direction into a step for a walking character. That is the
  next feature, not a detail: no walking session can be replayed without it.

## Next round

1. The walk loop: direction → destination → `map::move_cost` → `step_from_vehicle`, with a dump of a walking session as the gate.
2. Then vehicle spawn items, which closes the mass gap and lets the impact damage be asserted.
