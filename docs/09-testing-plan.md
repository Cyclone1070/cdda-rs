# 09 — Testing plan

Operational detail behind [04-validation.md](04-validation.md): what is tested, how
much of it, where it runs, what counts as a pass, and what happens when something
fails. Every number marked **target** is a goal to be measured, not a measurement.

---

## Where the gates run

The gates belong on the remote build machine, not on the developer's laptop. There are two modes, and the
difference between them is **what a change can have broken**:

    python3 tools/remote_test.py             # focused: only the packages the working tree changed
    python3 tools/remote_test.py --all       # everything: the suite, clippy over the workspace, docs, oracle
    python3 tools/remote_test.py --slow      # put back the modules that cost minutes (see below)

**The focused run is the default**, because it is the one typed dozens of times a round. It:

* syncs the code, tools and documents (about four seconds) and **not** the 300 MB of reference data and corpus,
  unless the far side has never had them or a changed path is inside them;
* maps the changed files to workspace packages (`crates/<dir>`), adds the packages that **depend** on a changed one,
  and runs the tests of those packages only;
* filters the tests further by **module name**: a change to `crates/sim/src/multiple_work.rs` runs `test(/multiple_work/)`,
  which is eleven cases instead of 1,900 - a filter that is a superset (it can run a few extra cases, never too few),
  and a change with no recognisable module (a manifest, a shared helper) falls back to the whole package;
* lints the changed packages only, and **without** touching every source file first: a focused run looks at code that
  just changed, so cargo re-checks it by itself;
* runs the document check only when a document or the reference changed, and the oracle patch check only when the
  oracle or the reference changed. Both are fixed costs that a code change cannot break: four and twelve seconds
  respectively, against a gate whose real work is a two-second test run.

**Measured on the build machine** (six cores, warm target directory): **9.5 s** when nothing changed, **23 s** for a
one-file edit - sync, rebuild, the matching tests, clippy of the package, formatting of the changed files.

**The full run is for before a push.** It syncs everything, runs the **whole workspace** with nextest, the whole
workspace's clippy (touching the sources first so cached crates still emit their diagnostics), the documents, the
oracle patch and formatting: **5 min 6 s**, 3,261 cases.

**Two tests were ten of the suite's eleven minutes.** `sim_suite::eoc_programs` holds two cases that take **462 s**
and **587 s** - each builds a whole world before running a program - while the other 1,864 cases finish in **94 s**
together. So `eoc_programs` is in `SLOW_MODULES`: the focused and full runs skip it and say so, `--slow` puts it back,
and **CI always runs it** (CI is the full suite, and its ten-to-twelve minutes are almost entirely these two cases).
A rule change *inside* the EOC code needs `--slow` before the push; everything else does not.

**Polling costs more than the work if it is slow.** A job used to be looked at every twenty seconds, so a two-second
test run took forty seconds of wall clock. The first look is now at a third of a second, doubling to two, which
turned a no-change run from **69 s to 9.5 s** without touching the job itself.

**The default run does not include clippy**, and a round that says "the full gate passed" without naming
`--clippy` has not run the lint gate. That is how the lint gate went red for several rounds while every
receipt said "clippy clean": the findings were there to read, but the gate was never named and the command
was piped, so nobody read the exit status. A piped gate is a report about the pipe.

**A dropped link is not a failing gate.** The sync each invocation begins with is retried - up to
`CVE_SYNC_ATTEMPTS` times (three by default) with `CVE_SYNC_RETRY_DELAY` between them - but only when the
failure is **transport**: an exit code of 255 (rsync's dropped connection) or 124 (an attempt the
`CVE_SYNC_TIMEOUT` gave up on). A compile error or a failing test returns its own code and is reported at
once, because retrying it would only make the gate slower at saying the same thing. The rsync is given its
own `-e` with keepalives for the same reason: a stalled transfer must fail and be retried, not hang.

**A long job is detached and polled, not streamed.** Every job used to be one ssh session whose stdout
was the job's, and when the far side went quiet the session died taking the result with it: the suite
had finished and passed, but the gate reported the session's `255`. Now each job is written to a remote
script, launched with `setsid nohup`, and left to write its own log and its own exit code; the gate
polls for the code with short calls that can each fail on their own without losing either. The job's
log is printed once it is over. **A result that depends on a session surviving the job is a result the
job does not own.**

That retry was earned, and so was the detachment. One round lost a full gate to an `rsync` that
returned 255 halfway through the copy - nothing checked, nothing wrong with the tree - and another lost
a test run whose remote half had already passed while the session carrying it stayed open. The machine
is reached over a tailnet and goes away for minutes at a time; both failures were the link, and neither
was the port. The retry is proved by fault injection: a shim
`rsync` that returns 255 for its first two calls recovers on the third and reports green, and the same
shim with `CVE_SYNC_ATTEMPTS=1` fails with `copy <path> failed after 1 attempt(s)`. **A gate that can
only be trusted when the network is perfect is not a gate.**

Local cargo is for diagnosing a single failure, never for the gate: a full debug build of the workspace
plus a test build is minutes of CPU on the machine someone is trying to use.

The C++ oracle build belongs there too. `oracle/build` is a ninja tree that wants the same treatment - an
interrupted build leaves compiler children behind, and a rebuild is a hundred translation units. Until the
oracle tree is moved to the server, an oracle change is the one job that has to run locally, and it runs as
a single managed job with its output collected, never as a foreground call that can be interrupted halfway.

## 1. What "the same" means

Absolute sameness is not attainable and is not the claim. The claim is:

> For every recorded session and every scripted screen scenario, the port produces
> the same observable result as the reference, or the difference is named, located
> and either fixed or declared.

"Observable" is defined by the equivalence spec in
[04-validation.md](04-validation.md): exact, tolerant with a stated epsilon, or
ignored. An observation not in that table is untested by definition, so the table
is the first artifact of every phase.

**Out of scope, deliberately:** frame timing, memory layout, thread scheduling,
audio output, feel, and anything never exercised by a session or a scenario.

---

## 2. Test tiers

| Tier | What it proves | Corpus | Cadence |
|---|---|---|---|
| T0 Determinism | The reference agrees with itself, and the port is reproducible run to run | Gate 0 sessions | Every change |
| T1 Unit | One system's rules, written test-first | Grows by one or two tests per cycle | Every change |
| T2 Reference corpus | Behaviour the original already asserts | Re-expressed reference tests | Every change |
| T3 Differential replay | Whole-session integration, per turn | Session corpus | Every change (sharded) |
| T4 Invariants | State that is corrupt but hashes identically | All sessions | Every change |
| T5 Fuzzing | Divergence outside the recorded corpus | Generated action streams | Nightly |
| T6 Visual, semantic | Screens and HUD, cell by cell | Screen scenarios | Every change |
| T7 Visual, pixels | Graphics rendering | Graphics scenarios | Nightly |
| T8 Long play | Save/load, overmap, drift over hours | Long sessions | Nightly |
| T9 Performance | Speed regression | Benchmarks | Every change, reported nightly |

Cheap tiers fail first, so a content bug never burns a replay shard.

---

## 3. T0 — Determinism

Before anything else, three checks run on the reference and then on the port
(detail in [04-validation.md](04-validation.md), Gate 0):

1. Same binary, same session, twice → identical hashes.
2. Same session under the sanitizer build and under a second memory allocator.
3. Same session under both target standard libraries.

Sessions that fail are quarantined with a reason, never adjusted. **Target: under
5% of the corpus quarantined**; anything higher means the port's ordering policy
needs revisiting.

---

## 4. T1/T2 — Tests written by us, and the reference's own tests

Two sources of test material, kept separate:

- **T1** — tests written before the code, red-green-refactor, one or two per cycle.
  They encode *our* reading of a system.
- **T2** — the reference's 1,218 test cases across 218 files, triaged into: re-express
  as a test, convert into a scripted session, convert into an invariant, or drop
  (with a reason). The triage table lives in `docs/parts/test-triage.md`.

The mechanical inventory behind that triage is
[13-test-inventory.md](13-test-inventory.md), regenerated from the reference by
`tools/test_inventory.py` so it can never drift; the rules for taking a case -
what to keep, what to rewrite, how to cite, how to triage a wart - are in
[14-test-porting.md](14-test-porting.md).

T2 is the higher-value source: it encodes the original's behaviour, including the
parts nobody would think to re-derive. The triage is done — all 218 files, 1,217
cases, 85,073 lines, classified one file at a time in
[parts/test-triage.md](parts/test-triage.md):

| Reuse | Files | What it means |
|---|---|---|
| RUST-TEST | 169 | Re-express directly as tests in the port |
| INVARIANT | 22 | Keep as property or range checks |
| SESSION | 7 | Only reproducible by driving the game with recorded input |
| DROP | 20 | Helpers, fixtures, suite meta-tests, tiles-only code |

Two consequences shape the plan. First, **sessions are for integration and
interface, not for basic behaviour** — 169 files of behaviour port as ordinary
tests, and only seven files genuinely need scripted play. Second, sixteen files
assert tuned numbers (damage tables, lethality brackets, speed to 0.01 mph, exact
JSON strings); each needs a written tolerance decision, listed in the triage
document, rather than a rewrite that quietly loosens it. The same document names
the player-visible behaviours with no test at all, which is what the session
corpus in §5 is weighted toward.

---

## 5. T3 — Session corpus

A session is a starting state, an RNG seed, and the player's actions with turn
stamps (format in [11-oracle-harness.md](11-oracle-harness.md)). The corpus is
committed, reviewed in diffs, and grows monotonically: a divergence found once
becomes a session forever.

The parse cache has its own rule, and it is the only one that is about time
rather than behaviour: a value read from the cache must be identical to a value
parsed from the source, on every file, member for member and bit for bit. That is
tested over the whole content set, so the cache cannot quietly become a second
parser.

Because reference saves are out of scope (decision recorded in
[03-decisions.md](03-decisions.md)), the starting state travels as a **state dump
produced by the reference and loaded by the port**, not as a save file.

Replay itself runs with test mode **off** and a fixed 80x24 UTF-8 terminal:
the input layer throws in test mode, so a replay in test mode could not read the
recorded keys at all. The harness therefore starts the game by the real path,
stops at the last point before input is required, and takes over the input layer
from there (see [parts/headless-start.md](parts/headless-start.md)). That makes
the dump an input format with its own loader tests, and it keeps the two games
sharing exactly one definition of "the world at turn zero".

### Categories and targets

| Category | What it must exercise | Target count |
|---|---|---|
| Movement and terrain | Walking, climbing, doors, vehicles as obstacles, z-levels | 40 |
| Environment | Fire, gas, smoke, radiation, light across a day and night, weather | 30 |
| Items | Pickup, drop, containers, pockets, wear, wield, charges | 40 |
| Crafting and construction | Recipe selection, tools, batches, failures, disassembly | 30 |
| Combat | Melee, ranged, reloading, wounds, martial arts, monster abilities | 60 |
| NPCs and dialogue | Conversation trees, missions, trading, followers, factions | 40 |
| Spells | Each cost kind, each shape, enchantments on items and creatures | 30 |
| Vehicles | Driving, collisions, boarding, repairs, turrets, towing | 30 |
| Save and load | Round trips at many points, including mid-activity | 25 |
| Long play | Multi-day sessions that touch everything above | 10 |
| Menus and screens | Navigation of every screen that changes state | 30 |
| Regression | One per divergence ever found | grows forever |

**Target: 400+ sessions**, each short (tens to hundreds of turns) except the long
ones.

### How sessions are produced

1. **Played by hand** for the paths that matter and are hard to fake (crafting
   chains, dialogue, vehicles).
2. **Scripted** from the action vocabulary for repetitive coverage (open every
   screen, walk every terrain, fire every weapon class).
3. **Fuzzed and minimised**: random valid actions, then shrunk to the shortest
   failing sequence and promoted into the corpus.

Every recording is proven complete by replaying it into the reference first; a
recording that stalls is discarded, not repaired.

### Coverage accounting

Coverage per session is reported, and gaps are chased by name rather than hoped
away. **Target: every system in [parity.md](parity.md) exercised by at least one
session before its phase gate**, and no shipped system below 70% line coverage in
its own phase's sessions.

---

## 6. T4 — Invariants

Checked after every turn in every session, in both builds. An invariant failure is
a bug even when both sides agree.

| # | Invariant |
|---|---|
| I1 | Every entity that exists is reachable from the position index exactly once |
| I2 | The position index never names an entity that no longer exists |
| I3 | No two solid entities occupy the same square unless the reference allows it |
| I4 | Item count is conserved by every transfer (no duplication, no loss) |
| I5 | Every id resolves, or is explicitly a null id |
| I6 | Health, needs, temperature and charges stay inside their declared ranges |
| I7 | Energy and action points are never negative beyond the declared floor |
| I8 | A dirty flag is set whenever the data behind it changed |
| I9 | Save, load, save produces the same state hash |
| I10 | Neither build ever writes outside its own world directory |
| I11 | The RNG stream advances only forward and always in the reference's order |
| I12 | No NaN or infinite value reaches a position, stat or timer |

---

## 7. T5 — Fuzzing

| Aspect | Plan |
|---|---|
| Alphabet | The 620 rebindable actions, filtered to those legal in the current context |
| Seeds | Fixed per run, recorded; nightly rotation over a committed seed list |
| Oracles | Crash, hang, divergence, invariant violation |
| Minimisation | Automatic shrink to the shortest failing action sequence |
| Promotion | Every minimised failure becomes a committed session and a permanent test |
| Budget | A fixed action count per night, so growth is deliberate |

The fuzzer is deliberately dumb: it presses valid actions and lets the game decide
what happens. Its job is to reach states nobody thought to record.

---

## 8. T6/T7 — Visual

**Semantic frames (primary).** For each screen scenario: open the screen from a
fixed save, press a fixed key sequence, dump every cell's character, foreground and
background, and compare. Zero tolerance — one wrong glyph is a failure. The
scenario list is derived from the screen inventory, one scenario per input context
at minimum (**target: 87+ scenarios**, one per input context, plus states that matter for large screens:
empty and full inventory, long item names, translated text, different terminal
sizes).

**Pixels (secondary).** Graphics build only, fixed tileset, fixed resolution,
dummy video driver, small per-pixel tolerance. Nightly, because it is noisy and
slow, and a loose pixel gate is worse than none.

Coverage notes: the frame gate also catches formatting, wrapping, colour and
layout differences, which is where most interface bugs will actually appear.

**Per platform** ([12-platforms.md](12-platforms.md)): the semantic frame gate must
produce identical logical frames on every target, because layout is behaviour. The
pixel gate keeps one baseline per platform, because glyph rasterisation legitimately
differs. Only macOS, Linux and Windows are targets for now, so this is three
baselines, not five.

Scenario classes that need more than a key sequence, from the screen inventory:
19 screens need mouse input, 3 need a prepared save (built vehicle, founded
camp, live NPC), 14 are debug-gated, and the map view needs a fixed seed, date
and weather state. Menus deliberately assert outside normal play: `uilist::init`
refuses to run in data-check mode ([uilist.cpp:466](../references/Cataclysm-DDA/src/uilist.cpp#L466)),
so the reference comparison runs as a normal session with injected input — the
same path the harness already uses for replay.

---

## 9. T8 — Long play and persistence

- Save, load, save round trips for the port's own format, at many points: in the
  middle of an activity, while driving, with a world partially generated.
- A dump loaded into the port, played, saved, reloaded and compared — proving the
  handover format and the port's own format agree about what the world is.
- Multi-day sessions to catch slow drift: rot, weather cycles, horde movement,
  NPC schedules, map memory growth.
- Reference saves are not read, by decision: the port refuses a 0.I save clearly
  rather than half-loading it.

---

## 10. T9 — Performance

| Benchmark | Measures | Budget |
|---|---|---|
| Turn time, late-game scene | Simulation throughput | ≤10% slower than the reference |
| Frame time, busy scene | Interface throughput | ≤10% slower |
| World load | Startup and save load | ≤10% slower |
| Save size | Storage growth | ≤10% larger |

Method: same machine, same data, warm cache, median of repeated runs, fixed seed,
no profiler attached. A regression beyond budget fails the change; the report is
attached to the run so the cause can be diffed later.

---

## 11. Where it runs

Job names below are the ones already defined in
[port-validation.yml](../.github/workflows/port-validation.yml).

| Job | Tiers | When | Budget |
|---|---|---|---|
| `oracle` | builds the instrumented reference | Every change | cached, minutes |
| `data-digest` | T2 (content), prototype digest (staged: ids, then fields per type) | Every change | seconds |
| `rust-checks` | build, lint, T1 | Every change | under 15 min |
| `conformance` | T2 (re-expressed reference tests) | Every change | under 15 min |
| `replay` (sharded) | T0, T3, T4 | Every change | under 30 min total |
| `invariants` | T4 | Every change | minutes |
| `visual-semantic` | T6 | Every change | under 10 min |
| `fuzz` | T5 | Nightly | under 2 hours |
| `visual-pixels` | T7 | Nightly | under 1 hour |
| `perf` | T9 | Every change, reported nightly | under 15 min |
| `report` | publishes evidence for the phase gate | Every run | seconds |

T8 (long play) has no job yet: it needs a corpus that does not exist until saves
load, and it is added in P1 alongside `replay`.

Sharding is by session, so a slow session cannot block the rest of the corpus.

---

## 12. When something fails

1. The hash chain names the first divergent turn.
2. System traces name the system.
3. The full state diff names the entity, component and field.
4. A session reproducing it is committed **before** the fix.
5. If the cause is reference order-sensitivity, the session is quarantined with a
   reason and a linked note — never silently skipped.

Step 4 is what makes the corpus grow toward real bugs. A fix without a new
permanent test is treated as unfinished work.

---

## 13. What this cannot prove

- **Unexercised behaviour.** Coverage names the gaps; it does not close them.
- **Feel and balance.** Only play tells you a fight is dull.
- **Undefined behaviour in the reference.** Where the original is undefined, the
  harness locks in whatever the compiler did, and that is recorded as a declared
  divergence.
- **Screens never scripted.** The scenario list is the boundary; anything outside
  it is unverified, and the inventory exists so that boundary is visible.

## A rebuild is a gate of its own (round 420)

The reference was rebuilt on this machine after the harness patches were re-applied. A rebuild is a chance to silently invalidate every corpus at once - a changed patch, a
changed default, a changed data path - so the three dumps the loaders are gated on were re-run from the new binary and compared byte for byte:

| Dump | Corpus | Result |
|---|---|---|
`--dump-fields` | `corpus/prototypes/fields.json` | identical |
`--dump-terrain` | `corpus/prototypes/terrain.json` | identical |
`--dump-items` | `corpus/prototypes/items.json` | identical |

**Run this after every rebuild**, before trusting any loader gate: `cmp` the fresh dump against the committed corpus. An identical file says the reference the loaders were gated
against is still the reference; a different one is a corpus update that has to be committed and a loader that has to be re-gated, and the two cases look exactly the same from a
green test run.


## Two inventories, not one (round 465)

`docs/13-test-inventory.md` lists the **reference's** test files - 215 of them, with their case counts and tags - and it is the coverage map the port is measured against. The
port's own counts come from `cargo test --workspace` and are reported per binary (content, json, core-types, sim). Conflating the two is easy and was done once in conversation: a
number from the C++ inventory is not a number of port tests, and a port test is not evidence about a C++ case unless it names the rule it transcribed.

