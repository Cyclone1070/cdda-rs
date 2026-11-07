"""Fails when a public rule has no caller anywhere in the tree.

A rule the port has finished is a rule something calls: a system, another rule, or a test. A `pub fn` that nothing names is one of exactly three things, and all three want a human's eyes
before they land:

* **a rule waiting for its caller** - named in `docs/parity.md`, allowed here with the reason;
* **a leftover from a rename** - dead code, which should go;
* **a second implementation of a rule that already has one** - the failure this check exists for. Round 1358 wrote `melee::wear_chance` beside the already-ported
  `melee::melee_wear_chance`, and the only signal that anything was wrong was that nothing called the new one.

The check counts identifiers rather than parsing, which is enough for the question "does anything name this at all", and cheap enough to ride along with every gate run.
"""

import collections
import pathlib
import re
import sys

#: Rules that are ported and gated but whose caller is not written yet, with the reason they are allowed to be uncalled. Every entry is also named in `docs/parity.md`, because a
#: ledger that lives only in a script is a second source of truth about what is missing.
ALLOWED = {
    "is_friendly": "the faction rule, waiting for a faction store",
    "wielded_damage": "`Character::get_wielded_item`'s damage, waiting for the survivor's damage display",
    "gibs_scattered": "`mdeath::gib`'s scatter, waiting for the remains pipeline",
    "monster_spawn_attempts": "the spawn counter, waiting for the spawn schedule stage",
    "sunlight_through": "`lightmap`'s through-transmission, waiting for a single-square light query",
    "load_species_paths": "the species loader, waiting for the content table that reads it",
}

DECLARATION = re.compile(r"^pub fn ([a-z_][a-z0-9_]*)", re.M)
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def main() -> int:
    counts: collections.Counter[str] = collections.Counter()
    sources = {}
    for path in sorted(pathlib.Path("crates").rglob("*.rs")):
        text = path.read_text()
        sources[str(path)] = text
        counts.update(IDENTIFIER.findall(text))
    problems = []
    for path, text in sources.items():
        if "/tests/" in path or path.endswith("/tests.rs"):
            continue
        for name in DECLARATION.findall(text):
            # **Two is the threshold, not one**: the declaration itself is one occurrence, so a rule nothing calls has exactly one and a rule called once has two.
            if counts[name] <= 1 and name not in ALLOWED:
                problems.append(f"{path}: pub fn {name} has no caller and no ledger entry")
    for problem in problems:
        print(problem)
    if problems:
        print(
            f"\n{len(problems)} uncalled rule(s). Either call it, delete it, or add it to ALLOWED here **and** to docs/parity.md with the reason it waits.",
            file=sys.stderr,
        )
        return 1
    print(f"no uncalled rules outside the ledger ({len(ALLOWED)} allowed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
