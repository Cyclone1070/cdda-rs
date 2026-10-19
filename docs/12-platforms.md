# 12 — Platform plan

**Decision (settled, revised):** the port targets **desktop only for now** —
macOS, Linux and Windows, from one codebase. Android and web are **deferred, not
cancelled**: nothing may be built in a way that makes them unreachable later.
Save compatibility with existing worlds is **not** in scope.

This document records what each platform actually costs, measured from the
reference rather than guessed, and what "keep the door open" means concretely.

---

## 1. What the reference does today

| Platform | Build system | Renderer | Audio | Extra input | Packaging | Shipped? |
|---|---|---|---|---|---|---|
| Linux | CMake (primary), Makefile | SDL2 or terminal | SDL2_mixer | keyboard, mouse, gamepad | tar.gz, snap, two Flatpak manifests | yes |
| Windows | MSVC projects, vcpkg, MinGW | SDL2 or terminal | SDL2_mixer | keyboard, mouse, gamepad | zip | yes |
| macOS | Makefile | SDL2 or terminal | SDL2_mixer | keyboard, mouse, gamepad | dmg, universal binary | yes |
| Android | ndk-build + Gradle | SDL2 with GLES | SDL2_mixer, mpg123 | touch: virtual joystick, shortcuts, vibrate | apk (x64, x32) | yes |
| Web | Makefile + emsdk | SDL2 via WebGL | SDL2_mixer | keyboard, mouse | bundle | **built in CI, not released** |

Measured platform-specific code in `src/` (excluding vendored libraries):

| Marker | References | Files |
|---|---|---|
| Android (`__ANDROID__`) | 124 | 33 |
| Windows (`_WIN32`) | 83 | 30 |
| Apple (`__APPLE__` and friends) | 32 | 12 |
| Linux-specific (`__linux__`) | 15 | 5 |
| Web (`__EMSCRIPTEN__`) | 1 | 1 |

Plus 67 files under `android/` and 19 under `msvc-full-features/`, both thrown away
in a port.

---

## 2. What that tells us

**Desktop is the easy half, and the user's instinct is right there.** Linux, macOS
and Windows share one window/render/audio surface, and the port replaces it with
equivalents; the platform-specific code in the reference is mostly path handling,
file dialogs and one-off workarounds.

**The web build is not code-heavy but is the riskiest.** One `__EMSCRIPTEN__` guard
in the entire source tree means the *game* needs no special casing — it works
because SDL, the filesystem shim and the audio backend are compiled for the target.
The port would inherit none of that: it must supply its own file access, its own
audio path, and a runtime without threads. It is also the one target the reference
does not actually ship today.

**Android is real work, and it is not in the simulation.** 124 references across 33
files plus a whole Gradle project. The simulation code barely differs; what differs
is input (touch, gestures, on-screen controls), the graphics path (GLES), and the
packaging pipeline.

**The interface is where platform differences become visible**, which matters
because the visual gate compares what the game asks to draw. Font metrics, hinting
and DPI differ per platform, so:

- the **semantic frame gate** must produce identical logical frames on every
  platform — a platform that changes layout is a declared divergence, not a pass;
- the **pixel gate** keeps one baseline per platform, because glyph rasterisation
  legitimately differs.

---

## 3. What the port must provide per platform

| Concern | Choice to make | Why it is safe |
|---|---|---|
| Window, input and events | SDL2-equivalent binding | The reference already isolates this behind one window abstraction and one event poll site |
| Rendering | Same graphics path on all platforms, GLES on Android | The reference already spans exactly this range |
| Audio | SDL2_mixer-equivalent | Audio is not part of the state comparison |
| Text and fonts | Consistent metrics per platform, declared per platform | Fonts are the main source of legitimate pixel differences |
| Filesystem and user directories | One path abstraction, per-platform roots | The reference already has exactly one path layer, with platform roots |
| Touch and gamepad | Android-specific affordances kept as they are today | The reference's Android UI is a behaviour to reproduce, not to redesign |
| Threads | Optional everywhere, required nowhere | The simulation is single-threaded by design; web needs no threads |
| Packaging | Five pipelines, nine artifacts | Mirrors the reference's release matrix |

---

## 4. Cost, stated plainly

| Platform | Difficulty | Where the work actually is |
|---|---|---|
| Linux | low | Packaging variety (tar, snap, two Flatpak manifests — the reference ships two divergent ones) |
| Windows | low to moderate | Build and runtime packaging; MSVC-era workarounds do not carry over |
| macOS | low | Universal binary and signing/notarisation are the real tasks |
| Android | moderate | Touch input equivalence, GLES path, Gradle packaging, on-device performance |
| Web | high, and unproven here | File access, audio, memory limits, and no reference release to compare against |

The honest summary: **the three desktop targets are one codebase and ordinary
engineering.** Android and web are deferred; the web target remains the only one
that could force a design change, which is exactly why it is deferred rather than
promised.

---

## 5. Keeping Android and web reachable

Deferring them costs nothing today **if** these hold. Each is a cheap constraint
now and an expensive rewrite later:

| Constraint | Why it preserves the option |
|---|---|
| One window, input and event abstraction; no OS calls in simulation code | Android and web replace exactly that layer |
| One filesystem and user-directory layer, no absolute desktop paths | Web has no real filesystem; Android sandboxes differently |
| No feature that *requires* threads | Web cannot rely on them |
| All content loaded through one loader, no memory-mapped assumptions | Web file access differs |
| Renderer choice stays separate from simulation | Android needs a different graphics path |
| Touch and gamepad input modelled as events, even if unused on desktop | Android input arrives as touch, not keys |
| Audio behind one interface, with a null backend | Web audio is asynchronous and permission-gated |

None of these are speculative generality: every one of them is already how the
reference behaves, so following it is the default rather than extra work.

## 6. Effect on the plan

- **Roadmap P11** carries the platform work; its gate is "artifacts build and
  smoke-run on macOS, Linux and Windows".
- **CI** gains one build job per desktop target, plus the existing gate jobs.
- **Testing**: the full corpus runs on desktop. If Android or web are picked up
  later, they get a smoke suite (launch, load, draw a frame, take one action, save)
  rather than the full corpus, because the simulation is identical and only the
  shell differs.
- **Gate 0** (see [04-validation.md](04-validation.md)) runs per target: if a
  target disagrees with itself, that is found before it is blamed on the port.
- **Still open:** whether the port keeps the reference's mutually exclusive
  terminal and graphics builds. Recommendation: keep both — the terminal build is
  what makes headless comparison cheap.
