#!/usr/bin/env python3
"""Check the planning documents for internal consistency.

Verifies:
  1. every relative link in docs/*.md resolves to a real file
  2. every `path:line` citation into the reference checkout points at a line
     that exists, and every `path#Lnn` / `#Lnn-Lmm` anchor is in range
  3. the phase totals in appendix-file-map.md match the phase table in
     05-roadmap.md, and both add up to the file count in src/
  4. every phase that appears in the appendix has a section in the roadmap
  5. citations written as `file:line` in prose name a file that exists

Exit code is non-zero if anything fails, so CI can gate on it.

Usage: python3 tools/check_docs.py [--docs docs] [--ref references/Cataclysm-DDA]
"""

import argparse
import pathlib
import re
import subprocess
import sys

CITE = re.compile(r"`([\w./+-]+\.(?:cpp|h|json|md|yml|yaml|py|txt)):(\d+)(?:-(\d+))?`")
ANCHOR = re.compile(r"\]\(([^)#\s]+)(?:#L(\d+)(?:-L(\d+))?)?\)")
PHASE_ROW_APPENDIX = re.compile(r"^\| (P\d+) \| (\d+) \| ([\d,]+) \|")
PHASE_ROW_ROADMAP = re.compile(r"^\| (P\d+ [^|]+?) \| (\d+) \| ([\d,]+) \|")


def _is_under(path: pathlib.Path, root: pathlib.Path) -> bool:
    """Whether `path` is inside `root`, comparing the way the filesystem will."""
    try:
        path.relative_to(root.resolve())
    except ValueError:
        return False
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--docs", default="docs")
    ap.add_argument("--ref", default="references/Cataclysm-DDA")
    args = ap.parse_args()

    docs = pathlib.Path(args.docs)
    ref = pathlib.Path(args.ref)
    problems = []
    checked = {"links": 0, "citations": 0, "anchors": 0}

    md_files = sorted(docs.glob("*.md"))

    # ---------------------------------------------------------------- 1 + 2
    for d in md_files:
        text = d.read_text(errors="ignore")
        for m in ANCHOR.finditer(text):
            target, lo, hi = m.group(1), m.group(2), m.group(3)
            if target.startswith(("http", "mailto")):
                continue
            checked["links"] += 1
            path = (d.parent / target).resolve()
            if not path.exists():
                # The pinned reference checkout is fetched, never committed (`.gitignore`), so a machine
                # without it - which is every CI runner - cannot check a link into it. That is a link it
                # could not check, not a broken one: count it and say so, so the gate reports how much of
                # itself ran instead of failing on a file it was never given.
                if not ref.exists() and _is_under(path, ref):
                    checked["unchecked_reference_links"] = checked.get(
                        "unchecked_reference_links", 0) + 1
                    continue
                problems.append(f"{d.name}: broken link {target}")
                continue
            if lo:
                checked["anchors"] += 1
                n = len(path.read_text(errors="ignore").splitlines())
                if int(lo) > n or (hi and int(hi) > n):
                    problems.append(f"{d.name}: {target}#L{lo}"
                                    + (f"-L{hi}" if hi else "")
                                    + f" exceeds file length {n}")

    # ---------------------------------------------------------------- 3 + 4
    appendix = docs / "appendix-file-map.md"
    roadmap = docs / "05-roadmap.md"
    if appendix.exists() and roadmap.exists():
        ap_text = appendix.read_text()
        rm_text = roadmap.read_text()

        ap_rows = {m.group(1): (int(m.group(2)), int(m.group(3).replace(",", "")))
                   for m in (PHASE_ROW_APPENDIX.match(l) for l in ap_text.splitlines()) if m}
        rm_rows = {}
        for l in rm_text.splitlines():
            m = PHASE_ROW_ROADMAP.match(l)
            if m:
                phase = m.group(1).split()[0]
                rm_rows[phase] = (int(m.group(2)), int(m.group(3).replace(",", "")))

        for phase, (files, loc) in ap_rows.items():
            if phase not in rm_rows:
                problems.append(f"phase {phase} is in the appendix but not in the roadmap table")
            elif rm_rows[phase] != (files, loc):
                problems.append(f"phase {phase} disagrees: appendix {files}/{loc} "
                                f"vs roadmap {rm_rows[phase][0]}/{rm_rows[phase][1]}")
        for phase in rm_rows:
            if phase not in ap_rows:
                problems.append(f"phase {phase} is in the roadmap table but not in the appendix")

        ap_total = sum(v[0] for v in ap_rows.values())
        if ref.exists():
            # Count tracked files so build artifacts generated into the tree
            # (version.h, prefix.h) can never skew the total.
            try:
                tracked = subprocess.run(
                    ["git", "-C", str(ref), "ls-files", "src"],
                    check=True, capture_output=True, text=True).stdout.split()
                actual = len([t for t in tracked
                              if t.endswith((".cpp", ".h"))
                              and "third-party" not in t
                              and "/" not in t[4:]])
            except Exception:
                actual = len([p for p in list((ref / "src").glob("*.cpp"))
                              + list((ref / "src").glob("*.h"))
                              if "third-party" not in str(p)])
            if ap_total != actual:
                problems.append(f"appendix covers {ap_total} files, reference has {actual}")

        for phase in ap_rows:
            if f"### {phase} —" not in rm_text:
                problems.append(f"phase {phase} has no section in 05-roadmap.md")

    # ---------------------------------------------------------------- 5
    for d in md_files:
        for m in CITE.finditer(d.read_text(errors="ignore")):
            path, line = m.group(1), int(m.group(2))
            candidate = (d.parent / path)
            if not candidate.exists():
                if (ref / path).exists():
                    candidate = ref / path
                elif (ref / "src" / pathlib.Path(path).name).exists():
                    candidate = ref / "src" / pathlib.Path(path).name
                else:
                    continue  # not a reference citation
            checked["citations"] += 1
            n = len(candidate.read_text(errors="ignore").splitlines())
            if line > n:
                problems.append(f"{d.name}: citation {path}:{line} exceeds file length {n}")

    print(f"docs checked: {len(md_files)}")
    print(f"  links resolved:   {checked['links']}")
    print(f"  anchors in range: {checked['anchors']}")
    print(f"  citations in range: {checked['citations']}")
    if checked.get("unchecked_reference_links"):
        print(f"  links into the unfetched reference, not checked: "
              f"{checked['unchecked_reference_links']}")
    if problems:
        print(f"\nPROBLEMS ({len(problems)}):")
        for p in problems:
            print(f"  - {p}")
        return 1
    print("\nOK: documents are internally consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
