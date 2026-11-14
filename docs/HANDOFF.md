# Handoff

Last round: 46. State at hand-off: **CI green**, gate green, tree clean.

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

## Rounds 43 to 46: the vehicle moves

- **43**: `vehicle_move::thrust_velocity` - the speed a thrust step leaves, from `vehicle::thrust` (`vehicle_move.cpp:453-620`), gated on thirteen recorded cases.
- **44**: `vehicle_move::thrust(world, vehicle, direction)` wired it to the store, with `Vehicle::cruise_velocity`; `max_reverse_velocity` and the battery-engine
  question ported.
- **45**: `vehicle_move::slowdown_of` and `coast` - `vehicle::slowdown` (`vehicle_move.cpp:112-153`) and the block in `gain_moves`. 56 recorded rows, both engine
  states.
- **46**: `vehicle_move::move_step` and `turn_cost` - `vehicle::act_on_map` (`vehicle_move.cpp:1829-2010`): a square costs `400 / |velocity|` of a turn, a square
  costing exactly the turn left is not affordable, below 20 vmiph the vehicle stops, and the leftover fraction is carried. 120 recorded steps replayed exactly.
  `Vehicle` carries `of_turn` and `of_turn_carry`; the displacement goes through `vehicles::move_vehicle`, so the index and the riders follow.

## The next cycle

1. Collisions: `vehicle::collision` (`vehicle_move.cpp`), the cases it can report and the damage each does. This is what stops a vehicle driving through a wall, and
   it is the biggest remaining piece of movement. Dump first: a vehicle driven into terrain, another vehicle and a creature, recording where it stops, the speed
   after, and the damage to the parts.
2. `vehicle::strain` and `safe_velocity`, which the engine-damage branch of `thrust` reads.
3. Fuel burn per step (`consume_fuel`) and plows (the rest of `static_drag`).

## What must stay true

- `python3 tools/remote_test.py` is the gate: formatting, lints, tests, docs, source ledgers, oracle
  patch. Read the exit status.
- `python3 tools/remote_test.py --clippy` alone re-lints; it touches every `.rs` first, because a test
  target has its own fingerprint and a stale one lies.
- Push, then poll CI until green. The runner now has the reference's data, so CI and the build machine
  check the same 2,853 tests.
- `plan-audit.md` and this file are untracked by design.
