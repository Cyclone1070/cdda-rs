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
