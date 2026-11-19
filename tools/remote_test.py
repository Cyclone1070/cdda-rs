#!/usr/bin/env python3
"""Runs the gates on the remote build machine instead of this one.

The suite is fast but not free: it loads the reference's corpus several times over and it competes with everything
else on a laptop. The desktop has more cores and nothing else to do, so this syncs the workspace there and runs the
gates remotely.

Only what the gates read is synced - the crates, the corpus, the reference data the corpus gates load, the tools and
the docs. The build directories, the oracle's source copy and the reference's own checkout are not synced: they are
either rebuilt on the far side or not read at all.

Usage:
    python3 tools/remote_test.py              # sync, then run the fast suite and the document check
    python3 tools/remote_test.py --tests      # the suite only
    python3 tools/remote_test.py --docs       # the document check only
    python3 tools/remote_test.py --clippy     # clippy only
    python3 tools/remote_test.py --fmt        # formatting only
    python3 tools/remote_test.py --sync       # sync without running anything
    python3 tools/remote_test.py --oracle     # the oracle harness patch check only
    python3 tools/remote_test.py --jobs 8     # how many test binaries at once over there
"""

import argparse
import os
import pathlib
import subprocess
import sys

REMOTE = "cyc@elitedesk"
REMOTE_DIR = "cve-test"
ROOT = pathlib.Path(__file__).resolve().parent.parent

# Everything the gates read, and nothing else. `references/Cataclysm-DDA` is 6 GB of checkout; only `data` is corpus.
SYNC = [
    "scenarios",
    "Cargo.toml",
    "Cargo.lock",
    # Clippy reads its thresholds from here; a gate that syncs the code but not its configuration lints
    # the old rules.
    "clippy.toml",
    "crates",
    "corpus",
    "docs",
    "tools",
    "references/Cataclysm-DDA/data",
    # The document check counts the reference's sources and links into them, so the subset has to include them.
    "references/Cataclysm-DDA/src",
    "references/Cataclysm-DDA/doc",
    ".github",
]

EXCLUDES = [
    "*.rs.bk",
    "__pycache__",
    "target",
    "*.pyc",
]


def sync() -> None:
    """Copies the workspace to the remote, deleting files that no longer exist here.

    `--delete` is safe because it only ever applies to the paths listed in SYNC, each of which is a directory the
    remote copy owns outright. The remote directory itself is never the target of a delete.
    """
    print(f"syncing to {REMOTE}:{REMOTE_DIR}", flush=True)
    # `--no-times` matters: `-a` preserves modification times, and a machine whose clock is a little behind the server then delivers a "new" file
    # with an old timestamp, so cargo reuses the artifact it already has and the gate silently tests the previous round's code. Writing the time as
    # now for the files that actually changed costs nothing and removes a whole class of false green.
    args = ["rsync", "-az", "--no-times", "--delete"]
    for pattern in EXCLUDES:
        args.extend(["--exclude", pattern])
    for path in SYNC:
        source = ROOT / path
        if not source.exists():
            print(f"  skipping {path}: not here", flush=True)
            continue
        remote_path = f"{REMOTE_DIR}/{path}"
        # Directories are synced by their *contents* into a directory made first, so the far side ends up with
        # `crates/content` rather than `crates/crates/content`. This is the form that behaves the same on the old
        # rsync this machine ships and the new one on the server.
        if source.is_dir():
            subprocess.run([*ssh_args(), f"mkdir -p {remote_path}"], check=True)
            source_arg = f"{source}/"
            destination = f"{REMOTE}:{remote_path}/"
        else:
            parent = pathlib.PurePosixPath(remote_path).parent
            subprocess.run([*ssh_args(), f"mkdir -p {parent}"], check=True)
            source_arg = str(source)
            destination = f"{REMOTE}:{remote_path}"
        print(f"  {path}", flush=True)
        subprocess.run([*args, source_arg, destination], check=True)


def ssh_args() -> list[str]:
    """How to reach the build machine: with keepalives, so a dropped connection fails instead of hanging.

    A gate that blocks forever is worse than a gate that fails: two rounds were lost to an ssh that had gone quiet with nothing running on the far side. The keepalives make
    the client notice, and the timeout bounds the wait.
    """
    return [
        "ssh",
        "-o", "ServerAliveInterval=10",
        "-o", "ServerAliveCountMax=3",
        "-o", "ConnectTimeout=20",
        REMOTE,
    ]


#: How long one remote command may take before it is given up on. The slowest job (the whole suite) is under ten minutes when the machine is quiet.
GATE_TIMEOUT = int(os.environ.get("GATE_TIMEOUT", "2400"))


def run(command: str, label: str, jobs: int) -> int:
    """Runs one command on the remote and returns its exit code."""
    command = command.replace("{jobs}", str(jobs))
    print(f"\n=== {label} on {REMOTE}\n$ {command}\n", flush=True)
    # The rustup toolchain lives in the user's home and is newer than the distribution's, so it must come first.
    try:
        return subprocess.run(
            [*ssh_args(), f"cd {REMOTE_DIR} && export PATH=$HOME/.cargo/bin:$PATH && {command}"],
            timeout=GATE_TIMEOUT,
        ).returncode
    except subprocess.TimeoutExpired:
        print(f"\n=== {label}: gave up after {GATE_TIMEOUT}s", flush=True)
        return 124


def main() -> int:
    global REMOTE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tests", action="store_true", help="run the fast suite")
    parser.add_argument("--docs", action="store_true", help="run the document check")
    parser.add_argument("--clippy", action="store_true", help="run clippy")
    parser.add_argument("--fmt", action="store_true", help="check formatting")
    parser.add_argument("--oracle", action="store_true", help="check the oracle harness patch reproduces the working copy")
    parser.add_argument("--sync", action="store_true", help="sync only")
    parser.add_argument("--jobs", type=int, default=0, help="test binaries at once, default the remote cores less one")
    parser.add_argument("--remote", default=REMOTE, help="user@host to run on")
    options = parser.parse_args()
    REMOTE = options.remote

    everything = not (options.tests or options.docs or options.clippy or options.oracle
                      or options.fmt or options.sync)
    sync()
    # `--sync` alone means "only sync". Combining it with a check used to return here and run nothing,
    # which is how a run that looked green was a run that never happened.
    if options.sync and everything:
        return 0

    jobs = options.jobs
    if jobs <= 0:
        cores = subprocess.run([*ssh_args(), "nproc"], capture_output=True, text=True, check=True)
        jobs = max(1, min(6, int(cores.stdout.strip()) - 1))

    failed = []
    # Formatting runs here rather than on the build machine: `rustfmt` needs no target directory, and
    # the CI runner checks the same thing - the first CI run failed on it because nothing local did.
    if everything or options.fmt:
        if subprocess.run(["cargo", "fmt", "--all", "--", "--check"]).returncode != 0:
            failed.append("formatting")
    if everything or options.tests:
        if run("python3 tools/test_fast.py --jobs {jobs}", "the fast suite", jobs) != 0:
            failed.append("tests")
    if everything or options.docs:
        if run("python3 tools/check_docs.py", "the document check", jobs) != 0:
            failed.append("docs")
        # **A rule with no caller is a rule nobody has proved works** - or a second copy of one that does. Round 1358's duplicate passed every test its original passed; the only signal was
        # that nothing called it, so the gate that notices that now rides along with the docs check on every run.
        if subprocess.run([sys.executable, "tools/check_dead_rules.py"], capture_output=True).returncode != 0:
            subprocess.run([sys.executable, "tools/check_dead_rules.py"], check=False)
            failed.append("dead rules")
    # **The full run includes clippy.** It did not, and that cost a red CI: the complete gate reported green on a tree whose test target held an `items_after_statements`
    # error, because the run had only asked for tests, docs and the oracle. A gate that is not the gate is worse than no gate - so `everything` means every job.
    if options.clippy or everything:
        # **Touch every crate root first, or cargo's cache eats the gate.** `cargo clippy -D warnings` only fails on diagnostics it *emits*, and a crate whose fingerprint has not changed is
        # not re-checked - so a green run could mean "nothing was looked at". That is exactly what happened: the gate reported clean for many rounds while `crates/sim` held twenty-three
        # deny-level lints, and a fresh checkout of the same tree failed instantly. The touch is cheap (a mtime) and forces the diagnostics to be re-emitted.
        # **The touch has to happen on the build machine.** It was run here, against this checkout, while
        # clippy ran over there - so the files cargo looked at kept their old mtimes and a cached clean
        # result was reported for code that had just changed. Three rounds in a row reached CI with a
        # lint the gate said was clean: `float_cmp` twice and `collapsible_if`.
        # **Not a second sync here.** The gate syncs once before any job starts; a per-job sync runs concurrently with the other jobs' rsyncs and
        # can leave the linter reading a half-copied tree - which is how a green run reached CI red twice. The single sync plus the touch below is
        # what the gate needs.
        run(
            "find crates -name '*.rs' -exec touch {} +",
            "touch every source file on the build machine",
            jobs,
        )
        if run("cargo clippy --workspace --all-targets -- -D warnings", "clippy", jobs) != 0:
            failed.append("clippy")
    # The gate that rotted for several rounds because it was the one command nobody typed (round 1200). It is cheap - it runs
    # on this machine, against the pinned checkout - so it rides along with everything else.
    if everything or options.oracle:
        if subprocess.run([sys.executable, "oracle/apply_harness_patches.py", "--verify"]).returncode != 0:
            failed.append("oracle")
        else:
            print("the oracle patch reproduces the working copy", flush=True)

    print()
    if failed:
        print(f"FAILED: {', '.join(failed)}")
        return 1
    print(f"all green on {REMOTE} ({jobs} jobs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
