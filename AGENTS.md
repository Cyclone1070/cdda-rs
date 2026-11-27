# AGENTS.md — what this repo is, what to read, what not to break

A Rust port of *Cataclysm: Dark Days Ahead*, built on `bevy_ecs`. Baseline: tag `0.I-1`,
commit `7b2efa5cea38e4d4d97dd0e63b28b9148623da59`. Desktop only — macOS, Linux, Windows, one
codebase.

## The one rule

**The architecture is absolute. This port translates the reference's logic, never its structure.**

| Comes across | Does not come across |
|---|---|
| what a rule decides, and the numbers it uses | the classes, members and containers it decides them in |
| the data's meaning | per-square arrays, parallel containers, caches beside the truth |
| the order the game's own steps depend on | a turn function that calls twenty subsystems in code |

Read [docs/01-architecture.md](docs/01-architecture.md) before touching code. If a task seems to
need a second store, a global or a per-square array, the task is understood wrongly, not the
architecture.

## Docs

| Doc | What it settles |
|---|---|
| [01-architecture.md](docs/01-architecture.md) | the store, the index, components, systems, the gates |
| [02-turn-loop.md](docs/02-turn-loop.md) | the shape of a turn and where the systems sit |
| [03-decisions.md](docs/03-decisions.md) | every decision with its rationale, and what it rejects |
| [04-validation.md](docs/04-validation.md) | what "the same behaviour" means and how it is proven |
| [05-roadmap.md](docs/05-roadmap.md) | the phases, in order, with what each closes |
| [06-inventory.md](docs/06-inventory.md) | the code surface being ported |
| [07-entity-component-map.md](docs/07-entity-component-map.md) | what is an entity, a component, a prototype, a cache |
| [08-workflow.md](docs/08-workflow.md) | how work is organised day to day |
| [09-testing-plan.md](docs/09-testing-plan.md) | the gates a change must pass |
| [10-task-breakdown.md](docs/10-task-breakdown.md) | the task list behind the roadmap |
| [11-oracle-harness.md](docs/11-oracle-harness.md) | building the reference and dumping ground truth |
| [12-platforms.md](docs/12-platforms.md) | the desktop targets |
| [13-test-inventory.md](docs/13-test-inventory.md) | the reference's tests, file by file |
| [14-test-porting.md](docs/14-test-porting.md) | how a reference test becomes a port test |
| [15-overmap-terrain-ids.md](docs/15-overmap-terrain-ids.md) | how overmap terrain ids are made |
| [parity.md](docs/parity.md) | the running record of what is ported and what is not |

## Gates

All builds run on `cyc@elitedesk`; nothing heavy runs on this machine.

| Command | Checks | Cost |
|---|---|---|
| `python3 tools/remote_test.py` | **the edit loop**: the packages and modules the working tree changed, their clippy, formatting | 9.5 s if nothing changed, ~23 s for an edit |
| `python3 tools/remote_test.py --all` | the whole workspace's suite and clippy, docs, oracle patch, formatting | ~5 min |
| `python3 tools/remote_test.py --slow` | adds the modules that cost minutes (`eoc_programs`: 462 s and 587 s) | ~11 min |
| `python3 tools/remote_test.py --docs` | every doc link, anchor and `file:line` citation | 4 s |
| `python3 oracle/apply_harness_patches.py --verify` | the oracle's patches still apply byte for byte | 12 s |
| `./tools/oracle_sync.sh --dump-<name>` | rebuilds the reference and dumps new ground truth | minutes |

**The focused run is the default and the full run is for before a push.** CI runs everything, including the slow
modules, so nothing is skipped anywhere that matters - it is only skipped where it would cost ten minutes to answer
a question a one-file edit did not ask.

Green means: suite passes, clippy quiet, docs consistent, oracle patches verified.

## The cycle

Every cycle is the same shape, and it ends when CI is green — not when the commit exists.

1. **Red.** A failing test first, from an oracle dump or a transcribed upstream case
   ([14-test-porting.md](docs/14-test-porting.md)). In the reference's order: dump, gate, port.
2. **Green.** The smallest change that passes it.
3. **Refactor**, then **mutation-check**: break the rule on purpose, watch the focused test fail,
   restore. A test that cannot fail proves nothing.
4. **Gate**: `python3 tools/remote_test.py` for the edit loop (focused, ~20 s), `--all` before a push.
   Read the exit status; a piped gate is not a gate.
5. **Commit** with the receipt in the message: suite counts, clippy, docs citations, QA results.
6. **Push, then poll CI until green.** A red run is the next cycle's work; nothing advances on it.
7. **Update the record**: [parity.md](docs/parity.md), [13-test-inventory.md](docs/13-test-inventory.md)
   and `docs/HANDOFF.md` (untracked, refreshed at the end of every round).

**Prerequisite, currently missing:** this checkout has no git remote, so step 6 cannot run yet. Set
the remote and credentials before the first CI-polled cycle.

## What CI must run

`.github/workflows/foundation.yml`, on every push and pull request:

| Step | Command |
|---|---|
| formatting | `cargo fmt --all -- --check` |
| lints | `cargo clippy --workspace --all-targets -- -D warnings` |
| tests | `cargo test --workspace` |
| documents | `python3 tools/check_docs.py`, plus the generated inventory diff |

The runner fetches the reference's **data** at the pinned commit (shallow, cached by the pin) and links
it where the binary looks, because 149 tests read it and it is gitignored. A checkout that still lacks
it — a machine without network, a fresh clone — skips those tests with a line saying so rather than
panicking; the local gate on `cyc@elitedesk` always has it. A skip is a weaker gate than a pass.

`.github/workflows/port-validation.yml` holds the oracle replays, fuzzing and visual diffs; every job
is skipped until the repository variable `PORT_CI=on` is set.

The audit lints live in `plan-audit.md` and belong in `Cargo.toml`, so CI enforces them on every push
instead of relying on a remembered flag.

## Lints: where the rules actually live

- `clippy::all` and `clippy::pedantic` are `warn` workspace-wide and the gate runs `-D warnings`, so
  every pedantic lint that fires is an error.
- **`restriction` and `nursery` lints are off by default** — that is where most of the code-quality
  antipatterns hide (`as_conversions`, `wildcard_enum_match_arm`, `print_stdout`, `redundant_clone`).
  Enable them per name, never as a group: `restriction` wholesale is thousands of hits, most of them
  deliberate.
- Use `#[expect(lint, reason = "...")]` rather than `#[allow]`: an expectation that stops being
  needed fails `unfulfilled_lint_expectations` under `-D warnings`, so stale suppressions delete
  themselves.
- Some classes have no lint and never will — an authoritative global, a second source of truth,
  absence read as zero, clone cost, file size, TDD order. Those are enforced by the source ledgers in
  `architecture.rs`, by behaviour tests, or not at all; do not pretend a lint covers them.

## Working rules

- **Test first, one or two at a time.** Red, then the smallest green, then refactor, then the next
  test. A feature with no test that needs it does not get written.
- **The corpus is the red gate.** Behaviour is pinned against dumps from the reference, not against
  what the code looks like it should do.
- **Mutation-check every rule.** Break the rule deliberately and watch the test fail; a test that
  cannot fail proves nothing.
- **Counts prove non-vacuity.** A test that asserts on a corpus says how many cases it saw.
- **Architecture gates are tests too.** `crates/sim/tests/sim_suite/architecture.rs` builds a real
  world and checks the shape dynamically: the index matches the store in both directions, a drifted
  index is caught, a destroyed thing leaves no ghost, a thing is whatever components it carries, a
  query never sees what it did not ask for. Two source ledgers sit beside them: the global list may
  only shrink, and the simulation may hold no pointer or shared ownership.
- **Ask before any destructive git command.** `status`, `diff` and `log` need no permission;
  `commit`, `push`, `reset`, `rebase` and branch deletion do.
- **Never guess.** If a value is not in the source, the data or a dump, say so and get it.
