# The local gate versus CI

Two rounds have reported the local gate clean and reached CI red, on code that had just been written.

| Round | Lint | Found |
|---|---|---|
| 47 | `float_cmp` ×2, `collapsible_if` | the "touch every file" step ran on this machine while clippy ran on the build machine, so cargo reused old fingerprints |
| 41 | `float_cmp` ×2 in a content test | no explanation; a planted `float_cmp` in the same file *is* caught, so the gate is not blind to content tests |

What is known:

- the clippy job runs `cargo clippy --workspace --all-targets -- -D warnings` on the build machine, so test files are in scope;
- it touches every `.rs` file on the build machine first, so fingerprints cannot hide a new lint;
- the sync copies `crates` wholesale with `rsync -az --no-times --delete`.

What changed in round 42 to close the unexplained case: the clippy job now **syncs again immediately before linting**. If the tree was ever copied
before the last edit landed, the second sync removes the gap. It costs one no-op rsync per gate.

What remains open: if a third unexplained green reaches CI red, the next step is to have the job print the remote copy's hash of every file it lints and
compare it against this machine's, so a stale tree fails loudly instead of silently.

## Third incident, and a mechanism

Round 53: a `map().unwrap_or()` lint reached CI red after a green local gate. Probing the gate again with the lint still present reported it, so the gate can see
this class of lint - the green that mattered was produced by a run in which the tree and the linter disagreed.

The mechanism is now named: round 42 added a **second `sync()` inside the clippy job**, which runs concurrently with the other jobs' rsyncs against the same remote
directory. A linter that reads a half-copied tree reports on code that is not the code. That sync is removed; the gate syncs once, before any job starts, and each
job touches every `.rs` file on the build machine so cargo cannot serve a stale fingerprint.

What the probe also teaches: a lint the gate misses can always be re-checked by running `--clippy` alone *after* the failed commit - that costs one step and turns
"unexplained" into "reproduced" or "not reproduced".
