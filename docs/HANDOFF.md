# Handover — after round 1529

Written by the agent that ran rounds 1505–1529. Everything below was measured in this workspace, not remembered. **Read this file first, then [`05-roadmap.md`](05-roadmap.md) and [`parity.md`](parity.md).**

---

## 1. The goal, verbatim

Pasted as it was given, rounds limit included:

> Finish the whole port of Cataclysm: Dark Days Ahead (baseline tag 0.I-1, commit 7b2efa5cea38e4d4d97dd0e63b28b9148623da59) as a desktop Rust game on bevy_ecs, end to end, with the architecture absolute: one store of entities and components, one derived index, systems in an ordered schedule, ids never pointers, and no reference structure imported - only its rules, numbers and data meaning.
>
> ORDER OF WORK, and the first step is not porting:
> 1. ALIGN WHAT IS ALREADY PORTED. Convert every global Resource into an entity with a marker component, turn ground/terrain/furniture/traps into entities, delete the per-square arrays and any second source of truth, and put the systems into real bevy schedules with declared order. The architecture gates (crates/sim/tests/sim_suite/architecture.rs) must end with zero globals and no per-square arrays outside the index, and every step is test-first.
> 2. BUILD THE VIBE-CODING AND QA SURFACE before resuming the port: a headless binary that can run the game (turn loop, no window), scripted headless playtests, game-state inspection (dump/inspect entities, components, a square, the index, a turn digest), native UI frame capture to PNG so the rendered screen can be diffed, and a QA runner that plays scenarios and compares against the oracle.
> 3. THEN PORT FORWARD in docs/05-roadmap.md order, every behaviour test-first per docs/14 (red gate from an oracle dump or transcribed upstream case, smallest green, refactor, receipt), keeping docs/parity.md, docs/13-test-inventory.md and docs/05-roadmap.md current.
>
> Structure must stay vibe-coding friendly: small files, one concern each, behaviour named in plain English, no hidden global state, and reading any module tells you what it does. Keep tools/remote_test.py gates green (tests, clippy, docs) on cyc@elitedesk before advancing, never run heavy builds on this machine, and keep the oracle ahead of every loader. Stop only for a real blocker.

**Goal state when this was written** (`get_goal`):

| Field | Value |
|---|---|
| id | `goal-6dabb85b-f734-4864-80b9-bab3bbf6caee` |
| revision | 5 |
| roundsStarted | 4 |
| maxGoalRounds | 10000 |
| phase | active, armed |
| `update_goal` policy | `edit`/`pause`/`resume` need a direct human request; `complete`/`blocked` are allowed only during an automatic continuation; `blocked` is rejected before the configured minimum round count |

**A human asking to continue in any wording means `update_goal` `resume` first** if the goal shows disarmed (after a resume or fork).

**Standing human corrections, still binding:**

- *"I don't give a flying fuck about rewrite cost."* Structure first; parity proven against the oracle.
- *"the docs must clearly state that the ecs arch is absolute... We should not adapt any cdda arch style into our code."*
- *"THERE IS NO CONTEXT EXHAUSTION. YOU HAVE AUTO COMPACT, CONTEXT IS LIMITLESS. REARM THE GOAL AND CONTINUE"* — never report context as a blocker.

---

## 2. State of the tree — **read this before touching anything**

| Item | Value |
|---|---|
| working tree | clean except this file, which is **untracked on purpose** (the handover describes the tree; it is not part of the port's history) |
| HEAD | `8a7abfcf` *A damaged engine makes less power* |
| last full receipt | suite **1,495 sim / 538 content / 29 cli**, clippy **0 findings**, docs **2,960 citations**, dead rules clean, oracle patch verified, QA **5 of 5** (149 corpus files) — measured on `8a7abfcf` |

**Nothing was in flight.** The tree is exactly the last commit.

**One history blemish to know about:** `717f8629` *A creature boards the seat under it* carries **two rounds** — round 1512 (the move cost of a vehicle square) and round 1513 (boarding) — because both touch `crates/sim/src/vehicles.rs` and were staged together; its message describes only boarding. The code is correct and its parity sections are in `docs/parity.md` ("A vehicle's square has a cost to walk through" and "A creature boards the seat under it"); only the commit message is incomplete. Fixing it means rewriting history, which needs the human's word — do not rebase on your own initiative.

### This session's commits, newest first

| Commit | What it landed |
|---|---|
| `8a7abfcf` | round 1529: an engine's damage scaling — `part_power`, parts dump v9, `--dump-engine-power` |
| `884f6de4` | round 1528: engine power — `total_power`, `max_ground_velocity`, parts dump v8, `enabled` default corrected |
| `3bece7b5` | round 1527: air drag — `vehicle_air::coeff_air_drag`, `VehiclePart::inside`, nine top-speed cases |
| `66ecd3b7` | round 1526: the top-speed cubic — `top_speed_vmiph`, `--dump-vehicle-top-speed` |
| `448c298a` | round 1525: rolling resistance — `vehicle_drive::rolling_drag`, parts dump v7, `--dump-vehicle-rolling` |
| `af090afa` | round 1524: the armour swap — `blamed_part`, `--dump-vehicle-armour` |
| `6efc588f` | round 1523, same round: the regenerated oracle patch (the dump's terrain cases) |
| `a6778b5f` | round 1523: terrain collision — `mapsquare.rs`, `Collision::Terrain`, the exception clauses |
| `74453214` | round 1522: collision detection — `first_collision`, `--dump-vehicle-collision`, the pivot and last-part rules |
| `cfc50308` | round 1521: the plain move — `move_vehicle` shifts the origin, re-indexes the squares and moves the riders |
| `d5ca6b4e` | round 1520: mass — `part_mass`/`mass`, a version-6 parts dump, `--dump-vehicle-mass` |
| `c8b16a6a` | round 1519: `remove_item` — cargo comes out, and a wreck can be salvaged |
| `cb1e95cd` | round 1518: cargo — `cargo_capacity`/`stored_volume`/`free_volume`/`stow_item`, items held in a part |
| `76aa7c9e` | round 1517: the engine switch, `auto_select_fuel`, `engine_fuel_left`, a version-5 parts dump |
| `226f2477` | round 1516: `fuel_left`, with a version-4 parts dump and tanks filled by the oracle |
| `29764cf6` | round 1515: the harness places a vehicle; the port's placement matches all 71 of its parts |
| `6c77dc19` | round 1514: a part's open state; an open door stops blocking its square |
| `717f8629` | rounds 1513 **and 1512**: boarding a seat; a vehicle square's move cost |
| `52309be6` | a stale "rotation is missing" sentence, corrected |
| `d49ca4e7` | round 1511: removal takes its dependents (`WINDOW`→`CURTAIN`, `SEAT`→`SEATBELT`, battery mounts) |
| `d0769e6e` | round 1510: a facing turns every mount - `tileray` transcribed, 385 oracle integers matched |
| `11df41ec` | round 1509: tow cable / cargo pair / mirror-opaque rules |
| `af1775b1` | round 1508: install and remove a part + the core mount rule |
| `fda878ac` | round 1507: a vehicle stands on every square its parts do |
| `b02cf68e` | round 1506: the lint gate repair (141 findings, two duplicate arms) |
| `593b8b30` | round 1505: a vehicle is one entity holding its parts |

---

## 3. Where the port stands

| Phase | State |
|---|---|
| P0 foundation | ported and gated (RNG draw-for-draw, JSON, units, coordinates) |
| P1 registry & saves | in progress: 160 registries load, prototypes dump v3, kind tables filled |
| P2 world grid | ported: terrain, furniture, fields, fire, light, scent, sounds |
| P3 items | in progress: kinds, weights, volumes, pockets, damage, faults, wear, crafting rules |
| P4 creatures & combat | in progress: melee, blocking, ranged dispersion, armour, effects, monster hearing/wandering, martial arts end to end |
| P5 spells | open: enchantment arithmetic ported; 26 of 32 spell effect kinds, 237 of 245 spells |
| P6 activities | started: `Doing`, time-based vs speed-based `do_turn`, `ACT_CRAFT`; the rest of the activity kinds are not written |
| P7 AI/dialogue | dialogue reading **finished** (692 of 692 programs read); NPC decisions and the behaviour tree are not |
| P8 vehicles | **open**: store, blueprint, damage, entity, occupancy, square index, install/remove, mount rule, facing, removal cascade, move cost, boarding, open doors |
| P9 overmap/worldgen | largely done: generation, specials, chunk rule at 291 of 576 squares with a trace instrument |
| P10 interface | not started; headless CLI, PNG frame capture and inspection surface exist |
| P11 shell | partially (headless binary, QA runner) |

### The dialogue census (finished — do not re-litigate)

| Measure | Number |
|---|---|
| programs defined / inline | 692 / 221 |
| programs that **read whole** | **692 of 692 — zero refusals** |
| run alone in a fresh world | 481 |
| run alone in a world with terrain, overmap and player | 486 |
| cannot run alone | 206, and only one of its three causes is a gap |

The 206 split as: **chains** whose variable an earlier program sets, **selection surfaces** with no chooser (P10), and **rules genuinely unwritten** (`u_teleport` 7, six legacy talk functions 6). A census that runs one program at a time cannot see a conversation between two of them — say that beside the number.

---

## 4. The current frontier: P8 vehicles

Done, in order:

1. **Store** (`crates/content/src/vehicles.rs`): `PartSpec`, `Prototype`, `Placement`; gate `content_suite::vehicles`.
2. **Damage rule** (`crates/sim/src/vehicle_damage.rs`): `hit(part, hp, amount, kind, no_resist) -> Outcome`.
3. **Entity** (`crates/sim/src/vehicles.rs`): `VehicleStore` singleton, `Vehicle { parts, rotation }`, `VehiclePart { part, x, y, hp, broken, open }`, `spawn`, `parts_at`, `damage`.
4. **Occupancy and the square index**: `part_squares`, `occupied_squares`, `vehicles::at`; `PositionIndex::rebuild` lists a vehicle at its origin **and** every part square.
5. **Install/remove** (`install_part`, `remove_part`) with the mount rule `can_mount`, and the dependent-part cascade.
6. **Facing** (`crates/sim/src/rotation.rs`): `coord_translate`/`TileRay`; the gate matches 385 oracle integers at 0/45/90/180/270 degrees.
7. **Move cost** (`move_cost_at`): 0 for an unbroken obstacle, 2 for an aisle, 8 open; an open door stops blocking.
8. **Boarding** (`Riding { vehicle, part }` on the creature, `board`, `unboard`, `rider_at`): one seat, one creature; a removed seat releases its rider.
9. **Doors** (`open_or_close`): `OPENABLE` only, per part, read by the move cost.
10. **Oracle placement** (`--dump-vehicle-placement`): the reference places the helicopter at the avatar's own square and the port reproduces all 71 mounts and squares; the dump carries each part's `fuel_type`, tank ammo and charges. The corpus file is `corpus/vehicle_placement.json`.
11. **Fuel** (`fuel_left`, `VehiclePart::fuel`/`charges`, `PartSpec::fuel_store`/`battery`/`fuel_type`): the parts dump is version 4, the placement dump fills 3 tanks with 300 gasoline and the battery with 100, and the port sums both branches. Named gaps: connected vehicles' cable loss, frozen liquid, muscle/perpetual fuel.
12. **Engines** (`is_engine_on`, `start_engine(world, vehicle, index, turn_on)`, `auto_select_fuel`, `fuel_current`, `engine_fuel_left`, `VehiclePart::enabled`/`fuel_preference`, `PartSpec::fuel_opts`): the parts dump is version 5 with `fuel_opts` in the reference's order; the turbine's seven fuels make the choice testable. Named gaps: the single-argument start's driver, immobiliser, starter/battery, fuel-pump and damage-roll checks with sounds and backfires.
13. **Cargo** (`crates/sim/src/vehicle_cargo.rs`: `InVehiclePart`, `items_in_part`, `cargo_capacity`, `stored_volume`, `free_volume`, `stow_item`, `remove_item`): items in a part are entities with the part's owner recorded as a component, the basket gate fills exactly, and a broken part still gives its contents back. Named gaps: gun-ammunition matching, stack merging, bucket spilling, the 4,096-item limit; the 10,000 L cap and CARGO-flag branch are unreachable in the data.
14. **Mass** (`crates/sim/src/vehicle_mass.rs`: `part_mass`, `mass`, `PartSpec::cargo_weight_modifier`): a part weighs its base item plus its cargo (scaled by the modifier), the whole vehicle sums the parts, and `--dump-vehicle-mass` pins the bicycle at 43,667 g with fifty rocks in the basket. Named gaps: tools, tied-down furniture, passenger/monster weight, `removed`/fake parts, enchantments.
15. **The plain move** (`move_vehicle`): an empty or longer-than-a-square displacement is refused untouched; a real one shifts the origin, replaces the vehicle's squares in the one index and puts every rider on its part's new square. Named gaps: ramps, map edge, pets, remote control, falling/floating, and every collision decision.
16. **Collision detection** (`first_collision`, `Collision`): every real `structure`/`ROTOR` part is projected forward in part order and the first square holding something answers - a vehicle first, then a creature, never the vehicle's own passenger, then terrain. The target part is the **last** part on the square, because the reference's square cache is written part by part. Named gaps: impassable fields, the `RAIL` clause, the `ARMOR` swap.
17. **Terrain in the way** (`mapsquare.rs`: `SquareData`, `move_cost`, `impassable`, `bashable`; `Collision::Terrain { bashable }`): a wall and underbrush are bashable, a floor and open air are not, and the exception clauses - `TINY` needs a wheel in the part's space, a `PROTRUSION` passes over `SHORT`, `NOCOLLIDE` never interacts - are pinned by a port-only gate. `impassable` is `move_cost == 0`, not a flag. `mapsquare.rs` is now the one place a square's terrain and furniture are read, shared with bashing.
18. **Armour takes the hit** (`blamed_part`): a creature or terrain collision names the plate on the part's mount - the part itself when it is armour, else the first unbroken plate - and a vehicle collision keeps the part that reached it. `--dump-vehicle-armour` pins the quad bike blaming `plating_steel` and then its `frame` once the plate is ruined; the self-check is unreachable from content and pinned by a fixture.
19. **What the wheels resist** (`vehicle_drive::rolling_drag`, `PartSpec::rolling_resistance`): `0.000225 * 9.81f * wheel_factor * mass_kg`, with the wheel sum over unbroken `WHEEL` parts scaled by `1.25 / (5 - 4 + n)` and 50 for no wheel at all. Both `accel_g` and the wheel resistance are **floats** widened inside the product; the parts dump is version 7 and prints nine significant digits so a float round-trips.
20. **Three coefficients become a top speed** (`vehicle_drive::top_speed_vmiph`): the reference's cubic `simple_cubic_solution` (`vehicle.cpp:4163-4201`) including its `std::complex` branch, `33.33` as the linear coefficient, and `mps_to_vmiph`'s float constant. `--dump-vehicle-top-speed` records six vehicles and one wheel-less, engine-less humvee that reaches the complex branch; a fixture with a non-zero complex answer is cross-checked against a bisection.
21. **The air is priced one column at a time** (`vehicle_air::coeff_air_drag`, `VehiclePart::inside`): the reference's column scan and its ideal-shape tally, `tile_to_width`, the 0.1 floor, and `inside` as a per-part flag that is false on a fresh part because `refresh_insides` is only reached through `enclosed_at`. Nine recorded vehicles match, including a rammed 4x4 and an aisle-carrying pickup. `pro` and `inside` are named as branches that cannot change an answer yet.
22. **The engines add up, and the vehicle drives itself** (`vehicle_drive::total_power`, `max_ground_velocity`, `PartSpec::power`): engines filtered by `is_engine_on`, summed, shared between more than one by `4 / (4 + count - 1)`, and the whole chain - the port's own air drag, rolling drag and power - reaches the recorded speeds on the three vehicles whose mass matches exactly. **`VehiclePart::enabled` now defaults to true**, the reference's own default, which the engine gate had wrong.
23. **A damaged engine makes less power** (`part_power`): `power * (dpf + (1 - dpf) * health)` in the reference's own unit, an integer of milliwatts that truncates before dividing by a thousand, with `1.0f - dpf` as the float subtraction the reference writes. `--dump-engine-power` walks the quad bike's engine through eight healths from whole to broken and all eight match.

**Next steps, in the order that unblocks the most:**

| Next | Rule to read | Why |
|---|---|---|
the moving vehicle | `vehicle::velocity`, `thrust`, `drag`, `map::move_vehicle`'s `can_move`/`veh.stop` | the state and the acceleration that change it; crash damage waits for this because `vehicle::collision` forces detect-only at zero speed. Thrust is the top-speed cubic's mirror: `F_prop = power / v`, drag is `c_air v^2 + c_rolling ( v + 33.33 )`, and both coefficients are already in the port |
`refresh_insides` | `vehicle.cpp:7525-7575` | needed before anything asks whether a passenger is inside - and it is the rule that would make the air scan's `exposed` term mean something for an enclosed cabin |
`refresh_insides` | `vehicle.cpp:7525-7575` | needed before anything asks whether a passenger is inside - and it is the rule that would make the air scan's `exposed` term mean something for an enclosed cabin |
velocity state | `vehicle::velocity`/`cruise_velocity`/`vertical_velocity`, `vehicle::thrust` (`vehicle_move.cpp:430-640`), `vehicle::drag` | the state a vehicle carries between turns, and the acceleration that changes it. **crash damage waits for this**: `vehicle::collision` forces detect-only at zero speed, so every damage number below `part_collision`'s detect return is unreachable until a vehicle has a speed |
what a crash does, then | `veh_handle_collision` (`map.cpp:830-870`), `part_collision`'s damage half (`vehicle_move.cpp:975-1200`), `vehicle_vehicle_collision` | damage to the part and the target, impulse, `total_mass` measured only after a collision is certain - mass finally bites here |
then | the rest of P8: turrets, tools, `removed`/fake parts, multisquare doors, `advance_precalc_mounts` | the list in `docs/05-roadmap.md` P8, item by item |
burning fuel | `vehicle::consume_fuel` (`vehicle.cpp:5226-...`), `fuel_usage()`, `drain_energy` | needs a load, a strain and a driver's stamina, so it belongs with movement, not before it |
items inside a part | `vehicle::add_item`, `part.items`, `vehicle_stack` | tanks, batteries, turret guns and cargo are items held in a part; fuel can be modelled with two fields, but cargo and turrets need the item entities themselves |
movement and collision | `vehicle::move`, `map::move_vehicle`, `advance_precalc_mounts` | the big one; pointer-order ties are blocker B1 in its purest form. Riders and cargo move with the vehicle, so those come first |
unboarding, tools, `removed` flag | `remove_part` (`:2213-2357`) | closes the remaining named gaps in the cascade round |

**Constraint that will bite:** the architecture gate forbids per-square arrays outside the index. A vehicle must be found through the one derived index, never by walking every vehicle each query. Rounds 1507 and 1510 are the pattern to extend: derive the squares (with facing), insert once, never keep a second list.

---

## 5. Working rules that are not negotiable here

**TDD:** red gate first, 1–2 tests per cycle, smallest green, refactor, receipt. No feature without a test that needs it.

**Mutation-check every rule:** break the rule deliberately, sync, run the focused test, watch it fail, restore. **`--sync` must come after the mutation** or the remote builds the old code. Mutate to a *valid wrong value* (`>= 1`, `false`, `Some("")`), never to something that fails to compile. **Never mutate by replacing a block with the empty string** — `str.count("")` is the string length, so the restore cannot find its anchor and the mutation is left in the tree (it happened in round 1513). Replace with a marker line (`    // mutation C`) and restore by that marker.

**Everything heavy runs on `cyc@elitedesk`, never on the Mac.** Run `tools/remote_test.py` **locally**; it syncs and then builds remotely. Running it *inside* an `ssh` command makes the remote ssh to itself and fail on publickey.

| Command | What it actually does |
|---|---|
| `python3 tools/remote_test.py --sync` | rsync the workspace to `~/cve-test` (every invocation syncs first anyway) |
| `python3 tools/remote_test.py` | tests, docs, dead rules and the oracle patch — **and NOT clippy** (~40 min: the cli binary alone is ~20) |
| `python3 tools/remote_test.py --tests` / `--clippy` / `--docs` / `--oracle` | one gate each; `--clippy` is ~10 s from a warm cache |
| `python3 tools/check_docs.py` | local, fast, every doc link and `file:line` |
| `cargo run -q -p cdda-cli -- qa` (on the remote) | the QA runner, **5 of 5** — corpus (142 files), scenarios, screens, oracle sessions, index |
| focused | `ssh cyc@elitedesk 'cd cve-test && export PATH=$HOME/.cargo/bin:$PATH && timeout 1200 cargo test -p cdda-sim --test sim_suite -- <filter>'` (build + seconds) |

**A round's receipt is all of:** `--tests`, `--clippy`, `--docs`, `--oracle`, and `qa`. The default run omits clippy; naming one command and calling it "the full gate" is how the lint gate stayed red for rounds (see §6).

**Clippy is `-D warnings` over every target, pedantic included.** Test files start with
`#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]`; anything else needs a site fix or an `#[allow]` with a reason. `indexing_slicing` bites on const tables too (`SX[q]`) — use `.get(q).copied().unwrap_or(..)`. Nested `if let` is collapsed to a let-chain (edition 2024) when clippy asks.

macOS has **no** `timeout`; use it only inside the ssh command line. Full gates belong in a background job with `job_output`, not in a 4-minute tool call.

**Git:** `status`/`diff`/`log` freely; **ask before** `reset`, `rebase`, `push`, `branch -D`. Commits were pre-authorised by the human; keep one round per commit with its receipt in the message.

---

## 6. Hazards that cost real time here

| Hazard | What happened | Do this instead |
|---|---|---|
| a gate that is not named is not run | rounds reported "clippy clean" while `--clippy` held **141 findings**; the default `remote_test.py` run does not include clippy, and the runs that did were piped so the exit status was never read | name each gate in the receipt, read the **exit status**, never a grep of a pipe; `--clippy` is cheap |
| a piped oracle build | `oracle_sync.sh … \| tail` reported exit 0 while the build had failed, and the dump file was 92 bytes of a half-written header | never pipe the oracle build; redirect to a log and check `$?`, then the log |
| blind `replace(..., 1)` edits | two duplicate `match` arms came from them (`"u_is_underwater"`, `Condition::GetCondition`), plus a doc comment on the wrong item; one anchor guess failed in round 1514 and left the parity file half-edited in memory (nothing written, but the assertion cost a cycle) | `assert s.count(anchor) == 1` before replacing, grep the result, build immediately; restore with `git show HEAD:<path>` |
| truncating `oracle/src/src/harness.cpp` | a replace whose end anchor was "to end of file" deleted every dump function after the one being edited — **and `oracle/src` is not tracked by git** | anchor the end at the next function name; if it happens, `rsync` from `references/Cataclysm-DDA`, re-apply the **committed** patch, then `--regen` and `--verify` |
| `docs/` truncation | `open(p,'w').write('')` wiped `13-test-inventory.md` | restore with `git show HEAD:docs/…`; prefer append-to-file over rewrite |
| a harness dump that needs a loaded map | the first rotation dump called `map::add_vehicle` after `start_headless_game` and returned null at every facing; two rebuilds were wasted | dump the **pure rule** (`tileray`/`coord_translate` over a grid) instead of a placed object where possible |
| census numbers | three rounds of confident wrong labels (a `switch` not descended, a depth limit read as "missing", an isolation read as "refusal") | **a census is an instrument**: when a number moves, ask what it measures before believing it |
| guessed numbers in tests | helicopter 17 parts (dump says 71), seat size 0 (dump says 25 L), turret parts 239 (dump says 241), frame armour (dump says 20) | when a test and the reference disagree, **the test is the suspect** |
| the slow cli test | `every_scenario_in_the_repository_passes` re-parses the 7.5 MB monster dump **each turn** (`cli::take_turn` → `monster_types`), so the gate spends ~20 minutes in one test | it is pre-existing and real; a cache would pay for itself, but do not start P8 by refactoring the harness |

| a vehicle dump that will not place | two oracle rebuilds were spent before dumping the refusal: the branch never called `check_mod_data`, so the prototype did not exist, and then `impassable(p)` refused because the avatar's start square is **furnished** | in a placement dump: call `g->check_mod_data( { mod_id( "dda" ) } )` before `start_headless_game`, lay `t_floor` **and clear furniture** over the box, and print `valid`, `inbounds`, `ter`, `furn`, `impassable` and `veh_at` on failure |
| `vpart_position` and `vpart_reference` | the first placement loop used `vpart_position` and could not reach `part()`; `veh->parts` is private | iterate `veh->get_all_parts()` as `const vpart_reference &`; it has `mount_pos()`, `pos_bub(here)`, `part()` and `info()` |

| a dump version bump | `corpus/vehicles.json` went to version 4 with the fuel fields, and the parser is strict, so the store cannot load the old file at all - every vehicle test fails until the new dump is copied in | regenerate the dump and copy it in the **same** step as the parser change; expect the focused vehicle tests to be red on "a part has no fuel_type" until then |
| `is_fuel_store` without a placed part | the parts dump has no vehicle instance to ask, and `is_tank()`/`is_magazine()` are questions about the base item | build one `item base_probe( part.base_item )` in the dump loop and ask it; the reactor case is a part flag |

| a dump that is not JSON | adding `fuel_opts` to the placement dump appended it **after** the closing brace of each part, and `python3 -c 'json.load'` caught it only after the rebuild | parse the new dump locally (`python3 -c 'import json;json.load(open(...))'`) **before** scp; a malformed corpus file is not a gate |

**Oracle workflow when a loader needs data the dump does not carry:** edit `oracle/src/src/harness.cpp` (+ `harness.h`, `main.cpp`) → `apply_harness_patches.py --regen` → `--verify` (must print *byte for byte*) → `./tools/oracle_sync.sh --dump-<name> /home/cyc/cve-test/dumps/<file>` (rebuilds, ~12–15 min, **no pipe**) → `scp cyc@elitedesk:~/cve-test/dumps/<file> corpus/<name>.json` → **then** write the loader/test. The oracle stays ahead of the loader; that is the only order in which a loader can be wrong out loud. A dump that only needs `units`/`tileray` needs no world load at all.

---

## 7. Map of the code

| Path | What lives there |
|---|---|
| `crates/content/src/` | data readers: `dialogue.rs` (the EOC/dialogue reader, ~4.5k lines), `vehicles.rs`, `items.rs`, `mapgen.rs`, `factory.rs` (shared copy-from machinery) |
| `crates/sim/src/vehicles.rs` | the vehicle entity, `VehicleStore`, the mount rule, install/remove + cascade, occupancy, `at`, `move_cost_at`, `Riding`/`board`/`unboard`, `open_or_close`, `fuel_left`, `fuel_current`/`engine_fuel_left`, `is_engine_on`/`auto_select_fuel`/`start_engine` |
| `crates/sim/src/vehicle_cargo.rs` | what a part holds: `InVehiclePart`, `items_in_part`, `cargo_capacity`/`stored_volume`/`free_volume`, `stow_item`, `remove_item` |
| `crates/sim/src/vehicle_mass.rs` | what a vehicle weighs: `part_mass`, `mass` |
| `crates/sim/src/vehicles.rs` (movement) | `move_vehicle`, which shifts the origin, re-indexes the squares and moves the riders |
| `crates/sim/src/vehicles.rs` (collision) | `Collision`, `first_collision`, `creature_at`, `terrain_collision` |
| `crates/sim/src/mapsquare.rs` | what the data says about a square: `SquareData`, `move_cost`, `impassable`, `bashable`, `has_flag`, and the `MapDataTable` bashing shares |
| `crates/sim/src/rotation.rs` | `tileray` and `coord_translate` |
| `crates/sim/src/vehicle_damage.rs` | the part damage rule |
| `crates/sim/src/routing.rs` | `square_cost` (terrain only today) and the path scorer |
| `crates/sim/tests/sim_suite/vehicles.rs` | all P8 gates and their helpers (`store`, `world_with_the_helicopter`, `part_index`, `creature_at`, `recorded_facings`) |
| `corpus/` | every reference dump the port is measured against; `vehicle_rotation.json` is the facing grid, `vehicle_placement.json` the placed hull |
| `oracle/` | the instrumented reference: `src` (disposable, untracked), `patches/0001-prototype-digest.patch` (**tracked — the only authority for what the oracle adds**) |

**Architecture in one line:** one store of entities and components; no `Resource`s (a singleton is a data component plus a `The*` marker, reached by `singletons::singleton::<T, M>(world)` and read-only by `singletons::holder::<M>(&world)`); ids are data, never pointers; systems run in declared order.

---

## 8. What "done" looks like from here

The honest remaining work, largest first: **P6 activity kinds**, **P8 vehicles** (items in parts, then movement and collision), **P10 interface**, **P5 spell effects** (6 kinds, 8 spells), **P7 NPC decisions**, **P9's last 285 map squares**, and the run-time dialogue gaps that are real rules (`u_teleport`'s chain, six legacy talk functions, the two selection menus).

Do not chase the 206 dialogue refusals: most are the census's isolation, and the number is a floor on capability, not a deficit.
