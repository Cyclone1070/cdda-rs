# Handoff

Updated every round. Untracked by design; the record that ships is [parity.md](parity.md).

## Where the port stands

- Gate green: **1,596 sim / 547 content / 32 cli / 40 json**, clippy quiet, docs 3,690+ citations, oracle patch verifies byte for byte, CI green.
- The store is one `bevy_ecs::World`; the index is derived; no globals; no per-square arrays. Architecture gates in `sim_suite/architecture.rs`.
- Vehicle stores: `corpus/vehicles.json` (definition parts) is correct - `get_all_parts()` and the definition agree for all 249 prototypes.

## The current edge

- **Vehicles (P8).** Driving replay, two-hull impact along centres of mass, boarding (part + mass), the driver's exit skid, and a step taken from a vehicle are ported and gated.
- **Named gaps.** Vehicle spawn items (loot) → the mass difference in the impact test; carried weight behind `creature::Weight`; autodrive target; turrets; towing; z/ramps/rails.
- **Walking exists now.** `sim::walk` turns a direction into a square, prices it (vehicle first, then terrain), and steps through `vehicles::step_from_vehicle`.
- **The price is a named gap.** The reference charges `Character::run_cost` (100 + carried weight); the port charges the terrain's own cost. `corpus/walk.json` cannot be committed
  until carried weight exists, because its `cost` column is the reference's encumbered answer (163 for a floor step).

## Next round

1. **The step price is complete for a bare survivor**: 100 × limb blend × flat ground. The flat-ground term is `FLEET`'s -0.15, and the port takes it as an input until an
   enchantment cache exists.
2. **Carried weight and the limb-score chain**: `run_cost_effects` (`character.cpp:10376+`) multiplies by `get_modifier(character_modifier_limb_run_cost_mod)`, which comes from
   `limb_score_movecost` and therefore from encumbrance. `limbs.rs` has the arithmetic; what is missing is the weight data behind it. That chain turns the walk dump's `cost` column
   into a comparable number.
2. Vehicle spawn items, which closes the mass gap and lets the impact damage be asserted.
