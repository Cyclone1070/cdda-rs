# 04 — Validation strategy

Premise: **absolute certainty is not attainable.** The C++ is not a specification —
it contains undefined behaviour and emergent effects, so "the same" means "the same
observably, on measured inputs." The goal is a system where divergence cannot be
merged unnoticed.

Implemented by [.github/workflows/port-validation.yml](../.github/workflows/port-validation.yml).

---

## Layers

| Layer | Catches | Cost | Runs on |
|---|---|---|---|
| Data digest | Content-layer drift: any prototype differing in any field | Seconds | Every PR |
| Conformance corpus | Behaviour the reference already asserts | Minutes | Every PR |
| Differential replay | Divergence in real sessions, per turn | 10–60 min | Every PR, sharded |
| Invariants | State corruption the hash chain cannot name | Minutes | Every PR |
| Action-stream fuzzing | Divergence outside the recorded corpus | Hours | Nightly |
| Visual, semantic | UI divergence: wrong glyph, wrong colour, wrong layout | Minutes | Every PR |
| Visual, pixels | Tileset rendering divergence | 30–60 min | Nightly |
| Performance | Turn-time regression | Minutes | Every PR |
| Long replay | Slow drift: save/load, overmap, long-running state | Hours | Nightly |

Order matters: cheap gates fail first, so a broken data layer never burns a
replay shard.

---

## The equivalence spec

Written down before any test is. Every field falls into exactly one bucket.

| Bucket | Meaning | Examples |
|---|---|---|
| **Exact** | Must match bit-for-bit after normalisation | positions, health, item contents, field strength, RNG state, message text |
| **Tolerant** | Must match within a stated epsilon | accumulated temperature, float-derived stats |
| **Ignored** | May differ freely; excluded from the digest | entity id values, cache contents, draw order, memory layout, timing |

Without this table every run "fails" on meaningless differences, and the gate gets
switched off within a week.

---

## The oracle harness

The reference binary must gain four modes. These **do not exist today** — this is
work to do in the reference build, and it is the foundation of everything else:

| Mode | Purpose | Status in reference |
|---|---|---|
| `--replay <session>` | Load a save, seed the RNG, apply an action list | Not present; `--seed` exists ([main.cpp:292](../references/Cataclysm-DDA/src/main.cpp#L292)) |
| `--dump-state <path>` | Canonical, sorted, normalised state per turn | Not present; save format is the basis |
| `--dump-frame <path>` | Logical UI frame per turn: glyph, fg, bg per cell | Not present; screenshot code exists ([game.cpp:8528](../references/Cataclysm-DDA/src/game.cpp#L8528)) |
| `--dump-prototype-digest` | Sorted digest of every loaded prototype | Not present; `--jsonverify` exists but validates rather than digests |

Keep these modes behind a build flag so they never ship to players.

---

## Gate 0: the oracle must be self-consistent first

Nothing above works if the reference disagrees with itself. It can, in principle:
the simulation draws from one global RNG stream while breaking ties by **heap
address** in at least two hot paths — vehicles are held in a pointer-ordered set and
the mover picks the maximum-moves vehicle with a strict comparison
([level_cache.h:81](../references/Cataclysm-DDA/src/level_cache.h#L81),
[map.cpp:729](../references/Cataclysm-DDA/src/map.cpp#L729)) — and loaded overmaps are
iterated in hash order
([overmapbuffer.h:581](../references/Cataclysm-DDA/src/overmapbuffer.h#L581)).

Three checks run before any other gate:

| Check | Method | If it fails |
|---|---|---|
| Self-consistency | Same binary, same session, twice → identical hash | The session is not a fixture; log and exclude it |
| Configuration sensitivity | Same session under sanitizer build and a second allocator | The session is order-sensitive; compare with the responsible systems masked |
| Cross-library RNG | Same session under the two target standard libraries | The oracle is platform-defined; pick one and pin it |

A session that fails any of these is recorded with the reason, not silently dropped.
The port adopts one canonical order for tie cases and documents every place where the
reference has none — a port with stable IDs is *more* deterministic than the
reference, and a divergence in that direction still has to be declared rather than
discovered later as a mystery.

---

## Determinism rules

Violating any one of these makes every downstream gate useless:

- RNG seeded from the session; single stream; identical call order in both builds.
- No wall-clock time inside the loop; fixed date injected.
- Stable iteration order everywhere. Ordered containers where the reference is
  ordered; explicitly sorted where it is not.
- UI stubbed or driven programmatically; redraws never alter simulation state.
- A digest run is rejected unless all of the above are confirmed — the harness
  fails loudly rather than producing a plausible-looking mismatch.

---

## Session format

A session is a small text file, versioned in the repo, reviewable in a diff:

```
# sessions/basic/001-doorway.jsonl
{"kind":"init","save":"saves/standard-01","seed":"equiv-0001","date":"2026-01-01T00:00:00Z"}
{"kind":"actions","turns":[{"move":"N"},{"move":"N"},{"use":"item: crowbar"}]}
{"kind":"expect","turns":50,"state":"exact"}
```

Fuzz failures are written back as sessions with their seed and action list, so a
divergence found once is never found twice.

---

## Visual testing

Two layers, deliberately different:

**Semantic frames (primary).** Capture what the renderer is *asked* to draw: one cell
per square, each with glyph, foreground and background. Compare grids cell by cell.
This is stable across terminals, fonts and renderer rewrites, and a mismatch points
at a coordinate you can inspect. Failures render a side-by-side PNG plus a list of
differing cells for the artifact.

**Pixels (secondary, tiles only).** Same scene, same tileset, same resolution, SDL
dummy video driver. Compare with a small per-pixel tolerance — enough to absorb
driver variation, not enough to hide a wrong tile. A pixel gate with a loose
threshold is worse than none, so this runs nightly rather than per PR.

The world view and the menus are both covered: the same semantic frame includes the
inventory panel, message log and sidebar, because in this game the interface *is*
part of the observable behaviour.

---

## When a gate fails

1. The hash chain names the first divergent turn.
2. Per-system traces in both builds name the system.
3. The state diff names component, entity and field.
4. A regression session is added before the fix is written.

Step 4 is not optional. Every divergence discovered becomes a permanent test, which
is the only mechanism that makes the corpus grow in the direction of real bugs.

---

## What this cannot catch

- **Input-space gaps.** Behaviours never exercised by the corpus or the fuzzer are
  unverified, and no amount of CI changes that.
- **Feel.** Whether combat pacing, balance or UI layout still feels right is not
  measurable by diffing state.
- **Undefined behaviour in the reference.** Where the original is UB, the harness
  locks in whatever the compiler happened to do.
- **Performance regressions outside the benchmark scenario.** The perf gate measures
  one late-game scene, not every scene.

---

## Running locally

```bash
# full differential run against a local oracle build
cargo run --release -p replay -- run --sessions sessions/ \
  --oracle ../oracle-build/cataclysm --port ./target/release/cdda-port

# one visual scenario, printable diff
cargo run --release -p replay -- frames --scenarios scenarios/visual/doorway.json \
  --oracle ../oracle-build/cataclysm --port ./target/release/cdda-port --out ./frames
```
