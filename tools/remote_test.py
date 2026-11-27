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
import base64
import re
import os
import pathlib
import subprocess
import sys
import time

REMOTE = "cyc@elitedesk"
REMOTE_DIR = "cve-test"
ROOT = pathlib.Path(__file__).resolve().parent.parent

#: What a **focused** run needs: the code, its configuration, the tools and the documents. These are small - a change to
#: the workspace copies in about a second - so every focused gate syncs them.
SYNC = [
    "scenarios",
    "Cargo.toml",
    "Cargo.lock",
    # Clippy reads its thresholds from here; a gate that syncs the code but not its configuration lints
    # the old rules.
    "clippy.toml",
    "crates",
    "docs",
    "tools",
    ".github",
]

#: What only a **full** run needs: the reference's data (read by the corpus gates), its sources and its documentation
#: (read by the document check's citation counts), and the corpus itself. Together they are nearly 300 MB, and with
#: `rsync -c` that is about ten seconds of hashing on every sync - a third of a focused gate's whole cost, spent asking
#: a question whose answer has not changed since the reference was pinned. They are copied when a full run asks for
#: them, when a changed path is inside them, or when the far side has never had them.
BIG_SYNC = [
    "corpus",
    "references/Cataclysm-DDA/data",
    # The document check counts the reference's sources and links into them, so the subset has to include them.
    "references/Cataclysm-DDA/src",
    "references/Cataclysm-DDA/doc",
]

#: A file on the build machine that says the big directories were copied at least once.
BIG_STAMP = "cve-test/.big-sync-stamp"

EXCLUDES = [
    "*.rs.bk",
    "__pycache__",
    "target",
    "*.pyc",
]


#: How many times one copy to the build machine is attempted before the gate gives up on it.
#:
#: **A dropped link is not a failing gate.** One round lost a full run to an `rsync` that returned 255 halfway through the sync - nothing had been checked and nothing was wrong
#: with the tree. Another lost a test run whose remote half had already passed while the session that carried it stayed open. A single attempt turns either into "FAILED" for the
#: wrong reason, so the copy is retried, and the retry is bounded so a genuinely broken remote still fails the gate instead of living in it.
SYNC_ATTEMPTS = int(os.environ.get("CVE_SYNC_ATTEMPTS", "3"))
#: How long to wait between attempts, in seconds. Overridable so a test can run the retry without the wait.
SYNC_RETRY_DELAY = float(os.environ.get("CVE_SYNC_RETRY_DELAY", "5"))
#: How long one attempt may take. A copy of this workspace is a few seconds on a working link; ten minutes means the link is dead, not slow.
SYNC_TIMEOUT = float(os.environ.get("CVE_SYNC_TIMEOUT", "600"))


def attempt(label: str, command: list[str], check: bool = True) -> bool:
    """Runs one command, retrying a transport failure and giving up on anything else.

    The retry is for **the link**, not for the work: an `rsync` killed by a dropped connection returns 255, and an `ssh` that never answers is
    bounded by the timeout. A command that runs and fails - a compiler error, a test failure - returns its own code and is **not** retried, because
    retrying it would only make the gate slower at reporting the same thing.
    """
    for tries in range(1, SYNC_ATTEMPTS + 1):
        try:
            finished = subprocess.run(command, timeout=SYNC_TIMEOUT, check=False)
            code = finished.returncode
        except subprocess.TimeoutExpired:
            code = 124
        if code == 0:
            return True
        transport = code in (124, 255)
        print(f"  {label}: attempt {tries} of {SYNC_ATTEMPTS} failed with {code}" + (" (the link, not the tree)" if transport else ""), flush=True)
        if not transport or tries == SYNC_ATTEMPTS:
            if check:
                raise SystemExit(f"{label} failed after {tries} attempt(s)")
            return False
        if SYNC_RETRY_DELAY > 0:
            time.sleep(SYNC_RETRY_DELAY)
    return False


def sync(paths: list[str] | None = None) -> None:
    """Copies the workspace to the remote, deleting files that no longer exist here.

    `--delete` is safe because it only ever applies to the paths listed in SYNC, each of which is a directory the
    remote copy owns outright. The remote directory itself is never the target of a delete.

    **Every copy is retried**, because a dropped link is not a failing tree - see [`attempt`].
    """
    print(f"syncing to {REMOTE}:{REMOTE_DIR}", flush=True)
    # `--no-times` matters: `-a` preserves modification times, and a machine whose clock is a little behind the server then delivers a "new" file
    # with an old timestamp, so cargo reuses the artifact it already has and the gate silently tests the previous round's code. Writing the time as
    # now for the files that actually changed costs nothing and removes a whole class of false green.
    #
    # `-c` is what makes `--no-times` affordable. Without it rsync's quick check compares size and time, and a file stamped "now" by the last sync never
    # matches its source, so **every** file is transferred and re-stamped **every** time - which makes cargo rebuild the whole workspace, and made a
    # focused test take 9-19 minutes. With a checksum, an unchanged file is left alone (its stamp stands, its artifact stays fresh) and a changed one is
    # still stamped "now" - both halves of the rule hold at once.
    args = ["rsync", "-azc", "--no-times", "--delete"]
    # **The copy must notice a dead link as fast as a command does**: without `-e`, rsync's own ssh has no keepalives and a stalled transfer sits
    # there until the socket gives up - which is the shape of the hang this round's retry exists to prevent.
    args.extend(["-e", "ssh -o ServerAliveInterval=10 -o ServerAliveCountMax=3 -o ConnectTimeout=20"])
    for pattern in EXCLUDES:
        args.extend(["--exclude", pattern])
    for path in SYNC if paths is None else paths:
        source = ROOT / path
        if not source.exists():
            print(f"  skipping {path}: not here", flush=True)
            continue
        remote_path = f"{REMOTE_DIR}/{path}"
        # Directories are synced by their *contents* into a directory made first, so the far side ends up with
        # `crates/content` rather than `crates/crates/content`. This is the form that behaves the same on the old
        # rsync this machine ships and the new one on the server.
        if source.is_dir():
            attempt(f"mkdir {remote_path}", [*ssh_args(), f"mkdir -p {remote_path}"])
            source_arg = f"{source}/"
            destination = f"{REMOTE}:{remote_path}/"
        else:
            parent = pathlib.PurePosixPath(remote_path).parent
            attempt(f"mkdir {parent}", [*ssh_args(), f"mkdir -p {parent}"])
            source_arg = str(source)
            destination = f"{REMOTE}:{remote_path}"
        print(f"  {path}", flush=True)
        attempt(f"copy {path}", [*args, source_arg, destination])



# ---------------------------------------------------------------------------
# What changed, and what that makes it worth running
# ---------------------------------------------------------------------------


def changed_paths() -> set[str]:
    """Every path that differs from the pushed baseline, plus the working tree's own edits and untracked files.

    **The baseline is `origin/main`**, not `HEAD`: a commit that has not been pushed is a change as far as the gate is
    concerned, and CI will not see it until it is. A branch with no `origin/main` falls back to `HEAD`, so the check
    still answers - with a smaller set - instead of failing.
    """
    found: set[str] = set()
    for command in (
        ["git", "diff", "--name-only", "origin/main...HEAD"],
        ["git", "diff", "--name-only"],
        ["git", "diff", "--name-only", "--cached"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    ):
        finished = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        if finished.returncode != 0 and command[3] == "origin/main...HEAD":
            finished = subprocess.run(
                ["git", "diff", "--name-only", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
            )
        found.update(line.strip() for line in finished.stdout.splitlines() if line.strip())
    return found


def package_names() -> dict[str, tuple[str, set[str]]]:
    """Each workspace package: its directory, its name, and the workspace packages it depends on."""
    manifests = sorted((ROOT / "crates").glob("*/Cargo.toml"))
    named: dict[str, str] = {}
    texts: dict[str, str] = {}
    for manifest in manifests:
        text = manifest.read_text()
        texts[manifest.parent.name] = text
        for line in text.splitlines():
            if line.startswith("name = "):
                named[manifest.parent.name] = line.split("=", 1)[1].strip().strip('"')
                break
    known = set(named.values())
    packages: dict[str, tuple[str, set[str]]] = {}
    for directory, text in texts.items():
        mine = named[directory]
        dependencies = {
            other
            for other in known
            if other != mine and re.search(rf"^{re.escape(other)}\s*=", text, re.MULTILINE)
        }
        packages[directory] = (mine, dependencies)
    return packages


def changed_packages() -> list[str]:
    """The workspace packages a focused gate should build and test: what changed, and what depends on it.

    A change to `cdda-content` changes what `cdda-sim` compiles against, so the packages that **depend** on a changed one are
    included - their tests can fail from a change they did not make. The closure is computed from the manifests, once.
    """
    packages = package_names()
    direct = {
        path.split("/")[1]
        for path in changed_paths()
        if path.startswith("crates/") and len(path.split("/")) > 2 and path.split("/")[1] in packages
    }
    if not direct:
        return []
    selected = set(direct)
    selected_names = {packages[directory][0] for directory in direct}
    growing = True
    while growing:
        growing = False
        for directory, (name, dependencies) in packages.items():
            if directory in selected:
                continue
            if dependencies & selected_names:
                selected.add(directory)
                selected_names.add(name)
                growing = True
    return sorted(packages[directory][0] for directory in selected)



#: The test modules that cost **minutes** each, measured on the build machine: `eoc_programs` holds two cases that take
#: 462 s and 587 s on their own - they build a whole world before running a program - and the rest of the suite's 1,864
#: cases finish in 94 s together. A focused gate skips them and says so; `--slow` puts them back, and CI always runs
#: them, so they are covered without costing every edit ten minutes.
SLOW_MODULES = ("eoc_programs",)


def test_filters(changed: set[str]) -> list[str]:
    """The test modules a change is likely to have touched.

    A file `crates/sim/tests/sim_suite/foo.rs` is the `foo` module of that suite, and `crates/sim/src/foo.rs` is the `foo`
    module of the library - and in this repository a rule's tests live in a module of the rule's name. The filter is a
    **superset**, not a guess at one case: a wrong filter runs a few cases too many, never too few, because a change with
    no recognisable module (a `Cargo.toml`, a helper shared by everything) falls back to the whole package.
    """
    filters = set()
    for path in changed:
        parts = path.split("/")
        if len(parts) < 4 or parts[0] != "crates":
            continue
        if parts[2] != "src" and parts[2] != "tests" and len(parts) < 5:
            continue
        if not parts[-1].endswith(".rs"):
            continue
        name = parts[-1].removesuffix(".rs")
        if name in ("lib", "main", "mod"):
            continue
        filters.add(name)
    return sorted(filters)


def nextest_expression(filters: list[str], slow: bool) -> str:
    """A nextest filter expression: the touched modules, minus the slow ones unless they were asked for."""
    parts = [f"test(/{'|'.join(filters)}/)" if filters else "all()"]
    if not slow:
        parts.extend(f"not test(/{module}/)" for module in SLOW_MODULES)
    return " and ".join(parts)


def big_sync_needed(changed: set[str]) -> bool:
    """Whether the big directories have to be copied: never copied before, asked for, or a change inside them."""
    if any(
        path.startswith(("corpus/", "references/Cataclysm-DDA/data", "references/Cataclysm-DDA/src", "references/Cataclysm-DDA/doc"))
        for path in changed
    ):
        return True
    return ssh_quiet(f"test -f {BIG_STAMP}")[0] != 0


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


#: How long to wait between looks at a detached job, in seconds. The first look is short and the wait grows: a focused
#: job is over in a second, and a gate that sleeps its way through twenty looks costs more than the work it is watching.
#: A long job still sees a poll every couple of seconds, which is nothing against a build.
POLL_SECONDS = float(os.environ.get("CVE_POLL_SECONDS", "0.3"))
POLL_MAX_SECONDS = float(os.environ.get("CVE_POLL_MAX_SECONDS", "2.0"))


def ssh_quiet(command: str, timeout: float = 120) -> tuple[int, str]:
    """Runs one **short** command on the remote and returns its code and output.

    A short command is a different animal from a long one: it either answers in seconds or its session is dead, and a dead session costs one call rather than the run.
    """
    try:
        finished = subprocess.run(
            [*ssh_args(), command], capture_output=True, text=True, timeout=timeout, check=False
        )
        return finished.returncode, finished.stdout
    except subprocess.TimeoutExpired:
        return 124, ""


def run(command: str, label: str, jobs: int) -> int:
    """Runs one command on the remote and returns its exit code.

    **The job is detached and polled, not streamed.** Every long job used to be one ssh session whose stdout was the job's - and when the far side went quiet the session
    died, taking the result with it: the suite had finished and passed, but the gate reported the session's 255. A detached job writes its own log and its own exit code on
    the remote, and each poll is a short call that can fail on its own without losing either.
    """
    command = command.replace("{jobs}", str(jobs))
    print(f"\n=== {label} on {REMOTE}\n$ {command}\n", flush=True)
    slug = "".join(character if character.isalnum() else "-" for character in label.lower()).strip("-")
    script = f"/tmp/cve-gate-{slug}.sh"
    log = f"/tmp/cve-gate-{slug}.log"
    code_file = f"/tmp/cve-gate-{slug}.rc"
    # The script is carried as base64 because a command may hold quotes of its own - `find ... -exec touch {} +` does - and a quoted heredoc through two shells is a way to
    # get that wrong slowly.
    # **The exit code is written by the script**, not by the shell that launches it: the launcher is gone by the time the job ends, and a code nobody wrote is a job the poll
    # waits on until the gate gives up - which is exactly what the first version of this did.
    encoded = base64.b64encode(
        (
            f"cd {REMOTE_DIR} && export PATH=$HOME/.cargo/bin:$PATH\n"
            f"{command}\n"
            f"echo $? > {code_file}\n"
        ).encode()
    ).decode()
    launch = (
        f"echo {encoded} | base64 -d > {script} && rm -f {code_file} && "
        f"( setsid nohup bash {script} > {log} 2>&1 < /dev/null & ) ; echo launched"
    )
    attempt(f"launch {label}", [*ssh_args(), launch])
    started = time.time()
    code = None
    wait = POLL_SECONDS
    while time.time() - started < GATE_TIMEOUT:
        time.sleep(wait)
        wait = min(wait * 2, POLL_MAX_SECONDS)
        answer, output = ssh_quiet(f"cat {code_file} 2>/dev/null")
        if answer == 0 and output.strip():
            try:
                code = int(output.strip().splitlines()[0])
            except ValueError:
                code = 125
            break
        # A poll that cannot answer at all is not the job failing; the next poll asks again.
    if code is None:
        print(f"\n=== {label}: gave up after {GATE_TIMEOUT}s", flush=True)
        code = 124
    # The log is printed once the job is over, so the reader sees what it said without a session having to survive it.
    attempt(f"read {label} log", [*ssh_args(), f"cat {log} 2>/dev/null"], check=False)
    if code != 0:
        print(f"\n=== {label} failed with {code}", flush=True)
    return code



def fmt_check(changed: set[str]) -> int:
    """Checks formatting, over the files that changed when there are some.

    `cargo fmt --all -- --check` is five seconds on this checkout because it walks every target; `rustfmt --check` over
    the handful of files a round touched is a tenth of that. A change to a manifest or a new file falls back to the whole
    workspace, because formatting rules can move with the edition and a file that is not in the tree yet is not formatted.
    """
    sources = sorted(path for path in changed if path.endswith(".rs") and (ROOT / path).exists())
    manifests = [path for path in changed if path.endswith("Cargo.toml") or path.endswith("Cargo.lock")]
    if manifests or not sources:
        return subprocess.run(["cargo", "fmt", "--all", "--", "--check"], cwd=ROOT, check=False).returncode
    if len(sources) > 40:
        return subprocess.run(["cargo", "fmt", "--all", "--", "--check"], cwd=ROOT, check=False).returncode
    return subprocess.run(
        ["rustfmt", "--edition", "2024", "--check", *sources], cwd=ROOT, check=False
    ).returncode


def main() -> int:
    global REMOTE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changed", action="store_true", help="only the packages the working tree changed (the default)")
    parser.add_argument("--all", action="store_true", help="the whole suite and the whole workspace's clippy")
    parser.add_argument("--slow", action="store_true", help="include the modules that cost minutes (see SLOW_MODULES)")
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

    named = options.tests or options.docs or options.clippy or options.oracle or options.fmt or options.sync
    # **The default is the focused gate**, because it is the one a person runs dozens of times a day; `--all` is the one
    # that runs before a push that matters. Everything named explicitly behaves as before, over the whole workspace.
    focused = options.changed or (not options.all and not named)

    changed = changed_paths()
    packages = changed_packages() if focused else []
    wants_big = options.all or (focused and ("cdda-content" in packages or big_sync_needed(changed)))

    sync(SYNC + (BIG_SYNC if wants_big else []))
    if wants_big:
        subprocess.run([*ssh_args(), f"date > {BIG_STAMP}"], capture_output=True, check=False)
    if options.sync and not named and not options.all and not options.changed:
        return 0

    jobs = options.jobs
    if jobs <= 0:
        cores = subprocess.run([*ssh_args(), "nproc"], capture_output=True, text=True, check=True)
        jobs = max(1, min(6, int(cores.stdout.strip()) - 1))

    failed: list[str] = []
    # Everything that reads **this** checkout runs here: formatting, the documents, the dead-rule sweep and the oracle
    # patch. They cost seconds, they need no target directory, and moving them to the build machine would only add a
    # round trip - the documents in particular read the reference sources, which a focused run no longer copies there.
    if not options.sync or options.fmt:
        # A full run checks the whole workspace; a focused one checks the files that changed.
        code = fmt_check(changed) if not options.all else subprocess.run(
            ["cargo", "fmt", "--all", "--", "--check"], cwd=ROOT, check=False
        ).returncode
        if code != 0:
            failed.append("formatting")
    # **The document check is about the documents.** Its citations are line numbers into the reference, which the reference
    # does not change, so a change under `crates/` cannot break it - and it costs four seconds, which is a quarter of a
    # focused gate. It runs when a document changed, and on every full run.
    wants_docs = options.all or options.docs or any(
        path.startswith(("docs/", "references/", "tools/check_docs")) for path in changed
    )
    if (not options.sync or options.docs) and wants_docs:
        if subprocess.run([sys.executable, "tools/check_docs.py"], capture_output=True).returncode != 0:
            subprocess.run([sys.executable, "tools/check_docs.py"], check=False)
            failed.append("docs")
    # The dead-rule sweep reads the sources, so any code change can make a rule dead; it is under a second.
    if (not options.sync or options.docs) and (options.all or options.docs or any(path.startswith("crates/") for path in changed)):
        if subprocess.run([sys.executable, "tools/check_dead_rules.py"], capture_output=True).returncode != 0:
            subprocess.run([sys.executable, "tools/check_dead_rules.py"], check=False)
            failed.append("dead rules")
    # **The oracle's patch check is twelve seconds**, and it can only change when the oracle or the reference does. On a
    # focused run over the port's own code it is the single most expensive thing in the gate and answers a question
    # nobody had.
    wants_oracle = options.all or options.oracle or any(
        path.startswith(("oracle/", "references/")) for path in changed
    )
    if (not options.sync or options.oracle) and wants_oracle:
        if subprocess.run([sys.executable, "oracle/apply_harness_patches.py", "--verify"], capture_output=True).returncode != 0:
            failed.append("oracle")

    if options.all or options.tests:
        # **The full suite is the whole workspace minus the slow modules.** `--slow` is what puts them back, and CI runs
        # everything regardless: the focused gate is for the edit loop, this is for before a push, and neither is the
        # place to spend ten minutes twice.
        expression = nextest_expression([], options.slow)
        if run(f"cargo nextest run --workspace -E '{expression}' --no-fail-fast", "the suite", jobs) != 0:
            failed.append("tests")
    elif focused:
        if packages:
            selector = " ".join(f"-p {name}" for name in packages)
            filters = test_filters(changed)
            expression = nextest_expression(filters, options.slow)
            label = f"the tests of {', '.join(packages)}" + (f" matching {filters}" if filters else "")
            if run(f"cargo nextest run {selector} -E '{expression}' --no-fail-fast", label, jobs) != 0:
                failed.append("tests")
        else:
            print("\nnothing under crates/ changed: no tests to run", flush=True)

    if options.all:
        # **Touch every crate root first, or cargo's cache eats the gate.** `cargo clippy -D warnings` only fails on
        # diagnostics it *emits*, and a crate whose fingerprint has not changed is not re-checked. A full run therefore
        # forces the diagnostics out; a focused run does not need to, because the crates it looks at have just changed.
        run("find crates -name '*.rs' -exec touch {} +", "touch every source file on the build machine", jobs)
        if run("cargo clippy --workspace --all-targets -- -D warnings", "clippy", jobs) != 0:
            failed.append("clippy")
    elif options.clippy or focused:
        if packages:
            selector = " ".join(f"-p {name}" for name in packages)
            if run(f"cargo clippy {selector} --all-targets -- -D warnings", "clippy of the changed packages", jobs) != 0:
                failed.append("clippy")
        elif options.clippy:
            if run("cargo clippy --workspace --all-targets -- -D warnings", "clippy", jobs) != 0:
                failed.append("clippy")

    print()
    if failed:
        print(f"FAILED: {', '.join(failed)}")
        return 1
    print(f"all green on {REMOTE} ({jobs} jobs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
