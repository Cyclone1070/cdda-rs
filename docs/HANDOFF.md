# Handoff

Last round: 43. State at hand-off: **CI green**, gate green, tree clean.

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

## This round: the thrust step

`vehicle::thrust` (`vehicle_move.cpp:453-620`) decides the next turn's speed, and it is ported and gated: `vehicle_move::thrust_velocity` plus
`corpus/vehicle_thrust.json`, thirteen recorded cases from a bicycle, a car and an electric car - at rest, rolling, at cruise, braking,
reversing, and one engine-off. Two mutations checked: no reverse penalty fails `bicycle_reversing`, no brake floor fails `bicycle_braking`.

## The next cycle

The thrust step is a pure function; nothing in the store calls it yet. Next, in order:

1. `thrust(world, vehicle, direction, cruise)` - read `current_acceleration`, the top speed and the reverse floor from the store, apply
   `thrust_velocity`, write `Vehicle.velocity`. Traction is `1.0` until `k_traction` is ported, and the doc says so.
2. `max_reverse_velocity` (`vehicle.cpp`) - the reverse floor. The corpus already carries it per case (`max_reverse`), so the gate starts from rows
   that exist.
3. `map::move_vehicle` - velocity into squares per turn through the facing: the part that makes the vehicle go somewhere.

## What must stay true

- `python3 tools/remote_test.py` is the gate: formatting, lints, tests, docs, source ledgers, oracle
  patch. Read the exit status.
- `python3 tools/remote_test.py --clippy` alone re-lints; it touches every `.rs` first, because a test
  target has its own fingerprint and a stale one lies.
- Push, then poll CI until green. The runner now has the reference's data, so CI and the build machine
  check the same 2,853 tests.
- `plan-audit.md` and this file are untracked by design.
