# 14 — Turning the reference's tests into the port's tests

The reference has 1,217 test cases in 206 files ([13-test-inventory.md](13-test-inventory.md)).
They are the best description of intended behaviour that exists, and they are not an
equivalence oracle. This document says what they are good for, how a case is taken,
and what a test in this port must look like.

## Why they cannot be the gate on their own

| Property | Consequence |
|---|---|
They assert **behaviour through C++ APIs**, not serialised output | there is nothing to diff; a port consumes the *assertion*, not the test |
They seed from the **clock** (`--rng-seed time`) and draw random inputs in 27 files | a failure upstream is often unreproducible, so a copied number is not a fixture |
They reach for **global game state** (758 of 1,217 cases) | they cannot run before that subsystem exists |
Some of them **pin reference warts** | reproducing them blindly would port a bug on purpose (see [03-decisions.md](03-decisions.md)) |

So the port uses two kinds of test, and neither replaces the other:

* **Corpus gates** (`corpus/`, produced by the instrumented reference): prove the
  *data* came out identical — "this item weighs 305 g".
* **Ported cases**: prove the *arithmetic* is identical — "this formula turns these
  inputs into 305 g".

A wrong formula that happens to agree on the sampled data passes the first and
fails the second. A wrong table passes the second and fails the first.

## The order that has worked

1. **Dump first.** Add the oracle mode, run it, commit the corpus. Nothing is
   asserted about a system the reference has not been made to report.
2. **Write the failing corpus gate.** It prints the differences it found, by field.
3. **Port the smallest thing that removes them** — one field, one rule at a time.
4. **Cite the line** the rule came from in the code comment, not in the test.
5. **Tighten the gate.** When a difference is closed, turn the note that recorded
   it into an assertion, so it cannot come back silently.

## How to take a case from the reference

A ported case is written like this, and the shape is not negotiable:

```
//! What this file pins, and which reference file and lines it comes from.
//!
//! Reference: `tests/line_test.cpp` (`line_to`), `src/line.cpp:40-120`.

#[test]
fn a_line_from_a_corner_includes_the_far_end_and_not_the_near_one() {
    // The reference's own assertion, in its own words, so the intent survives.
    ...
}
```

Rules:

* **Port the assertion, rewrite the harness.** Their scaffolding is C++ globals;
  ours passes state explicitly. The numbers and the edge cases are what we keep.
* **Fix the seed.** Where upstream generates random inputs, pin the seed and the
  count. A ported case must fail for exactly one reason.
* **Keep their edge cases.** The interesting ones are the off-by-one boundaries:
  the negative coordinate that floors instead of truncating, the zero-chance roll,
  the empty container that must not advance the stream. Those are the cases that
  find port bugs.
* **One behaviour per test, named as a sentence.** `a_remainder_is_measured_from_the_cell_it_falls_it`
  tells a reader what broke; `test_projection_2` does not.
* **Cite `file:line` on every rule.** If the comment cannot name a line, the rule
  was invented and the test is worthless.
* **Triage warts.** A case that pins a reference bug goes into
  [03-decisions.md](03-decisions.md) as a declared divergence, with the reason,
  and the port asserts *its own* behaviour instead.
* **No tolerance without a number.** Approximations are allowed only with a
  measured bound and a recorded reason (see `normal_roll_chance`, 4e-8).

## The TDD cycle for porting work

Red-green-refactor, one or two tests per cycle, and the corpus gate is the red:

1. **Red:** write the case (or extend the corpus gate) so it fails for the reason
   the port is missing. Run it and read the message — if the message does not tell
   you what to fix, fix the message first.
2. **Green:** the smallest change that passes. No field, branch or loader beyond
   what the failing test needs.
3. **Refactor:** remove the duplication the change exposed, rename to the
   reference's vocabulary, delete the note the test replaced. Then run the gates.

Never batch tests. A cycle that adds six tests at once hides which change made
which test pass, and the whole value of the order is knowing that.

## What a good failure message says

Every gate must answer three questions without a debugger:

| Question | Example |
|---|---|
What differed | `mon_zombie: speed 70 != 65` |
How much of it | `37 mismatches: 30 materials, 6 flags, 1 name` |
How to look further | `first 20 shown; rerun with --nocapture` |

And on success, one line saying what was proved:

```
items: 9,912 prototypes, 11 fields, 0 mismatches (13.1 s)
```

A gate that is silent when it passes cannot be distinguished from a gate that did
not run.

## Speed: the loop must stay usable

Measured on this machine: the suite was **156 s**, of which `prototype_ids` was
32 s because it parsed the 107 MB tree six times - and every gate re-parsed the
same tree for itself, run after run.

Two changes fixed it, and both are structural rather than a tweak:

* **The parse memo (`content::read_json`).** Parsed files are remembered for the
  life of the process, so the second loader to walk the tree pays nothing.
  `prototype_ids` went from 32 s to 10 s and `items` from 15 s to 8 s. Set
  `CDDA_NO_PARSE_MEMO=1` to measure the parser itself.
* **`tools/test_fast.py`.** Cargo runs one test binary at a time; this builds once
  and runs them concurrently, printing what each cost. The suite is now **44 s**
  end to end, and the table says which gate is slow the moment it becomes slow.

Rules:

* **Iterate on the fast suite.** `cargo test -p cdda-core-types` is 8 s and covers
  every P0 algorithm. Reach for the corpus gates when a change touches loading.
* **Watch the table.** A gate that climbs the timing table is a gate that will
  soon be skipped "just this once".
* **Never let a gate be skipped.** If a gate cannot pass yet, it asserts the exact
  size of what is missing (see `KNOWN_GAPS` in `prototype_ids.rs`) — an `#[ignore]`
  cannot fail, and a test that cannot fail is not a gate.

## Where a case lands

| Reference area | Port target |
|---|---|
Algorithms with no game state (line, point, units, rng, math parser, weighted list) | take as soon as the port has the type |
Content loading (`generic_factory`, `json_load`, `item`, `monster`) | behind the matching corpus gate |
World, character, combat, vehicles | with their phase, as that phase's acceptance criteria |

The tracker for which file has been taken is the Port status column in
[13-test-inventory.md](13-test-inventory.md); update `tools/test_inventory.py` when a
file's status changes, so the table stays derived rather than remembered.
