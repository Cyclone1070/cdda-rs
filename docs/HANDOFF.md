# Handoff

Last round: 48. State at hand-off: **CI green**, gate green, tree clean.

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

## Rounds 43 to 47: the vehicle moves

- **43-46**: thrust (the speed a step leaves), the store-level thrust, drag and coasting, and the movement step itself - a square costs `400 / |velocity|` of a turn,
  a square costing exactly the turn left is not affordable, below 20 vmiph the vehicle stops, the remainder is carried.
- **47**: `move_step` asks `vehicles::first_collision` before it takes a square and stops on terrain (`map.cpp:948-950`), so a vehicle no longer drives through a
  wall. Gated by driving a bicycle into a wall in a real world.

**The gate bug worth remembering:** `remote_test.py`'s "touch every source file" step ran on this machine while clippy ran on the build machine, so cargo
re-reported a cached clean result - three rounds in a row reached CI with a lint the gate had called clean (`float_cmp` twice, `collapsible_if` once). The touch
now runs where the lint runs, proved with a deliberate probe.

## The next cycle

1. Collisions that do something: the crash dump exists (`corpus/vehicle_crash.json`, eight rows) but still lacks the four numbers the split needs - `mass2`, `e`,
   `part_dens` and `k`, all derived from bashable terrain. Extend `dump_vehicle_crash` to write them, rebuild, then port `part_collision`'s impulse loop
   (`vehicle_move.cpp:1030-1165`) against the completed rows.
2. `vehicle::strain` / `safe_velocity` and the engine damage they feed.
3. Fuel burn per step (`consume_fuel`).

## What must stay true

- `python3 tools/remote_test.py` is the gate: formatting, lints, tests, docs, source ledgers, oracle
  patch. Read the exit status.
- `python3 tools/remote_test.py --clippy` alone re-lints; it touches every `.rs` first, because a test
  target has its own fingerprint and a stale one lies.
- Push, then poll CI until green. The runner now has the reference's data, so CI and the build machine
  check the same 2,853 tests.
- `plan-audit.md` and this file are untracked by design.
