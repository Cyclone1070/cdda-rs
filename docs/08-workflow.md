# 08 — Working agreement

How the port is built, reviewed and allowed to advance. The rules exist because
the failure mode of a project like this is not "it does not compile" — it is
"it seems to work and quietly differs".

---

## 1. Principles

| # | Rule | Consequence if broken |
|---|---|---|
| W1 | The reference is the specification, permanently | "Better than the original" becomes an untestable claim |
| W2 | Nothing is correct because a test was written; only because both builds agreed | Tests encode our assumptions, not the original's behaviour |
| W3 | Measure, never remember | Every number in the docs is reproducible by a script |
| W4 | Divergences are declared, never discovered | An undocumented difference is a bug even when it looks like an improvement |
| W5 | One gate at a time | A phase that is 90% done has 0% of its proof |

---

## 2. Where things live

```
docs/            this plan, kept current with the code
tools/           generators and checkers (no game logic)
oracle/          scripts + patches to instrument and build the reference
sessions/        recorded play sessions (small, committed, reviewed in diffs)
scenarios/       visual check scripts: which screen, which keys, which state
saves/           fixture worlds the sessions start from
crates/
  core-types/    coordinates, ids, units, calendar, rng, diagnostics
  data/          json reader, prototypes, inheritance, mods
  world/         entity store, index, environment
  items/         items, containers, crafting, interaction
  actors/        creatures, body, effects, combat, ai
  sim/           the schedule and every system registered in it
  persist/       save format, migrations
  ui/            screens, drawing, input
  harness/       replay runner, digests, comparison, reports
  xtask/         developer commands (gates, digests, corpus management)
```

The reference checkout stays pristine: instrumentation lives as patches under
`oracle/`, applied to a build directory, never to the pinned tree. That keeps
every citation in these documents valid.

---

## 3. Porting one system

Every system follows the same path. Steps 1–3 happen before any port code exists.

1. **Read the reference.** Name the entry points, the state it touches, the order
   it runs in, and everything it reads that is not its own.
2. **Write the equivalence rows.** For each field: exact, tolerant-with-epsilon,
   or ignored. Unlisted fields are not comparable, which means they are untested.
3. **Write the failing test first.** Red, then green, then refactor — one or two
   tests per cycle, never a batch of tests written after the code.
4. **Port the minimum that passes.** No features ahead of tests, no speculative
   generality.
5. **Run the gates in order** (see §4). Cheap gates first.
6. **Record the parity row** in [parity.md](parity.md): what is ported, which gate
   proves it, which divergences were declared.
7. **Delete the scaffolding.** Anything that existed only to get this system
   running is removed before the next system starts.

---

## 4. Phase gates

A phase is finished when its gate is green **and** the evidence is published —
not when the code compiles.

| Phase | Gate | Evidence artifact |
|---|---|---|
| P0 Foundation | RNG, text and JSON parity | Draw-for-draw comparison log; accept/reject corpus result |
| P1 Registry & saves | Prototype digest equal; oracle self-consistent | Digest file; Gate 0 report |
| P2 World store | Environment sessions replay equal | Per-turn hash chain for N sessions |
| P3 Items | Item/crafting sessions replay equal | Same, plus re-expressed reference tests |
| P4 Creatures | Combat sessions replay equal | Same |
| P5 Spells | Spell/enchantment sessions replay equal | Same |
| P6 Activities | Activity sessions replay equal | Same |
| P7 AI | Per-decision traces equal | Trace comparison for monster/NPC choices |
| P8 Vehicles | Vehicle sessions replay equal | Same |
| P9 World generation | Same seed, same world | Overmap/map digests over a seed corpus |
| P10 Interface | Zero differing cells | Frame comparison report per scenario |
| P11 Platform | Artifacts build; perf within budget | Release artifacts; benchmark report |

Advancing with a red gate is not allowed, and neither is widening a tolerance to
make a gate pass. Tolerances are changed by editing the equivalence spec, with the
reason written down, in its own reviewed change.

---

## 5. Definition of done (every change)

- Behaviour is proven against the reference, not asserted by us.
- A test exists that fails without the change.
- No new global state; nothing reaches across the world through a singleton.
- Iteration order is explicit wherever the result depends on it.
- Every string id, table entry and magic number it touches matches the reference.
- Divergences, if any, are listed in the equivalence spec with a reason.
- The docs that describe the touched area are updated in the same change.
- The performance gate has not moved outside budget.

---

## 6. Determinism rules (hard requirements)

- One RNG stream per session, seeded from the session file, drawn in the same
  order as the reference. No second generator, no per-system generators.
- No wall-clock reads inside the simulation; the date comes from the calendar.
- No iteration over an unordered container where the order can affect the result.
  If the reference iterates in an unspecified order, the port picks one canonical
  order and declares it.
- Identical seeds, options, terminal size and locale are part of every comparison.
- Redraws never mutate simulation state; the interface reads, never writes.

---

## 7. Review checklist

1. Does a test fail without this change?
2. Which equivalence rows does it touch, and are they still accurate?
3. Are all identifiers and constants taken from the reference rather than invented?
4. Is the iteration order deterministic and declared?
5. Is any state duplicated that should have one home?
6. Is any id replaced by a pointer, or a pointer by an id, without a reason?
7. Does any divergence appear that is not in the spec?
8. Is there a faster path being added that skips a system the reference runs?
9. Does the change make a future phase harder — and is that written down?
10. If this is wrong, which gate catches it, and how soon?

---

## 8. Decisions and blockers

Two kinds of question stop work, and both have a home:

- **Blockers** (B1–B8 in [05-roadmap.md](05-roadmap.md)) — things that must be
  decided or measured before a phase can be gated. Each is raised in
  [03-decisions.md](03-decisions.md) with the evidence, the options, and the cost
  of each option.
- **Open questions** — recorded with the phase that will answer them, and revisited
  at that phase's gate rather than mid-phase.

Nothing is settled by preference. If a claim is a judgment, it says so.

---

## 9. Continuous integration

| When | Jobs | Budget |
|---|---|---|
| Every change | Content digest, build, unit tests, invariants, lint, one replay shard | under 30 minutes |
| Every change (nightly budget) | Full replay corpus, semantic frame comparison | under 2 hours, sharded |
| Nightly | Fuzzing, pixel comparison, long sessions, coverage report, performance | under 4 hours |

The workflow skeleton already exists in
[port-validation.yml](../.github/workflows/port-validation.yml); it is gated off
until the port produces a binary.

---

## 10. Local loop

```bash
# regenerate the machine-checked parts of the plan
python3 tools/gen_inventory.py
python3 tools/check_docs.py            # links, citations, coverage

# build the instrumented reference (patches applied to a build dir, not the tree)
# Eight jobs by default: all cores makes every other shell command wait for minutes.
./oracle/build.sh

# record a session by playing, then prove the recording is complete
./oracle/cataclysm --record sessions/new.jsonl
./oracle/cataclysm --replay sessions/new.jsonl --dump-state /tmp/oracle.state

# run one session against both builds
cargo xtask replay sessions/new.jsonl --oracle ./oracle/cataclysm

# run the visual comparison for one screen scenario
cargo xtask frames scenarios/visual/inventory.json
```

---

## 11. Parity ledger

[parity.md](parity.md) is the single status table: one row per system, its phase,
the gate that proves it, and any declared divergence. It is updated in the same
change that ports the system, and it is the only place status lives — not in
chat, not in memory.
