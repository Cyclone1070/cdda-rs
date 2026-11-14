# Audit plan — turning the 14 issues into gates

Temporary working note, not a project doc. Delete when the items are done and folded into
`docs/01-architecture.md`, `docs/09-testing-plan.md` and `docs/parity.md`.

Ground rule: an item counts as fixable only if a **generic** check catches the class, not the one
instance found today. "Already enforced by a lint" is not a plan — it is debt with a ratchet missing.

## The two that no test can catch

1. **#2 — "the test was written before the code."** Authorship order is not in the tree and not in
   git. Substitute: a red-green gate (apply only the test diff to the base commit, require red; apply
   everything, require green). That tests *coupling*, not order, and costs roughly double the gate.
   Buy it or accept TDD as a review rule.
2. **#11 — "a missing component must not read as zero."** Whether absence is legal is per-component
   semantics; `unwrap_or_default()` is correct on an optional map and a bug on a required component,
   same syntax. Enforceable form: a per-rule behaviour matrix (spawn without the component, assert the
   rule gives the reference's absent answer), not one global gate.

## The real gap under four of the items

The default gate, `python3 tools/remote_test.py`, runs tests + docs + ledgers + oracle and **never
runs clippy**; `--clippy` is a separate flag. `cast_possible_truncation`, `cast_precision_loss`,
`cast_sign_loss`, `cast_possible_wrap`, `struct_excessive_bools`, `too_many_arguments`,
`too_many_lines`, `ptr_arg` are already pedantic/complexity lints and already denied by `-D warnings`
— they are only as strong as the invocation. **First change: clippy into the default gate.** That is
configuration, and it turns items #7, #10, #12 and #14 from "found by hand" into "fails every round".

## What each remaining item becomes

| # | Issue | Gate |
|---|---|---|
| 1 | `lib.rs` 8,738 lines, 73 structs | crate root is only `mod`/`use`/doc; per-file item budget, exact-set, shrink-only |
| 3 | 19.8-min gate | fix the cause: `read_json` memo is `thread_local!` while its doc says "this process" (1,070 s of the 1,186 s is one test); add a corpus-parse counter and a per-binary CPU budget |
| 4 | `snapshot` clones 12 containers | borrow view instead of clone; `ItemNames` loses `Clone` or gains a test-only counter; allocation-scaling test (10 vs 10,000 kinds) |
| 5 / 13 | intern `Mutex`, `thread_local!` memo | ban `thread_local!` in `crates/*/src`; exact-set ledger of mutable statics (path + name, shrink-only); two-world purity test |
| 6 | 622 `#[allow]` with no reason | `#[expect]` instead of `#[allow]` (unused expectations self-delete under `-D warnings`); `allow_attributes_without_reason`; per-crate/per-lint count ledger, shrink-only |
| 7 | 505 cast allows | already denied; the plan is the #6 ledger — each remaining allow cites a reference line |
| 8 | 4 redundant clones | `redundant_clone` at warn, not deny (nursery, MIR, slow) |
| 9 | 235 wildcard arms | `wildcard_enum_match_arm` scoped to `crates/sim`; per-enum ledger, shrink-only |
| 10 | bool/arg/line allows | `clippy.toml` thresholds (`max-struct-bools`, `max-fn-params-bools = 1`, `too-many-arguments-threshold = 4`, `too-many-lines-threshold = 60`) + allow ledger |
| 12 | `&Vec`/`&String` | already enforced by `ptr_arg`; drop as a separate item |
| 14 | bool-blind params | `max-fn-params-bools = 1` in `clippy.toml` + a starting ledger of the 30 two-bool functions |

## Order

1. ~~clippy into the default gate (cheap, unblocks #6/#7/#10/#14).~~ **Done** (round 42): the gate runs
   `cargo fmt --check`, `cargo clippy -- -D warnings`, docs, the source ledgers, the oracle patch and the
   tests in one run; `--sync` no longer silently skips the checks. `clippy.toml` holds the thresholds and
   is synced with the code, because a gate that lints with the old rules is not the gate.
2. ~~`read_json` memo → process-wide, with a transparency test; parse counter (#3).~~ **Done** (round 42):
   the memo was a `thread_local!` behind a doc that said "this process", so every test thread parsed
   the 6,220-file tree again. It is one process-wide map behind a lock held only for the map operation,
   with a per-thread parse counter; two tests prove a second thread and a second call parse nothing,
   and both fail if the memo is disabled. The content suite fell from 135 s to 73 s and the sim suite
   from 275 s to 232 s. The four turn-loop costs found on the way are below.
3. `#[expect]` + per-lint allow ledger (#6), retiring the cast/bool debt behind it.: the
   gate's 19.8 min was **not** the memo. One scenario cost 898 s of the 1,070, and the cost was in the
   turn loop around the stages - a whole-world fingerprint twice per turn (160 ms against 90 us of
   stage work), the monster dump parsed **every turn**, the item tree read by two paths, and the
   schedule rebuilt every turn. All four are fixed, with a test each; the playtest test is now ~170 s
   in debug and the butchery scenario 1.6 s in release (was 173 s). The memo is still `thread_local!`
   and still worth making process-wide - the per-binaries remaining time is now content parsing, not
   the turn.
4. Crate-root shell + item budget (#1).
5. Mutable-static ledger + `thread_local!` ban + two-world purity (#5/#13).
6. `snapshot` → borrowed view + allocation-scaling test (#4).
7. `clippy.toml` thresholds + ledgers (#9/#10/#14).

## Corrections to the earlier audit (verified)

- The architecture gate scans **every** crate, but its only needle is `#[derive(Resource)]`, which
  nothing uses, and `GLOBALS` is empty — so `found.len() == GLOBALS.len()` passes on `0 == 0`. It is
  vacuous, not merely narrow. The pointer gate *is* filtered to `/sim/src/`.
- `snapshot` clones **12** containers, not 6.
- The 19.8-minute gate is one test: `every_scenario_in_the_repository_passes` at **1,070 s**.
- `read_json`'s doc promises "remembering the result for this process"; the code uses `thread_local!`,
  so every test thread re-parses the 6,220-file tree.
- `fn_params_excessive_bools` defaults to 3, so it catches nothing today; the 30 two-bool functions
  are invisible without `max-fn-params-bools = 1`.

## Lint enforcement, from the research

Of ~40 catalogued antipatterns, **30 have a clippy lint that catches the class**, and **17 of those run
today** with `clippy::all` + `clippy::pedantic` + `-D warnings`. The rest sit in `restriction` and
`nursery`, which are off by default — that is why they were all still in the code.

The residue has **no lint, for a good reason**: `Deref` as inheritance, `Box<dyn Error>`, an
authoritative global, a per-square second source of truth, `unwrap_or_default` turning absence into
zero, clone-instead-of-borrow cost, monolithic file length, TDD order, slow tests. Those are
semantics, authority, cost and process — a lint would be wrong or trivially bypassed. They get a
ledger, a purity test, a behaviour matrix or a measurement.

So the work is: **enable the lint per name**, use `#[expect]` instead of `#[allow]` so stale
suppressions delete themselves, and keep a shrink-only count per crate per lint. Not "turn on a
group": `restriction` wholesale is 2,290 hits, most deliberate.

## The CI gate to establish

`.github/workflows/foundation.yml` already runs, on every push and pull request:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 tools/check_docs.py`, and the generated inventory diff

`.github/workflows/port-validation.yml` (oracle replays, fuzzing, visuals) exists but every job is
skipped until the repository variable `PORT_CI=on` is set.

**Prerequisite, and it is missing:** this checkout has **no git remote** (`git remote -v` is empty), so
a push, and therefore a CI run, cannot happen yet. Set the remote and credentials before the first
CI-polled cycle.

Once the audit lints are in `Cargo.toml`, CI enforces them on every push instead of relying on a
remembered `--clippy`. That is the point of the exercise: the gate has to fail without me.

## Current process — to be continued

Where this side business stops:

1. **Round 1530 is in the working tree, uncommitted** — `vehicle_move` (vmiph/cmps conversions,
   `ground_acceleration`, `current_acceleration`), the `velocity` field on `Vehicle`, two gates, the
   acceleration corpus, docs and the oracle patch. Its gate passed (1,497 sim / 538 content tests),
   but QA was killed before it ran, so it still needs QA + commit + receipt. **Finish it first.**
2. Then the audit items above, in order, each test-first with its own receipt.
3. Then back to porting in `docs/05-roadmap.md` order — the next slice is the moving vehicle
   (`velocity`, thrust wiring, drag, `map::move_vehicle`) per `docs/HANDOFF.md`.
