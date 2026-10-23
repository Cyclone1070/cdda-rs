#!/usr/bin/env python3
"""Runs every test binary concurrently and reports what each one cost.

`cargo test` runs one test binary at a time, so the corpus gates cost the *sum*
of their runtimes rather than the slowest. They are independent - each loads its
own corpus - so they can all run at once. This builds once, then runs the binaries
in parallel and prints a table, because a gate whose cost is invisible is a gate
nobody keeps fast.

Usage: python3 tools/test_fast.py [--jobs N] [--filter substring]
"""

import argparse
import concurrent.futures
import json
import os
import pathlib
import resource
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Cargo needs a writable home: it unpacks sources there, and a sandbox that forbids
# the account's own ~/.cargo turns "no writes allowed" into "cannot download a crate",
# which reads like a broken build. The repository keeps its own, so use it by default.
WORKSPACE_CARGO_HOME = ROOT / ".cargo-home"


def cargo_env() -> dict[str, str]:
    """The environment cargo should run in: the workspace's home unless one is set."""
    env = dict(os.environ)
    if "CARGO_HOME" not in env and WORKSPACE_CARGO_HOME.is_dir():
        env["CARGO_HOME"] = str(WORKSPACE_CARGO_HOME)
    return env


def binaries() -> list[pathlib.Path]:
    listing = subprocess.run(
        ["cargo", "test", "--workspace", "--no-run", "--message-format=json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        env=cargo_env(),
    )
    found = []
    for line in listing.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("executable")
            and message.get("profile", {}).get("test")
        ):
            found.append(pathlib.Path(message["executable"]))
    return found


def children_cpu() -> float:
    """Processor time spent by finished child processes, in seconds.

    The wall clock alone cannot tell a slow gate from a busy machine, so the run also reports how much
    processor time the binaries actually used.
    """
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def run(binary: pathlib.Path, threads: int) -> tuple[str, int, float, str]:
    began = time.monotonic()
    command = [str(binary)] if threads == 0 else [str(binary), f"--test-threads={threads}"]
    finished = subprocess.run(
        command, capture_output=True, text=True, check=False
    )
    return (
        binary.name,
        finished.returncode,
        time.monotonic() - began,
        finished.stdout + finished.stderr,
    )


def default_jobs() -> int:
    """How many binaries to run at once.

    Four, or half the cores when there are fewer. Measured on a twelve-core machine: four at a time and
    eight at a time both finish in 33 seconds, and eight leaves the load average at 16 instead of 14 -
    the same speed for more pressure on everything else running. Cases inside a binary run in parallel
    by default, so the machine is busy enough already.
    """
    cores = os.cpu_count() or 4
    return max(1, min(4, cores // 2))


def load_average() -> float:
    try:
        return os.getloadavg()[0]
    except OSError:
        return 0.0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jobs", type=int, default=default_jobs())
    parser.add_argument("--filter", default="")
    parser.add_argument(
        "--threads",
        type=int,
        default=0,
        help="cases to run at once inside each binary; 0 lets the test harness decide",
    )
    arguments = parser.parse_args()

    selected = [path for path in binaries() if arguments.filter in path.name]
    print(f"running {len(selected)} test binaries, {arguments.jobs} at a time")

    started = time.monotonic()
    cpu_before = children_cpu()
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
        for result in pool.map(lambda path: run(path, arguments.threads), selected):
            results.append(result)
    elapsed = time.monotonic() - started

    for name, code, seconds, output in sorted(results, key=lambda r: -r[2]):
        cases = next(
            (
                line.rsplit(";", 1)[0]
                for line in output.splitlines()
                if line.startswith("test result:")
            ),
            "no result line",
        )
        status = "ok  " if code == 0 else "FAIL"
        # A gate this slow is one to watch; the marker is here so the table can be read at a glance.
        slow = " *" if seconds >= 10.0 else "  "
        print(f"{seconds:6.1f}s  {status}  {name:28} {cases.strip()}{slow}")

    failures = [result for result in results if result[1] != 0]
    for name, _, _, output in failures:
        print(f"\n=== {name}")
        print("\n".join(output.splitlines()[-25:]))

    cpu_total = children_cpu() - cpu_before
    cores = os.cpu_count() or 1
    print(
        f"\ntotal {elapsed:.1f}s wall, {cpu_total:.1f}s cpu over {len(results)} binaries, "
        f"{arguments.jobs} at a time on {cores} cores, load {load_average():.2f}"
    )
    if cpu_total > 0 and elapsed > 0:
        # How many workers were busy on average. Well below the number asked for means either there was
        # little to run or the machine was busy elsewhere - not that the gates got slower.
        busy = cpu_total / elapsed
        print(f"kept {busy:.1f} of {arguments.jobs} workers busy on average")
    print(f"{len(failures)} failing binaries")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
