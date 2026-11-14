# 11 — Oracle harness

The reference build has to become measurable before anything can be compared to it.
This document is the design of that instrumentation: what gets added to the C++
reference, where it attaches, what it writes, and what it refuses to do.

The reference is never modified in behaviour. Every mode here is behind a build
flag, compiled out of release binaries, and must not change RNG draws, iteration
order, timing or file writes. If a mode changes the simulation, every result it
produces is worthless.

---

## Open defect: the reference's dice plateau

Measured, not guessed — `--dump-rng-probe` seeds the engine three times and takes three draws each:

| seed | draws |
|---|---|
12345 | 95, 84, **10** |
12346 | 71, 97, **10** |
12347 | 46, **10**, **10** |

The first two draws follow the seed; from the third on, the engine answers 10 whatever it was reseeded with, so any dump that needs more than a couple of distinct dice per seed records the plateau rather than a die. The cause is not yet established; `rng_set_engine_seed` seeds the same engine `rng()` draws from, and the standalone wrapper corpus (`corpus/rng/`) shows real streams, so the difference is in the game binary's use of it rather than in the generator.

**What this means for the corpus:** a recording that needs a die must record the die **it saw** and the port must replay exactly that (as `corpus/effect-activated.json` does), or must not roll at all. A dump that rolls and records only the *outcome* is asserting the plateau, not the rule — `corpus/per-turn.json` therefore contains no rolling shape, and the roll is pinned port-side on dice the test writes itself.

## `--probe-mapgen`: asking instead of guessing

`--probe-mapgen <oter_id>` runs one mapgen against a dummy `mapgendata` and prints the first row of what it
produced. It exists because the plan for a `--dump-map` flag assumed that a dummy mapgendata would do, and
that assumption had not been tested.

**What it found:** `run_mapgen_func( "shelter", dummy_dat )` **returns false**. A map cannot be built from a
dummy context, so a named-map dump needs a real `mapgendata` - one built from an overmap position
(`mapgendata( tripoint_abs_omt, map&, density, when, mission )`), which means the overmap has to exist first.

That is a small, concrete answer to a question that would otherwise have cost a round of guessing, and it is
why the probe is worth keeping: it is the tool for the assumption, not for the feature.

**Third look, and a decision.** `oter_mapgen`, the factory that builds JSON maps, is a **file-static inside
`mapgen.cpp`** - there is no public entry to build a named map on demand. The only path that builds maps is the
game's own generation, which needs an overmap and a map load. That is the same work as P9 world generation, so
the frame comparison is **not one flag away**: it waits on the port having a world, and the oracle having a way
to be asked.

The decision, recorded here so it is not rediscovered: **stop probing for a map dump now**. The frame gate has
both halves built and tested; what it lacks is a world to compare, and building one is P9's job rather than a
harness flag. The probes stay, because they answered the question in three rounds instead of thirty.

**Second probe, second answer.** Built with a real `mapgendata` from the player's overmap position instead of
a dummy, it **still returns false**. So the obstacle is not the context: `run_mapgen_func` is the reference's
path for **code** mapgens, and the JSON maps go through the mapgen factory's own builder. The next probe is the
one to write - a single call to that builder with the same id - before any dump flag exists.

## 1. Modes

| Mode | Writes | Used by |
|---|---|---|
| `--record <file>` | The session: what the player did, appended as it happens | Session capture |
| `--replay <file>` | Nothing; feeds recorded input instead of the keyboard | Differential replay, both builds |
| `--dump-state <file>` | A normalised fingerprint of the world, plus a full dump when asked | Every replay gate |
| `--dump-frame <file>` | Per cell: character, foreground, background, for each frame | Visual gate |
| `--dump-env <file>` | RNG position, calendar, and the options the simulation reads | Environment gate: needs an explicit `--seed` |
| `--dump-prototype-digest <file>` | Sorted digest of every loaded prototype | Content gate |
| `--dump-system-trace <file>` | Per-system fingerprints inside one turn | Localising a divergence |
| `--dump-butchery <file>` | How long each butchery action takes on each size of corpse, in four flag states | The butchery clock's parity gate |
| `--dump-vehicles <file>` | Every vehicle part and prototype, with durability, location, size, flags and armour | P8's store and mount rule |
| `--dump-vehicle-rotation <file>` | `coord_translate` over a grid of mounts at 0, 45, 90, 180 and 270 degrees | P8's facing gate |
| `--dump-vehicle-placement <file>` | A placed 2-seater helicopter: name, mounts, squares, fuel type and ammo, part by part | P8's placement gate and the fuel rounds |
| `--dump-vehicle-acceleration <file>` | What a vehicle's engine can add at a series of speeds, with the power, mass and top speed the rule divides | P8's thrust gate |
| `--dump-engine-power <file>` | One engine's power as it is damaged from whole to broken, with its health and `damaged_power_factor` | P8's damage-scaling gate |
| `--dump-vehicle-top-speed <file>` | Each vehicle's air drag, rolling drag, engine power and the top speed the reference's cubic makes of them; nine vehicles, including a rammed 4x4 and a pickup with an aisle | P8's top-speed and air-drag gates |
| `--dump-vehicle-rolling <file>` | What a vehicle's wheels resist rolling with: the unbroken wheels, each case's part weights, `coeff_rolling_drag`, and the power and top speed it feeds | P8's rolling and whole-chain gates |
| `--dump-vehicle-armour <file>` | Which part a wall would hit on an armoured quad bike and an unarmoured bicycle, before and after the plate is ruined | P8's armour gate |
| `--dump-vehicle-collision <file>` | A placed bicycle's one-square moves, detect-only: the hitting part, what it hits, the target part, and the projection used | P8's collision gate |
| `--dump-vehicle-mass <file>` | A placed bicycle with fifty rocks in its basket: each part's base and cargo mass, and the vehicle's total | P8's mass gate |

Plus one build-level addition: **coverage instrumentation** (clang source-based
coverage, the project already builds with clang in CI) so the harness can report
which code a session actually exercised.

---

## 2. Where each hook attaches

Verified attachment points in the pinned reference. These are chokepoints, not
convenience spots: a hook anywhere else would miss cases.

| Hook | Attachment point | Why here |
|---|---|---|
| Input | The input manager, below every screen <!-- verified: 0 raw `getch()` outside the input layer; 114 `handle_input` sites; 526 query/get_input sites --> | No screen reads the keyboard itself, so one hook sees every action, prompt, click and typed string |
| Frame | `ui_adaptor::redraw_invalidated` ([ui_manager.cpp:358](../references/Cataclysm-DDA/src/ui_manager.cpp#L358)), with the frame pump primed at [main.cpp:845](../references/Cataclysm-DDA/src/main.cpp#L845) | One place knows when a frame is complete, in both renderers |
| Draw | The window primitives (979 call sites) | Every glyph and colour the game asks for passes through them; the semantic frame is exactly this |
| Turn | After `do_turn()` returns ([do_turn.cpp:455](../references/Cataclysm-DDA/src/do_turn.cpp#L455)) | The turn is the atomic unit of the simulation, so it is the natural comparison boundary |
| Session start | RNG seed ([main.cpp:292](../references/Cataclysm-DDA/src/main.cpp#L292), applied at [:779](../references/Cataclysm-DDA/src/main.cpp#L779)) plus the save file | A session is reproducible only from save + seed |
| Save format | `chkversion` / `savegame_version` ([savegame.cpp:72](../references/Cataclysm-DDA/src/savegame.cpp#L72)) | Reading a save is itself a behaviour the port must match |

The backend is well contained: SDL events are polled in exactly two places, both
in one file, so the input hook does not have to chase platform code. <!-- verified: SDL_PollEvent appears only in sdltiles.cpp -->

---

## 3. Session file

One text file per session, line-delimited, diffable in review, small enough to
commit:

```
{"kind":"header","oracle":"0.I-1+harness","save":"saves/standard-01","seed":"equiv-0001",
 "date":"2026-01-01T00:00:00Z","terminal":"80x24","options":{"WORLD_COMPRESSION2":false}}
{"kind":"turn","n":0,"state":"sha256:..."}
{"kind":"input","turn":0,"context":"MAIN","action":"MOVE_N","raw":"key:k","mods":[]}
{"kind":"input","turn":1,"context":"INVENTORY","action":"CONFIRM","raw":"key:RETURN"}
{"kind":"frame","turn":1,"cells":"sha256:...","full":"frames/0001.txt"}
{"kind":"note","turn":1,"text":"player consumed item"}
```

Rules that make a session usable:

- Anything the player did that the game *branched on* is recorded, including text
  typed, mouse position, and the outcome of a timed prompt.
- The turn stamp is the game's turn counter, not wall-clock time.
- Options, terminal size and locale are part of the header, because they change
  what is drawn and sometimes what is simulated.
- Full frames are stored only for turns the visual gate checks; every other turn
  carries a hash.

---

## 4. Making a recording trustworthy

A recording is only evidence if it can be shown to be complete. The harness proves
this by replaying it **into the same binary that produced it**, with three possible
outcomes:

| Outcome | Meaning | Action |
|---|---|---|
| Reaches the end with identical turn hashes | Recording is complete and the reference agrees with itself | Session becomes usable |
| The game asks for input that is not in the recording | The recording missed something | Log the context and prompt, discard the recording, fix the recorder |
| Hashes diverge | The reference is order-sensitive here (address-dependent tie-breaks) | Quarantine the session, record why, keep it visible |

The third case is not hypothetical: see [04-validation.md](04-validation.md) Gate 0.
A session is never "fixed" by ignoring the mismatch.

During `--replay`, any attempt to read input that the recording does not contain
must fail loudly with a structured message naming the context, prompt and turn —
never substitute a default, never time out silently.

---

## 5. Constraints the harness must respect

Found while inventorying the interface; each one changes how the harness is built,
not whether it works.

| Constraint | Evidence | Consequence |
|---|---|---|
| Menus refuse to exist in data-check mode | `uilist::init()` asserts `!test_mode` ([uilist.cpp:466](../references/Cataclysm-DDA/src/uilist.cpp#L466)), and `test_mode` is set by `--check-mods` ([main.cpp:319](../references/Cataclysm-DDA/src/main.cpp#L319)) | Replay runs as a **normal session** with input injected, never in check mode. This is what makes screen capture of menus possible at all. |
| Some screens are clock- or locale-dependent | Title art and several date displays | Sessions freeze the date and locale in the header |
| Some screens need mouse input | 19 of the inventoried screens | The input hook records and replays mouse events, not just keys |
| Some screens need a prior state | A built vehicle, a founded camp, a live NPC | Those scenarios ship with a save file, not just a key sequence |
| Some screens are debug-gated | 14 screens | Scenarios state whether the debug menu is enabled |
| The map view depends on weather, light and RNG | — | Fixed seed, fixed date, fixed weather generator state |

The general rule: a screen is only "untestable" if no save plus key sequence plus
fixed environment can reach it. Everything else is a scenario waiting to be
written, and the screen inventory exists to make the list finite.

## 6. What the reference cannot give us

- **Feel.** Whether a fight is fun or a menu is readable is not in the data.
- **Unrecorded behaviour.** A session proves only what it exercised; the coverage
  mode exists to name what it did not.
- **Address-dependent behaviour.** Tie-breaks by pointer order cannot be
  reproduced, only declared.
- **Wall-clock and hardware behaviour.** Frame timing, thread scheduling and
  driver differences are excluded from every comparison by design.

---

## 7. Cost

| Artifact | Size | Notes |
|---|---|---|
| Session file | kilobytes per thousand turns | Input is tiny; hashes are 32 bytes |
| Full state dump | megabytes | Written only on mismatch or on demand |
| Full frame dump | hundreds of kilobytes per frame | Hashes everywhere else |
| Nightly fuzz corpus | grows continuously | Minimised sessions only |

The rule that keeps this affordable: **always compare hashes, materialise details
only when a hash disagrees.**

---

## 8. How the instrumentation is delivered

The pinned checkout is never edited, so every hook above lives in a patch series
applied to a build directory:

```
oracle/
  src/              disposable copy of the pinned tree (no .git, no translations)
  build/            out-of-tree build of that copy
  build.sh          copy, apply patches, configure, build
  patches/          numbered patches, each one self-contained and reviewable
    0001-state-dump.patch
    0002-prototype-digest.patch
    0003-record-replay.patch
    0004-semantic-frame.patch
    0005-coverage.patch
  flags/            harness CLI flags, one place, compiled out unless enabled
```

### Why a copy and not the pinned checkout

The reference's CMake writes generated files **into its own source tree**
(`src/version.h`, `src/prefix.h`, `VERSION.txt`, `CMakeFiles/`). Building in the
pinned checkout therefore dirties it and inflates any file count taken over
`src/`. The oracle is built from a disposable copy (`oracle/src`), which also gives
the patch series somewhere to live without touching the tree the documentation
cites by file and line.

### Building it (verified on macOS, 2026)

The reference builds headless with CMake and Ninja, but the CMake path does not
define `_XOPEN_SOURCE_EXTENDED` while its own Makefile does (`Makefile:628`), so a
wide-character ncurses build fails at `imtui-impl-ncurses.cpp:464` with `waddwstr`
undeclared. The configure line that works:

```
cmake -DTILES=OFF -DCURSES=ON -DSOUND=OFF -DLOCALIZE=OFF -DTESTS=OFF \
      -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS=-D_XOPEN_SOURCE_EXTENDED \
      -DCURSES_INCLUDE_PATH=/usr/local/opt/ncurses/include \
      -DCURSES_LIBRARY=/usr/local/opt/ncurses/lib/libncursesw.dylib
```

Rules for the patch series:

- One patch per mode, so a mode can be dropped without touching the others.
- Each patch touches as few files as possible, and never a hot loop.
- No patch changes behaviour when the harness flag is off; the flag defaults to off
  and release builds never enable it.
- Each patch carries the checks it must not break (RNG draws, iteration order).
- If a hook cannot be added without changing behaviour, it is not added — the mode
  is redesigned instead.

---

## 9. The prototype digest, staged

The plan calls for a prototype digest — a hash of every loaded definition, so that
"the port loaded the same content" is a fact rather than an impression. Building it
exposed a practical problem worth stating plainly: the reference has **88 prototype
registries** and no reflection, so a digest of *every field of every prototype* has
to be written one type at a time.

So the digest is staged, and each stage is a real gate rather than a placeholder:

| Stage | Content | Catches | When |
|---|---|---|---|
| 1 | Every registry's sorted prototype ids and counts | Missing, extra, renamed or unloaded prototypes | P1 |
| 2 | Full field digests for the types the port has reached | Wrong defaults, wrong inheritance, wrong parsing | P1 onward, per type |
| 3 | Full field digests for all registries | Everything above, everywhere | complete by P3 |

Stage 1 is generic — every registry exposes its contents — and it is the stage the
port can pass first, which is what makes it worth having before the per-type work.

### Digest definition (the port must reproduce this exactly)

- A registry contributes its ids, **sorted and deduplicated**.
- Every registry that reports the same type name is listed separately, and the
  list is sorted by type name.
- The per-registry digest is FNV-1a, 64 bit, offset basis
  `14695981039346656037`, prime `1099511628211`, computed over each id's bytes
  followed by one zero byte — so `{"ab","c"}` and `{"a","bc"}` differ.
- Output is one JSON object: the harness name and version, then for each registry
  its type name, count and the digest as sixteen lowercase hex digits.
- `--dump-prototype-ids` prints the same structure with the ids listed instead of
  hashed, for reading a difference rather than detecting it.

Both flags are read-only: they load data, write the file, and exit without touching
a save. They take a **path**, not stdout, because of the constraint in the next
paragraph.

### What runs in CI today

`.github/workflows/foundation.yml` runs on every push and pull request with no
reference checkout and no oracle build: formatting, lints as errors, the whole
test suite, the document checker, and a regeneration check on the inventory.
Every command in it was run by hand first.

The larger `port-validation.yml` stays aspirational until the crates it names
exist; its oracle pin has been corrected to the commit this repository actually
mirrors, and every job in it is behind the `PORT_CI` variable so it cannot fail a
build by accident.

### Resolved content, not just ids

`--dump-terrain` writes every terrain and furniture prototype *after* loading:
name, symbol per season, colour, move cost, light emitted, coverage, the stored
`transparent` bool and the flag set. 1,769 prototypes in 26 seconds,
byte-identical across runs. The bool is there because it **cannot be derived**
from the flags - a prototype can be opaque with an empty flag list, and the two
null prototypes are transparent without one.

This is the first oracle artifact that can prove a loader rather than a list of
names. Inheritance, `extend`/`delete` on flags, and the null prototypes all show
up in these fields, so a port that reads the files but merges them wrongly fails
here even with every id correct.

The dump is written from `mapdata.cpp` itself because the factories there are
file-static; one file-static walker inside its anonymous namespace, one exported
entry point after it closes.

The JSON is a list of lines rather than one object per prototype, deliberately:
a diff between two runs names the prototype that changed instead of reflowing a
thousand lines.

**A colour name is not a colour.** `color.cpp` builds two tables and picks one on
the terminal's colour count, and in the fallback table `c_dark_gray_red` is an
alias of `c_black_red`. A headless run has no terminal, so it reported the
fallback names — the harness now pins the 256-colour table, which is what a
player sees. Found by the field comparison, not by inspection: 23 prototypes
disagreed only on colour before the pin.

### The other two map-data lists

`--dump-traps` and `--dump-materials` add 125 traps and 193 materials. The traps
need two small patches because their factory and three fields are private
(`trap.cpp`, `trap.h`); materials are reachable through `materials::get_all()`.

They paid for themselves immediately. Bronze and pseudo fuel report a specific
heat of **zero**, not water's 4.186, because every heat field is read through
`optional` without an explicit default argument and that overload assigns the
template default when a fresh object omits the member (`material.cpp:103-105`).
The member initialisers in `material.h:90-92` never reach a loaded material.
A port that copied the header would have been wrong for 10 materials and right
for the 12 that state 4.186 in JSON.

### Field types, where inheritance runs inside an object

`--dump-fields` adds 145 field types with every intensity level: name, glyph,
colour, danger, transparency, move cost, concentration, light.

They earned their place by breaking two assumptions at once. A level inherits the
level *before it in the same object*, not another prototype - so a field's third
level is its second plus its own members. And concentration is the one field with
a default of its own rather than the inherited value, except that fog states
`0.5` and the reference's integer reader truncates it to `0`. Both rules are now
pinned in code rather than assumed.

### Items: 9,912 types, and the fields they expose

`--dump-items` writes every item type with its id, name and plural, glyph,
colour, weight in grams, volume in millilitres, price in cents, category,
materials and flags. 2.5 MB in 27 seconds.

Three findings came out of comparing it, none of which inspection had produced:

* A bare number in `weight` means **grams**, not the milligram the field is
  stored in. The reference's reader scales it; a port that treats it as the base
  unit is off by a thousand for every item that states a number.
* `proportional` beats `relative`, because items are read through the *reader*
  path (`generic_factory.h:1236-1241`), not through `assign` (`assign.cpp:58-75`).
  The one item in the content that states both wins by 500 grams either way.
* `replace_materials` rescales weight by the density ratio of the swapped
  material's share, so a steel sabre renamed to aluminium weighs a third as much
  (`item_factory.cpp:4001-4035`).

Category and flags came out of the same dump, and both were mostly derived
rather than stated:

* The category is derived **once, per prototype, after inheritance**, and only
  when the stated one names nothing the content registers. An item that copies a
  base which states none inherits the empty id and therefore gets a derived one -
  a port that derives at load time hands the base's answer to its children.
* Flags arrive from four places besides the file: a material table
  (`item_factory.cpp:3550-3591`), the charge-counting rule that makes anything
  stackable unrepairable, the non-player action chain
  (`item_factory.cpp:3619-3646`), and the `LIGHT_n` flags that are read into the
  light emission and then dropped. The order matters: comestibles replace their
  materials with the primary one *before* the table is consulted, so a wine whose
  fruit material is replaced by alcohol is not flagged as fruit.
* The dump carries the reference's own consolidated armour rows (`armor_data`),
  which turned the derivation from guesswork into a comparison: one row per body
  part, the first entry copied whole, later entries merged with weights measured
  before the row grows.
* Armour portions are accumulated in single precision. A 1.05 mm layer over two
  body parts is 209 hundredths of a unit in the reference, not 210, and the
  difference survives all the way to the dump.
* Liquids have no plural. `nname` forces the singular for anything whose phase is
  liquid (`itype.cpp:107-114`), so a soup, a drink or an ice cream prints its
  singular for any count. Fifteen items in the content state a plural that the
  reference never uses; a port that prints the stated plural disagrees on all of
  them.

### Monster types: 1,154, and three rules that differ from items

`--dump-monsters` writes every monster type with its id, name, glyph, colour,
body type, the numeric stats, weight, volume, species, categories and materials.
The whole set matches, and three things came out of it:

* Numbers go through `assign`, not through a reader, so **`relative` beats
  `proportional`** here (`assign.cpp:60-88`) - the opposite of items. A port that
  uses one order everywhere is wrong on exactly one of the two families.
* A monster that states no weight or volume is not zero: the type's own defaults
  are 81.499 kg and 62.499 litres, which is what makes the null type look like a
  person.
* The reference makes two kinds of monster that no file contains: `mon_null`,
  and one dormant twin per monster flagged `GEN_DORMANT`, which is the flag set
  rewritten, hp 5, speed 1 and no dodge. The id set only matches once both are
  generated.

Species, categories and materials are ordered by registration in the reference
and sorted in the port, so the comparison is by contents.

### About a sixth of the ids in the reference are not in any file

The id gate compares every registry against `--dump-prototype-ids`. Four
families came out of the comparison, and three of them are now reproduced
exactly:

* **Inline enchantments.** A mutation, bionic, effect or vehicle part may carry
  an `enchantments` array; each object in it becomes a prototype named
  `INLINE_ENCH_<parent>_<position>`. The position counts every entry, so a
  written id in the array still advances it. Bionics are the exception: they
  name their inline enchantments after the *text the player sees*, so a bionic
  called "Alarm System" produces `INLINE_ENCH_Alarm System_0`.
* **Martial-art buffs.** A style carries its buffs inline, one array per
  trigger. The member names are not fixed - `onhit_buffs`, `oncrit_buffs`,
  `onmiss_buffs` and others - so any member ending in `_buffs` is read.
* **Turret parts.** Every item that is a gun and not a gun modification becomes
  a vehicle part named `turret_<item>`, unless it carries one of the flags that
  make it unusable in a mount.
* **Inline effect-on-conditions** are the one family left: they are objects with
  their own id sitting in a slot inside furniture, tools and other prototypes.
  The slot names are per-type, so this needs the loaders that own those slots
  rather than a guess at which nested objects are conditions.

The remaining registries - overmap terrain, overmap terrain types and overmap
specials - are built from mapgen, which has no loader yet.

### The random-number corpus is the standard library, not the game

The engine and its distributions are implementation-defined, so "match the
reference" means "match libc++". `corpus/rng/wrappers.cpp` is a small program that
calls the reference's own wrapper layer - `rng`, `rng_float`, `normal_roll`,
`rng_normal`, `rng_bits`, `one_in`, `x_in_y`, `dice`, `roll_remainder`,
`rng_sequence` - against one engine seeded with 12345, in a fixed order, and
prints every value at full precision. Compiling it with the platform's libc++ and
running it once produces `corpus/rng/wrappers.csv`; the port must reproduce all
84 rows **exactly**, and a guard test proves a different seed does not.

Two things this pins that reading the wrappers alone would not:

* `rng_float` and `rng` swap reversed bounds rather than refusing them, so a
  call with the arguments the wrong way round consumes the same stream.
* The normal distribution caches the second value of each polar pair, so the
  engine advances in a pattern that depends on the *history* of calls: a wrapper
  that re-created the distribution each time would still look plausible and be
  wrong from the second call onwards.

### Replay runs without a terminal, and is reproducible

`--replay <file> --dump-state <file>` plays a recorded session: one key per line,
one turn per key, and writes the state at the end. No terminal, no
pseudo-terminal, no human.

Verified: `corpus/replay/walk-east.key` (walk east three times, wait twice)
reaches turn 5212804 from 5212800 — five inputs, four turns, because the first
turn does not advance the calendar — and two runs produce **byte-identical
state**. The frozen result is `corpus/state/replay-walk-east.json`.

Getting there needed four small hooks in the interface layer, all inert unless a
recording is armed (`oracle/patches/0001-prototype-digest.patch`, applied by `oracle/apply_harness_patches.py`):

| Hook | Why |
|---|---|
| The loading screen is not drawn | Without a terminal it blocked for thirteen minutes on the first attempt |
| The terminal-size and encoding loops do not run | They spin on a keypress waiting for a screen that is never going to be the right size |
| The event source serves the recording | This is the input itself |
| A popup is declined, and reported on standard error | It would otherwise wait for a key forever. A replay that silently took the "no" branch would be worse than one that says which prompt it declined |

Test mode stays **off** for a replay, because it changes simulation, not just
presentation: the map (`map.cpp:6788`, `:9595`), map memory, crafting and
proficiency caches, and `do_turn` all branch on it. A state produced under test
mode is therefore a *different artifact* from a replayed one — both are
reproducible, but the port must be compared against the replay, which is the path
a player takes. The cheaper dump remains useful for environment checks.

One reference defect surfaced here: `avatar::create` always writes a character
template (`newcharacter.cpp:870`), and if the template directory does not exist the
write fails and the game opens an error prompt. The harness creates the directory
first, which is what a player's installation has.

### The state dump works, and needs test mode to exist at all here

`--dump-state` starts a real game by the path in
[parts/headless-start.md](parts/headless-start.md), stops where the first input
would be needed, and writes the random stream position, the calendar, the avatar
and a tile-level digest of the reality bubble (14,641 tiles: terrain, furniture and
item count per square).

Verified: with `--seed 12345`, two runs from a clean save directory produce
**byte-identical output**, so the world the reference generates is reproducible and
the digest is a usable gate. The frozen artifact is
`corpus/state/state-seed12345.json`.

The run needs test mode, for a reason that matters for the replay design: without a
terminal, the loading screen blocks (the first attempt sat at "Loading files" for
thirteen minutes with no progress). Test mode suppresses that drawing
(`loading_ui.cpp:154`) and is safe for a dump, which reads nothing. A replay cannot
do this, because the input layer throws in test mode — so replay needs a null
terminal backend rather than a real one. That is the next piece of harness work,
and it is what makes replay possible on a machine with no terminal at all.

### Reaching the first turn without a human

Investigated in detail in [parts/headless-start.md](parts/headless-start.md); the
conclusions that shape the harness:

| Fact | Consequence |
|---|---|
| `start_game()` returning is the last point before input is required — the first `do_turn()` immediately reads a key | The state dump for "turn 0" is taken at that boundary |
| Test mode and turn driving are mutually exclusive: the input layer **throws** in test mode (`ncurses_def.cpp:410-415`) | The digest modes may use test mode; replay must not, which is why it needs its own binary path |
| Test mode is what makes the whole interface a no-op (`ui_manager.cpp:360`, `loading_ui.cpp:154`, `popup.cpp:288`) | Everything the harness skips under test mode is code the replay path will actually run |
| Two unbounded `getch()` loops in interface init (`ncurses_def.cpp:563-594`, `::607-619`) | The harness needs a terminal of at least 80x24 and a UTF-8 locale, or it hangs at startup |
| World or character names beginning `Test`, `Debug`, `Sandbox`… trigger a debug quick-setup inside `start_game()` (`game.cpp:1176-1179`) | Harness worlds must not use those names |
| `game::setup()` unloads and reloads all data | A harness step cannot keep data loaded across it |
| The reference's own tests never start a game — they build a flat synthetic world (`tests/test_main.cpp:88-168`) | The tests are a precedent for loading, not for playing; replay has to use the real path |

The minimal call sequence, and what it deliberately skips, are listed in the report.
The open question it raises is fidelity, not plumbing: `avatar::create` draws a
background and hobbies before a scenario can be installed, so an overridden
scenario is close to a normal one but not proven identical. That needs a
measurement, and it is the first experiment for the replay work.

### The environment dump, and why it needs a seed

`--dump-astronomy <file>` writes the sun's altitude and light over a year (424 samples),
which is what the astronomy port is gated against - a table from the reference rather than
trigonometry from memory. `--dump-env` prints the state that exists before any world does: the next four
draws of the random engine (taken from a copy, so measuring does not advance the
game's own stream), the calendar, and the five options the calendar and spawning
read. Two runs agree only when `--seed` is passed; without it the engine is seeded
from the clock, which is exactly the trap the old build had. Verified:

```
--seed 12345  -> rng_next [2139136712, 1446984157, 1357908071, 1052232628]
--seed 999    -> rng_next [2107762373,271962099,1021797077,2064231727]
```

That is the smallest useful fingerprint in the harness: before comparing worlds,
the two implementations must agree on where the random stream stands.

### First result (verified)

`--dump-prototype-digest` on the pinned data set produces **90 registries and
54,925 prototypes** in 31 seconds, and three consecutive runs produce byte-identical
files (same hash). That is the first artifact of the port's oracle: a measurement of
what the reference actually loads, not a description of it.

Two operational facts came out of making it run:

- `--datadir` must end in a slash. Several data paths are built by string
  concatenation, so `--datadir .../data` looks for `data/title/../..` as
  `datatitle/`. The failure surfaces as a missing `title/en.halloween`, which is a
  confusing way to learn it.
- An error during loading opens a popup and **waits for input**, which hangs an
  unattended run. The dump modes therefore set test mode the way `--check-mods`
  does; that is safe here because no menu is built. The replay harness cannot do
  this, which is the constraint recorded below.

### Curses initializes before the data load, and cannot be skipped

The first attempt skipped the terminal initialization for the dump modes, on the
theory that a data-dumping run does not need a screen. It hung: the process reached
`game_ui::init_ui()` and never came back, with nothing in the log and an empty
output file. The shipped non-interactive mode (`--check-mods`) avoids this by
setting test mode, which the replay harness must not do. So the dump modes behave
like `--jsonverify`: the terminal initializes, the data loads, and the harness
writes its output to a file rather than to stdout, which carries escape sequences
from the curses startup.

## 10. What the state dump contains

The dump is the contract for "the same", so its contents are enumerated rather
than implied:

| Included | Excluded (cache or presentation) |
|---|---|
| Every entity and its components, sorted by id | Light, vision and transparency arrays |
| Positions, health, energy, charges, timers | Pathfinding caches, spatial lookup tables |
| Prototype ids referenced by entities | Draw order, colours, fonts, window state |
| Calendar, weather, temperature, scent | Sound state, music, audio channels |
| RNG stream position | Free memory, addresses, container capacities |
| Options that affect simulation | Timing, frame counts, profiling counters |

Everything excluded is by definition reproducible from the included set, or it is
ignored on purpose. A field that is neither included nor reproducible is a
missing equivalence row, and the gate that needs it will say so.

---

## 11. Build order

The harness is P0/P1 work and blocks every later gate, so it is built first, in
this order:

1. State fingerprint and full dump (needs the save graph to walk).
2. Prototype digest (needs the content registries).
3. Replay driver and input hook.
4. Frame capture at the semantic level.
5. Coarse coverage report per session.
6. Per-system traces, added as each system is ported.

Each step is verified against the reference alone — no port required — which is
why this work can start immediately and in parallel with the first phases.

### `--dump-recipes <file>`

Writes every crafting recipe: its id, the item it makes, and whether it is blacklisted, plus the
total. 6,291 recipes in the pinned data.

The count matters more than it sounds: several documents in this repository quoted 5,632, a
number that had no dump behind it. A loader gated on a remembered figure is gated on nothing.

### `--dump-item-groups <file>`

Writes every item group the reference defines: its id, and every item it can produce with the
reference's own lower and upper figures. **8,230** groups, **281,546** spawn entries.

This replaced a remembered 5,031, the third figure in these documents that a dump has corrected.
The pattern is worth naming: counts written from memory are wrong by about ten per cent, and a
loader gated on one is gated on nothing.

### Probe: dumping the overmap directly (round 197, failed, reverted)

The frame gate needs a generated world, and the overmap decides what that world is, so the obvious next dump is the
overmap itself rather than the map. `--dump-overmap <file>` was written into the harness and it does not work yet.

What was tried: `overmap_buffer.get_existing()` at the player's overmap position, and - when that crashed - the
buffer's own `ter()` lookup, which is what the frame dump's terrain read goes through anyway.

What happened, both times: **SIGSEGV**, once inside `WORLD::has_compression_enabled()` reached from
`overmapbuffer::get_existing`, and once inside the buffer's terrain lookup. Before the crash the dump had already
written its header, and the header says the useful thing: **`"centre":[0,0]`**. The avatar's overmap position is the
origin, which means the headless start under this flag does not place the avatar the way the frame path does - and the
frame path is the one that passes `cli.start_location` and generates a map.

So the finding is not "the overmap cannot be dumped"; it is **"the overmap dump inherits the same setup the frame
dump needed, and this attempt skipped it"**. The next probe is to take the dump from inside the frame path, after the
map has been generated in the same run, rather than from a start of its own.

The change was reverted in full - the flag, the handler and the function - so the oracle is exactly what the patch
file says it is, and it rebuilds with no errors.

### Probe: the overmap dump from inside the frame path (round 198, failed, reverted)

The diagnosis from round 197 said the overmap read inherits the setup the frame dump needed, so this attempt did the
dump in the **same run** as the frame dump, after the map exists, with the centre taken from the map's absolute
position (`project_to<coords::omt>( get_map().get_abs_sub() )`) rather than from the avatar's overmap position - the
one that reported `[0,0]`.

It compiled and it ran, and it produced **nothing at all**: no frame file, no overmap file, within a 450 second
budget, with a save directory removed first. The log holds only the start screen's own text. The frame dump alone is
known to work, so the difference is the second flag or the run's length, and those are the two things to separate
next: run the frame dump alone again with the same budget and confirm it still produces a file, then add the overmap
call back with a longer budget and a check for the save directory appearing.

Reverted again in full. The oracle rebuilds with no errors and the patch file still describes it exactly.

### Probe: the frame dump alone (round 199) - the assumption was wrong

Last round's note said the frame dump is known to work, so the failure had to be the second flag. That was an
assumption and this round tested it: the frame dump **alone**, same invocation as before, save directory removed,
watched for **600 seconds**.

It produced **no file**, and the process was still alive when the watch gave up. The only evidence of progress is the
world directory, which now holds `Colonial Village` - a world was created, and the run never reached the dump.

So the frame dump is **not** known to work any more, and the thing that changed is not the flag. Two candidates
remain and they are both cheap to separate:

* the run needs longer than ten minutes, because a fresh world is generated first;
* the headless start no longer reaches the point the dump is taken from, and the world's creation is as far as it
  gets - the earlier crash's `centre [0,0]` points the same way.

The next probe watches the **save directory and the process**, not just the output file, and gives the run a longer
budget before concluding anything. A dump that takes eleven minutes is still a usable gate; a dump that never
happens is a different problem, and the difference is visible in what the save directory contains while it runs.

### Probe: does the oracle work at all? (round 200) - it loads for over fifteen minutes

The cheapest possible check, chosen because it needs no world and no map: `--dump-prototype-digest`, the dump the
corpus gates already depend on and which this same binary produced in an earlier round.

It never finished. The process was alive for **more than fifteen minutes** and `/tmp/digest.json` was never created.
Its log holds one loading-screen tip, which is printed while content is being read - so it is loading, not crashed and
not blocked on input.

That reframes the last three probes. The frame dump was never the suspect and neither was the overmap read: **this
binary takes minutes to load the reference's content on this machine, and the earlier dumps were taken when it did
not**. The machine has been carrying a load average of four to twenty for most of this session, which is the
difference between a dump that takes ninety seconds and one that takes a quarter of an hour.

What to do about it, in order:

1. **Measure the load, not the dump.** Run `--dump-prototype-digest` on an idle machine with a long budget and time
   it. If it finishes in a couple of minutes, every earlier probe was a timeout and the dumps work.
2. **Only then retry the frame and overmap dumps**, with a budget derived from that measurement rather than guessed.
3. If it does not finish even when idle, the binary is the problem and the oracle build needs looking at before any
   more dumping is attempted.

This is the check that should have come first, and it is written down here so the next attempt starts with it.

### Probe: is the oracle binary itself sound? (round 201) - no, it segfaults at startup

The measurement the last round asked for, and it answered a different question than expected.

First, a real fault of mine, found by checking the tree against its own patch instead of trusting it: **the oracle's
`main.cpp` had lost the harness's flags**. The revert two rounds earlier had removed my probe and, with it, every
`--dump-*` flag the harness adds - so the last three runs were a game starting with unknown arguments and sitting at
a screen, which is exactly what their logs showed. The file was restored from the committed patch, and the tree now
matches the patch apart from timestamps and blank lines.

Then the digest dump, with the flags back and a rebuilt binary: **SIGSEGV**, in the first seconds, right after
`Checking mod Dark Days Ahead [dda]` - before any content is read. That is the answer to all four probes: the binary
does not get far enough to be asked anything, so no dump could ever appear, and the "slow loading" reading of the last
round was wrong as well.

So the oracle build is what needs fixing, and the next steps are ordinary ones: build in a clean directory, and if it
still crashes at the mod check, bisect between the pinned reference's own build and this one. The port's side is
unaffected - every gate that runs is green - but **no reference dump can be taken until the oracle starts again**,
and that is the honest state of the frame gate.

### Resolved (round 202): the oracle was fine, the invocation was not

The segfault at the mod check was not the build. Running the same binary with **`--userdir oracle/userdir`** makes
everything work: the prototype digest in **30 seconds**, the frame dump in **30 seconds**, and the overmap dump in
**30 seconds**. The default home directory's config is what crashed it, and every probe from round 197 onwards had
been run without the userdir flag - so four rounds of "the dump does not work" were four rounds of the wrong command
line.

The lesson is written here rather than in a commit message: **the oracle has a canonical invocation, and it belongs
in the documentation next to the flags**, because a probe that fails for want of a flag looks exactly like a probe
that fails for want of a capability.

Canonical invocation:

```
oracle/build/src/cataclysm --userdir oracle/userdir --datadir oracle/src/data --dump-frame <file> --dump-overmap <file>
```

## `--dump-mods <file>`

Every mod the reference knows, with the dependencies it declares and the order its own `dependency_tree` resolves for it, plus the default mod list a new world
would load. The resolved list is the reference's answer and not a re-derivation, which is what makes it usable as a parity gate rather than as documentation.

## `--dump-geometry <file>`

The sizes of the world, read from the reference's own constants: a submap, the map, the vertical stack, the view distance. Cheap, needs no data, and pins the
numbers every later system divides by - including the two that are easy to conflate, the 132-square map and the 121-square view.
