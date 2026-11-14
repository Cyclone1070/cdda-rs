# Handoff

Last round: 44. State at hand-off: **CI green**, gate green, tree clean.

## Where the work stands

Alignment (phase 1) is done apart from terrain, furniture and traps, which are still per-square arrays
inside the level (`crates/core-types/src/submap.rs`). Everything else in the store: one `World`, one
derived index, systems in a declared schedule, no `Resource`.

The audit of round 41 is in `plan-audit.md` (untracked). Two of its items landed this round and the
turn loop's own costs were found and fixed on the way:

| Item | State |
|---|---|
| clippy/format in the default gate | done — one command, one exit status |
| parse memo process-wide | done — content suite 135 s → 73 s, sim suite 275 s → 232 s |
| turn-loop costs | done — butchery scenario 173 s → 1.6 s in release; the gate's playtest test 1,070 s → ~170 s |
| process-wide state ledger | done — nine entries, exact-set, shrink-only |
| CI parity with the build machine | done — the runner fetches the reference's data and links it where the binary looks |
| `#[expect]` + allow ledger (622 allows) | not started |
| `lib.rs` split (8,738 lines) | not started — deliberately, it is a large movement |
| `snapshot` borrowed view | not started — borrow-checker risk across 21 call sites |
| `clippy.toml` thresholds + ledgers | not started |

## Rounds 43 and 44: the thrust step, and the store driving it

`vehicle_move::thrust_velocity` is the speed a thrust step leaves, ported from `vehicle::thrust` (`vehicle_move.cpp:453-620`) and gated on thirteen recorded cases
(`corpus/vehicle_thrust.json`). Round 44 wired it: `vehicle_move::thrust(world, vehicle, direction)` reads acceleration, top speed, the reverse floor and the
battery question from the vehicle's own parts and writes `Vehicle::velocity`. `Vehicle` now carries `cruise_velocity`. Three more gates: the reverse floor against
the same thirteen rows, the battery-engine question on three vehicles, and a bicycle in the store reaching 1,650 from rest and being held at its 1,871 top speed.

## The next cycle

1. `map::move_vehicle` - the part that makes the vehicle go somewhere: velocity into squares per turn through the facing, with the reference's `of_turn` and the
   collision cases. This is the big one; dump first (`--dump-vehicle-move`), then the gate, then the port.
2. `vehicle::slowdown` (`vehicle.cpp`, the `vslowdown` in `gain_moves`) - the speed a vehicle loses per turn to rolling and air drag before thrust, which the
   movement rule runs before it moves anything.
3. Fuel burn per step (`consume_fuel`) and engine damage under strain - named gaps in `docs/parity.md`.

## What must stay true

- `python3 tools/remote_test.py` is the gate: formatting, lints, tests, docs, source ledgers, oracle
  patch. Read the exit status.
- `python3 tools/remote_test.py --clippy` alone re-lints; it touches every `.rs` first, because a test
  target has its own fingerprint and a stale one lies.
- Push, then poll CI until green. The runner now has the reference's data, so CI and the build machine
  check the same 2,853 tests.
- `plan-audit.md` and this file are untracked by design.
