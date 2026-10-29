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

| Command | Checks |
|---|---|
| `python3 tools/remote_test.py --tests` | the whole suite on the build machine |
| `python3 tools/remote_test.py --clippy` | lints |
| `python3 tools/remote_test.py --docs` | every doc link, anchor and `file:line` citation |
| `python3 oracle/apply_harness_patches.py --verify` | the oracle's patches still apply byte for byte |
| `./tools/oracle_sync.sh --dump-<name>` | rebuilds the reference and dumps new ground truth |

Green means: suite passes, clippy quiet, docs consistent, oracle patches verified.

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
