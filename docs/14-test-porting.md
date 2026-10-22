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
  and runs them concurrently, printing what each cost, how much processor time the
  run used, and the machine's load. The table says which gate is slow the moment it
  becomes slow.
* **`tools/remote_test.py`.** The suite loads the reference's corpus several
  times over, which is exactly the work a laptop should not be doing while
  someone is using it. This syncs **only what the gates read** - the crates, the
  corpus, the reference's `data`, `src` and `doc`, the tools, the docs and the CI
  files - to `cyc@elitedesk` and runs the gates there: the same 950 cases in
  **19.6 s** on six Linux cores, with the laptop idle. `--tests`, `--docs`,
  `--clippy`, `--sync`, `--jobs N` and `--remote user@host`; the default is the
  whole gate. The far side needed three things the distribution does not provide:
  **rustc 1.98** (Fedora ships 1.97.1, below the workspace's minimum), **clippy**
  for that toolchain, and the reference's `src` and `doc` trees - the document
  check counts and links into them, and syncing only `data` made it report a
  broken link and "the reference has 0 files" rather than passing quietly.
* **One test binary per crate, not per file.** Cargo gives every file in `tests/`
  its own binary, and every binary pays the same startup before a single case runs:
  measured at **1.7 s each**, and unchanged by what the binary contains - a
  `--list` that runs no tests at all costs the same. At 153 files that was
  **250 s of the suite's 283 s**, and the cost could not be parallelised away,
  because concurrent starts on this machine serialise. The files are unchanged;
  each crate now has one `tests/<crate>_suite/main.rs` that lists them as modules.
  **283 s to 185 s**, with the same 873 cases, and a case is still named after its
  file: `frame_diff::the_same_frame_diffs_to_nothing`.

* **One load per corpus per process.** Merging the files exposed what the real cost
  had been all along: each case loaded the reference content for itself, and loading
  the item registry takes about seven seconds. `tests/content_suite/corpus.rs`
  remembers each registry behind a `OnceLock` and hands back a copy - a copy of nine
  thousand items costs a fraction of a second, loading them costs seven. Nothing
  bypasses a loader: every accessor calls exactly the loader a case would have
  called, with the same directories. Cases that read a **fixture** directory keep
  loading their own tree, because that is what those cases are about.
  `content_suite` went from **185 s to 23 s** on its own and the suite from **185 s
  to 96 s** wall.

* **Cases inside a binary run in parallel.** `--test-threads=1` was hiding the
  machine: `json_suite` takes 51 s of work and 16 s of wall when its four heavy
  gates overlap. With the harness left to decide, the whole suite went from
  **96 s to 33 s** and nothing failed. Four binaries at a time, not eight: measured
  on twelve cores, four and eight both finish in 33 s, and eight leaves the load
  average at 16 instead of 14. Same speed, more pressure on everything else.
* **`json_suite`'s two tree walks share one parse per file.** `data/json` is
  inside `data/`, so the wider gate was parsing the narrower gate's 3,013 files a
  second time; `tests/json_suite/parsed.rs` remembers each file's answer, taking
  that gate from 11.7 s to 8.0 s.

The suite is now **33 s** for 873 cases, from 283 s at the start of this work.

Builds run at `CARGO_BUILD_JOBS=8` and the oracle at `JOBS=8`. Compiling on all twelve cores
makes every other shell command on the machine wait minutes for a turn while the browser stays
responsive, which reads as a mystery and is only the job count.

Rules:

* **Iterate on the fast suite.** `cargo test -p cdda-core-types` covers every P0
  algorithm in seconds. Reach for the corpus gates when a change touches loading.
* **Read the footer, not just the table.** `kept N of 8 workers busy` separates "the
  gates got slower" from "the machine was busy"; anything over ten seconds is
  marked with a `*`.
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
