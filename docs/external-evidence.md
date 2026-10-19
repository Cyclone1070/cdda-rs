# CDDA: Would a Rust port and/or an ECS rewrite buy performance?

**Scope:** web evidence only, gathered 2026-10-19. CDDA = [CleverRaven/Cataclysm-DDA](https://github.com/CleverRaven/Cataclysm-DDA) (C++, ~788k LOC `src/` + ~346k lines JSON data; repo reports 3,070 contributor pages and 5,043 commits since 2026-01-01).

**Bottom line.** No evidence supports a *language* port buying meaningful speed; measured Rust-vs-C++ deltas are ~±10%. ECS has real measured wins only at 10k+ homogeneous entities iterated in bulk — a regime CDDA's turn loop does not occupy — and ECS's own practitioners say the benefit is code organization, not speed. CDDA's actual measured wins came from targeted C++ fixes. Every attempt to reimplement CDDA elsewhere stalled far from feature parity.

**Label key:** (a) measured benchmark · (b) developer opinion · (c) speculation/marketing.

---

## 1. Port / reimplementation attempts — all stalled short of parity

| Project | Lang / stack | Status (verified) | Scope |
|---|---|---|---|
| [rustaclysm](https://github.com/papow65/rustaclysm) | Rust + Bevy 0.19/0.20 | **Active**; 26★; last push 2026-10-17; commits 2026-10-14→17 | "3D reimagining"; loads C:DDA 0.G saves + JSON; features are walking, inventory, crafting, combat, sleeping, day/night. **Not** a port; **no** performance claims |
| [CataclysmLD](https://github.com/sndwch/CataclysmLD) | (Looming Darkness) | **Dead**; 0★; 0 forks; last push 2023-08-03 (repo created 2024) | Never reached usable state |
| [Cataclysm-DDA-Rust](https://github.com/gold-silver-copper/Cataclysm-DDA-Rust) | Rust | **Dead**; 0★; created 2026-07-26, last push 2026-08-03 (**8 days**) | "Persistent multiplayer Rust port" |
| [Dimensionfall](https://github.com/Dimensionfall/Dimensionfall) | **Godot / GDScript** (+C#) | Active since 2023-09, last push 2026-09-21; 45★ | "Inspired by" CDDA, not a port; ~3.1 MB GDScript |
| [Cataclysm: Bright Nights](https://github.com/cataclysmbn/Cataclysm-BN) | **C++ (stayed)** | Active; 1,266★; last push 2026-10-19 | The major CDDA fork. Chose *incremental profiling + CI regression gates*, not rewrite |

**Key negative finding:** the only fork with a large maintenance budget **deliberately kept C++** and pursued measured, incremental optimization. It added a [CI performance regression gate (#10622, 2026-10-16)](https://github.com/cataclysmbn/Cataclysm-BN/issues/10622): *"Track frame time and the hot paths the fork changes, so a change that makes the game slower shows up before a release"* — one fixed scenario, fixed seed, numbers kept per commit, threshold breach reports with profiles. It also maintains a [Linux test/CI shard profile](https://raw.githubusercontent.com/cataclysmbn/Cataclysm-BN/main/build-scripts/linux-test-profile.md). **No evidence found** of any fork or port claiming a Rust or ECS performance win.

---

## 2. What CDDA maintainers actually said

- **(b) The 2017 ECS evaluation thread** — [discourse.cataclysmdda.org/t/13405](https://discourse.cataclysmdda.org/t/evaluating-the-entity-component-system-model-for-dda/13405) (2017-02-15→18), opened by lead dev kevin.granade. Two findings still decisive:
  - Structural change is *the* blocker: *"adding or removing a component from an entity would require allocating a new id and copying all components to a new location… it might make it totally unusable, or it might be fine, hard to tell until you have a running system."* He called ECS on CDDA *"MASSIVELY invasive… tantamount to a total redesign and reimplementation of all affected code."*
  - Models don't map: Coolthulhu (2017-02-15) — creatures are a list, terrain/furniture are IDs on a grid, vehicles are multi-tile, projectiles don't outlive a turn.
  - This 2017 prediction was **later independently confirmed by measurement** (see §4: archetypes pay heavily on structural change).
- **(b) "An ECS would have a lot of trouble beating [the current design]"** — kevin.granade, [issue #20069 comment, 2017-02-13](https://github.com/CleverRaven/Cataclysm-DDA/issues/20069): *"The thing keeping us from making the reality bubble larger is scaling of various algorithms that act on it, such as field of view and dynamic lighting calculation, I don't see how an ECS would have any impact. On the contrary, if you look at the key data structures for the reality bubble, they're fully decomposed into multidimensional arrays of values for efficiency. An ECS would have a lot of trouble beating that."* Developer bdgackle in the same thread: *"I am not proposing a full refactor of the game to an ECS… that's tough to do incrementally, and we don't have nice decoupled blocks of logic that can be carved off into 'systems' either."*
- **(b) Engine conversion dismissed** — [discourse.cataclysmdda.org/t/18029](https://discourse.cataclysmdda.org/t/converting-the-game-into-using-higher-game-engine-possible/18029) (2018-12-21). kevin.granade: *"Ahahaha, no."* Another contributor: *"it'd be less of a conversion and tinkering task, and more of a 'literally rewrite it from scratch' type of task."* Note the extensibility counter-argument raised there: CDDA's moddability comes from JSON, not the engine — *"you can literally just define something in .json with a few lines of code and it will work right away."*
- **No evidence found** of any CDDA maintainer proposing or endorsing a Rust port. **No evidence found** of a multithreading proposal being rejected; searches surfaced only 3 issues mentioning "multithread" (the newest, 2023, was about the crafting menu).
- **Tracker negative result (strong):** GitHub search over `CleverRaven/Cataclysm-DDA` returns **0 issues with "ECS" in the title**, 1 with "rewrite", and only 4 mentioning "entity component" (none proposing an ECS architecture).

---

## 3. What is actually slow in practice — 731 issues carry the `Code: Performance` label (27 open)

All below are **(a) measured/reproduced reports** on the public tracker. Turn-time cost scales with **item/entity count** and **NPC activity**, not with rendering.

| Issue | Date | Reported symptom |
|---|---|---|
| [#88850](https://github.com/CleverRaven/Cataclysm-DDA/issues/88850) (S2-Confirmed) | 2026-09-29 | **2–5 min, up to 5–15 min per turn.** Backtrace shows NPCs evaluating *every one of thousands* of pills/welding rods as a weapon via `evaluate_best_weapon` |
| [#85644](https://github.com/CleverRaven/Cataclysm-DDA/issues/85644) | 2026-03-03 | 6 NPCs on a *short* craft (corn kernels, 16 min) → *"noticeable pauses every 4-5 turns"*; long crafts are fine |
| [#84196](https://github.com/CleverRaven/Cataclysm-DDA/issues/84196) | 2025-12-14 | Item quantity degrades many things incl. UI; *"the game iterates over every single item available"*. Debug-spawning 5,000 items **froze the game** |
| [#84569](https://github.com/CleverRaven/Cataclysm-DDA/issues/84569) | 2026-01-09 | Walking past ~2,000 identical ground items → delay; caused by autopickup re-scanning |
| [#84604](https://github.com/CleverRaven/Cataclysm-DDA/issues/84604) | 2026-01-10 | *"Unplayable lag with big bag of sugar"* — nested-container interaction |
| [#88435](https://github.com/CleverRaven/Cataclysm-DDA/issues/88435) | 2026-08-27 | Crafting recipe list load **>2 minutes**; worst in armor category |
| [#64799](https://github.com/CleverRaven/Cataclysm-DDA/issues/64799) (S2-Confirmed) | 2023-04-02 | Faction-camp NPC re-ran *"trying to find necessary items"* **300–400 times per move** |
| [#45634](https://github.com/CleverRaven/Cataclysm-DDA/pull/45634) | merged 2020-11 | Quicksave: **50% of time in `ui_manager::redraw()`** |

**The decisive counter-evidence — measured C++ wins, no rewrite needed.** A single contributor landed an *"NPC performance (EPIC)"* plus ~55 performance PRs. Each is a localized algorithmic fix with a measured number **(a)**:

| PR | Claim | Merged |
|---|---|---|
| [#45099](https://github.com/CleverRaven/Cataclysm-DDA/pull/45099) | Interned `string_id`: **≈30%** overall; `string_id` map keys were **13× slower** than `int_id` | 2020-10-30 |
| [#45130](https://github.com/CleverRaven/Cataclysm-DDA/pull/45130) | `Creature::has_effect`: **≈9.5%** of turn time removed — including by switching `unordered_map`→`std::map` for a small map | 2020-10-29 |
| [#45217](https://github.com/CleverRaven/Cataclysm-DDA/pull/45217) | Translation caching: **≈20%** on `LOCALIZE=1` builds; one call was up to 12% of turn time | 2020-11-05 |
| [#45634](https://github.com/CleverRaven/Cataclysm-DDA/pull/45634) | Quicksave **up to 2×** | 2020-11-26 |
| [#47330](https://github.com/CleverRaven/Cataclysm-DDA/pull/47330) | Fungus processing **30%+** | 2021-02-08 |
| [#43469](https://github.com/CleverRaven/Cataclysm-DDA/pull/43469) | Removed a duplicated `build_map_cache` call per frame; optimized `build_sunlight_cache` | 2020-09-07 |
| [#26276](https://github.com/CleverRaven/Cataclysm-DDA/pull/26276) | Map-memory slowdown fixed by changing an LRU to O(n) | 2018-10-21 |

These demonstrate the bottlenecks are **data-structure and call-frequency choices**, addressable in place. Note the corollary: several wins (interning, container choice, deduplicating a call) are **language-neutral**, so they would *not* be gifted by a Rust port — a port would inherit or re-create them. **No evidence found** of a CDDA benchmark quantifying map-memory or overmap-generation cost; the tracker shows two benchmark tests exist ([`vision_benchmark_test.cpp`](https://github.com/CleverRaven/Cataclysm-DDA/blob/master/tests/vision_benchmark_test.cpp), [`npc_trip_bench_test.cpp`](https://github.com/CleverRaven/Cataclysm-DDA/blob/master/tests/npc_trip_bench_test.cpp)).

---

## 4. Rust vs C++ — no meaningful speed difference

- **(a) Benchmarks Game, Rust vs C++ g++** ([rust-gpp](https://benchmarksgame-team.pages.debian.net/benchmarksgame/fastest/rust-gpp.html), [gpp-rust](https://benchmarksgame-team.pages.debian.net/benchmarksgame/fastest/gpp-rust.html), accessed 2026-10-19). Fastest-entry ratios (Rust/C++, >1 = C++ faster): fannkuch-redux 1.14×, k-nucleotide 1.27×, mandelbrot 1.07×, binary-trees 1.06×, regex-redux 1.06×, n-body 1.02×, spectral-norm **1.00×**, fasta **1.00×**, pidigits **0.99×**, reverse-complement **0.72× (Rust faster)**. **Everything is within ~±10% except one outlier each way.** The site itself warns *"How the programs are written matters!"*.
- **(a) The strongest Rust↔C port measurement available: `rav1d`** (AV1 decoder, C→Rust by Immunant/ISRG) — [memorysafety.org, 2024-09-10](https://www.memorysafety.org/blog/rav1d-performance-optimization/). Baseline C, same LLVM backend (Clang 18 / rustc 1.81). The mechanically transpiled Rust was **3.8% slower**. A bounds-check-removing compiler hack recovered **<1%** — so bounds checks were *not* the cause. Safe Rust peaked **≈11% slower**, reduced to **<6%** after extensive work; remaining gap attributed to codegen (stack usage/spills, branch-vs-`cmov` lowering) and `rustc`-vs-`clang` differences, not the language. **Crucially: assembly was reused from C**, and the team needed hand-written branchless rewrites whose stability they could not guarantee. **This is a direct C→Rust port measurement, and it is a small loss, not a gain.**
- **(b) The noalias argument, and its scale.** [LWN comment, 2025-09-19](https://lwn.net/Articles/1038860/): Rust's mutable-XOR-shared references let it *"directly emit noalias annotations on nearly all safe references, and so it has roughly the same performance advantage as Fortran,"* giving *"a small but measurable speed advantage."* The author's own word is **small**. Note `noalias` regressions do occur ([rust-lang/rust#149670](https://github.com/rust-lang/rust/issues/149670)).
- **(a) A "Rust is fast" case that is mostly *not* the language.** ripgrep beats `git grep`/GNU grep by ~3–10× ([ripgrep README](https://github.com/BurntSushi/ripgrep), i9-12900K) — but the win comes from parallel directory traversal, a lazy-DFA regex engine, and skipping UTF-8/locale decoding, not from Rust. Kill the parallelism and the gap collapses.
- **(a) Build-flag caveat, both directions.** CDDA's [Makefile](https://github.com/CleverRaven/Cataclysm-DDA/blob/master/Makefile) uses `-O3` and `-flto` but leaves `-march=native` **commented out** (line 490). So a Rust rebuild with `target-cpu=native` would likely show a few percent — but so would uncommenting that line in C++. That is a *configuration* delta, not a language one. **No evidence found** for a measured Rust-vs-C++ comparison on a CDDA-like turn-based simulation workload.

> **Q1 answer: no evidence of a performance gain.** Expect parity to a few percent *worse* short-term (per `rav1d`), plus total loss of 788k LOC, 346k lines of content data, 3,070 contributors' work, and the JSON mod ecosystem — for a bottleneck set (§3) that is already measurable and fixable in C++.

---

## 5. ECS vs OO — measured tradeoffs

- **(a) The most direct measurement: [Cox et al., "Run-time Performance Comparison of Sparse-set and Archetype Entity-Component Systems," EG UK CGVC 2025](https://eprints.staffs.ac.uk/9315/1/cgvc20251224.pdf)** (DOI 10.2312/cgvc.20251224; code: [StaffsUniGames/cgvc25-ecs-comparison](https://github.com/StaffsUniGames/cgvc25-ecs-comparison)). Conway's Game of Life, C++20, single-threaded, 5,000 frames, N=5,000 samples:

  | Entities | Archetype (median frame) | Sparse-set (median frame) | Verdict |
  |---|---|---|---|
  | 100 | 0.797 ms | 0.799 ms | **not significant** (p = .49) |
  | 1,000 | 0.799 ms | 0.798 ms | **not significant** (p = .376) |
  | 10,000 | 1.721 ms | 2.945 ms | archetype **1.7×** faster (p < .001) |
  | 50,000 | 7.410 ms | 13.819 ms | archetype **1.9×** faster (p < .001) |

  Entity setup latency: sparse-set **6.6× cheaper** (median 1,000 ns vs 6,600 ns). Conclusion as published: *"sparse-set ECSes enable cheaper entity modifications but scale poorly during iteration, while archetypes excel at large-scale iteration through cache efficiency but incur higher composition change costs."* **Below ~1,000 entities there is no measurable ECS benefit at all.**
- **(b) The library author says ECS is not primarily a performance tool.** Michele Caini (EnTT), [ECS back and forth Pt. 1, 2019-02-14](https://skypjack.github.io/2019-02-14-ecs-baf-part-1/): *"Don't be fooled by what is said around. Unless you are working on AAA games of a certain level, the main reason why you should use such a tool is the code organization and not (only) the performance… for a lot of games you won't have performance problem even with OOP."* He also notes the naive map-of-components ECS *"can be even worse in terms of performance if compared to classical OOP."*
- **(b) Structural change is the archetype tax** — [Pt. 10, 2021-02-27](https://skypjack.github.io/2021-02-27-ecs-baf-part-10/): table/archetype models *"get moved around after any change"*, so you must defer add/remove to a sync point, *"no matter what"* — *"the price for this speed comes when components (with data!) are added and removed."* [Pt. 2 insights](https://skypjack.github.io/2019-03-21-ecs-baf-part-2-insights/) shows the same tax on grouping. Unity documents the required workaround as [sync points](https://docs.unity3d.com/Packages/com.unity.entities@0.7/manual/sync_points.html). Caini also warns an opaque storage API introduces *"subtle bugs that are hard to spot"* when layout changes.
- **(a) The 200× figure is real but confounded.** [Honka, LUT bachelor's thesis 2026, "Performance comparison of Object-Oriented Design and Data-Oriented Design in Unity"](https://lutpub.lut.fi/bitstream/10024/172753/1/Bacherlorsthesis_Honka_Lukas.pdf): full Unity DOTS hit *"over 200 times faster frame times"* vs MonoBehaviours in brute-force crowd avoidance; **ECS alone yielded only 2×–7.5×**; and **single-threaded Burst beat non-Burst multithreading by 1.46×**, which the author attributes to **SIMD vectorization**. So ~30–100× of the "ECS win" is really **Burst/SIMD + C# Job System parallelism** — obtainable in C++ too, but only after the same data-layout restructuring. OOD *"was able to hold competitive performance at tiny scales but quickly degraded as entity counts grew."*
- **(a)+(b) The retrofit case study, and it says don't.** [Ylikoski, LUT master's thesis 2025, "Transferring Game Mechanics to Unity DOTS and ECS After Release"](https://lutpub.lut.fi/handle/10024/169074) — a **shipped commercial early-access game** (*Ages of Conflict*), 250,000 tiles, rebuilt two systems in DOTS and benchmarked 10 runs each:
  - `TileOccupation` (250k tiles, bulk, isolated): **0.37 ms ECS vs 1.68 ms OOD ≈ 4.5× faster**, job spread over 6–7 worker threads.
  - `MapUpdate` (must touch Unity objects — texture/colours): **1.71 ms ECS vs 1.06 ms OOD ≈ 1.6× SLOWER.** Cutting the texture transfer alone saved 1.30 ms; the author attributes it to *"significant overhead and extra processing when moving or retrieving this information to work with Unity Objects."*
  - Verdict: *"while the raw performance results displayed improvements, difficulties in implementing the systems and how interconnected the base systems are with each other, it was ultimately deemed that the process would not be worth it to do for the full product. DOTS is a great tool when its usage is planned from the start and the project architecture is written to revolve around it."*
  - Effort/quality notes: steep learning curve, *"learn an entirely new language within C#"*; Burst unavailable because `ISystem` cannot touch managed/Unity classes, which *"basically prevents them from interacting with the rest of the objects in Unity"*; query-mutation errors surface only at runtime without line numbers.
  - **This maps onto CDDA almost exactly:** bulk terrain/creature iteration would speed up; UI, rendering, save/load, and every system that must interop with the existing non-ECS world would be neutral or slower.

> **Q2 answer:** an ECS would plausibly speed up *bulk* iteration and would marginally help extensibility *in code*. But CDDA's extensibility is deliberately delivered through JSON content data (see the 2018 forum point in §2), and CDDA's hot paths (§3) are heterogeneous per-entity logic and item-list scanning — the archetype/sync-point worst case, not the 50,000-homogeneous-cell case. Expect neutral-to-negative on average, plus a Ylikoski-scale abandonment risk.

---

## 6. Large mature games rewritten/ported — measured before/after

- **(a)+(c) RimWorld → Waylith engine, 2026-10-12** — [Ludeon blog](https://ludeon.com/blog/2026/10/announcing-the-waylith-engine-and-update-1-7/). Moved off Unity to an in-house engine: **2.2× on a stress-tested late-game colony**, a 1,500-mod pack's load dropping **14 min → 6 min**, faster startup/save-load/mapgen/worldgen/tick rate; savegames and most mods kept working. **Two caveats that deflate the analogy:** (1) vendor-reported, *"individual performance improvements may vary"*; (2) **RimWorld stayed in C#** — this is an *engine* rewrite by a paid studio, not a language or ECS port. Corroboration: [Rock Paper Shotgun](https://www.rockpapershotgun.com/rimworld-switches-engine-from-unity-to-the-devs-own-faster-waylith-engine-all-apparently-without-breaking-mods-somehow), [80.lv](https://80.lv/articles/rimworld-runs-2-4-times-better-on-new-in-house-engine).
- **(a)+(b) Ylikoski 2025** (§5): the clearest documented *failure to justify* an ECS retrofit — real speedups on the bulk system, a real slowdown on the interop system, abandoned on effort/risk grounds.
- **No evidence found** for: any released game rewritten from C++ to Rust with published before/after numbers; any CDDA-scale (500k+ LOC) OO→ECS rewrite with measured results and person-year accounting; any published person-year or LOC cost for porting CDDA specifically. Treat any such figure as unverified.
- **Effort is only bounded, not quantified** here: CDDA carries 3,070 contributor pages and 5,043 commits since 2026-01-01 alone; the 2018 forum answer put a clone at *"years for a dedicated team"* **(b)**.

---

## 7. What the evidence supports instead

1. Keep C++. Add/expand the benchmark harness CDDA already has (2 bench tests) into a CI regression gate, copying [Bright Nights #10622](https://github.com/cataclysmbn/Cataclysm-BN/issues/10622). **This is the only approach in the entire evidence set with a track record on this codebase.**
2. Attack the measured hotspots in §3 — `evaluate_best_weapon` (#88850), item-count scanning (#84196/#84569), recipe-list load (#88435), NPC item search (#64799). Prior wins of 9.5–30% each show the headroom is real and localized.
3. If bulk iteration ever becomes the bottleneck, apply **data-oriented layout changes in C++** (SoA for the specific hot array) *before* considering an ECS. The 2025 CGVC result says the cache-coherence win is what actually pays — and it needs 10k+ homogeneous entities to appear.
4. Consider enableable/filter components rather than structural changes, and batch add/remove at defined sync points — the concrete mitigations both Caini and the CGVC paper identify.

**Explicit gaps — no evidence found:** CDDA-specific profiling numbers for map memory or overmap generation; measured compile-time cost of CDDA's current build vs a hypothetical Rust build; any CDDA maintainer statement on Rust; any ECS-based sandbox-roguelike at CDDA's simulation depth (searches for ECS roguelikes returned only small demos, ≤102★).
