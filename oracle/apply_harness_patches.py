#!/usr/bin/env python3
"""Applies - or regenerates - the harness patch that turns the pinned checkout into the oracle.

The patch lives in `oracle/patches/0001-prototype-digest.patch` and is the **only** authority for what the
oracle adds to the reference. This script has three modes:

    (no argument)   apply the patch to the working copy (`oracle/src`), or report that it is already applied
    --verify        prove the patch reproduces the working copy byte for byte from the pinned checkout
    --regen         rewrite the patch from the working copy (do this after editing `oracle/src`)

Why one patch instead of a hundred string replacements: until round 307 this file applied its edits call by call,
and the working copy drifted away from it - `main.cpp` grew eleven dump branches that the script never learned, so
the script's own `--verify` failed and re-running it would have quietly deleted them. A patch generated from the
working copy cannot drift that way, and `--verify` turns "should be reproducible" into a command with an answer.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "oracle"
REF = ROOT / "references" / "Cataclysm-DDA"
SRC = ORACLE / "src" / "src"
PATCH = ORACLE / "patches" / "0001-prototype-digest.patch"


def run(args: list[str], cwd: Path = ROOT) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True, check=False)


def apply_patch(dry_run: bool, tree: Path, reverse: bool = False) -> subprocess.CompletedProcess[str]:
    """Applies the patch with the repository layout the patch paths assume."""
    flags = ["patch", "-p0", "-s", "-f", *(["--dry-run"] if dry_run else []), *(["-R"] if reverse else [])]
    return run([*flags, "-i", str(PATCH), "-d", str(tree)])


def apply(tree: Path) -> None:
    if not SRC.is_dir():
        sys.exit(f"oracle working copy missing at {SRC}; run oracle/build.sh first")
    probe = apply_patch(dry_run=True, tree=tree)
    if probe.returncode == 0:
        done = apply_patch(dry_run=False, tree=tree)
        if done.returncode != 0:
            sys.exit(f"patch failed:\n{done.stdout}\n{done.stderr}")
        print(f"harness patch applied ({PATCH.name})")
        return
    # An already-patched tree refuses the forward patch and accepts it backwards, which is the honest way to
    # tell "already applied" apart from "the working copy has drifted": with `-f` there is no prompt to read.
    backward = apply_patch(dry_run=True, tree=tree, reverse=True)
    if backward.returncode == 0:
        print(f"harness patch already applied ({PATCH.name})")
        return
    sys.exit(f"patch does not apply:\n{probe.stdout}\n{probe.stderr}")


def regenerate() -> None:
    """Rewrites the patch from the working copy, which is how a harness edit becomes durable."""
    result = run(["diff", "-ruN", "references/Cataclysm-DDA/src", "oracle/src/src"])
    # `diff` exits 1 when it finds differences, which is the normal case here.
    if result.returncode not in (0, 1):
        sys.exit(f"diff failed:\n{result.stderr}")
    if not result.stdout.strip():
        sys.exit("the working copy matches the pinned checkout; nothing to regenerate")
    PATCH.write_text(result.stdout)
    print(f"harness patch regenerated: {PATCH} ({len(result.stdout.splitlines())} lines)")


def verify() -> None:
    """Proves the patch rebuilds the working copy from the pinned checkout in a scratch tree."""
    if not SRC.is_dir():
        sys.exit(f"oracle working copy missing at {SRC}; run oracle/build.sh first")
    with tempfile.TemporaryDirectory() as scratch_name:
        scratch = Path(scratch_name)
        (scratch / "references").mkdir(parents=True)
        shutil.copytree(REF, scratch / "references" / "Cataclysm-DDA", symlinks=True)
        (scratch / "oracle" / "src" / "src").parent.mkdir(parents=True)
        shutil.copytree(REF / "src", scratch / "oracle" / "src" / "src", symlinks=True)
        applied = apply_patch(dry_run=False, tree=scratch)
        if applied.returncode != 0:
            sys.exit(f"patch does not apply to the pinned checkout:\n{applied.stdout}\n{applied.stderr}")
        difference = run(["diff", "-r", str(scratch / "oracle" / "src" / "src"), str(SRC)])
        if difference.returncode != 0:
            sys.exit(f"the patch does not reproduce the working copy:\n{difference.stdout}")
    print("verify: the patch reproduces the working copy from the pinned checkout, byte for byte")


if __name__ == "__main__":
    if "--verify" in sys.argv:
        verify()
    elif "--regen" in sys.argv:
        regenerate()
    else:
        apply(ROOT)
