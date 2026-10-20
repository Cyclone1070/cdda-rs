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
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent


def binaries() -> list[pathlib.Path]:
    listing = subprocess.run(
        ["cargo", "test", "--workspace", "--no-run", "--message-format=json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
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


def run(binary: pathlib.Path) -> tuple[str, int, float, str]:
    began = time.monotonic()
    finished = subprocess.run(
        [str(binary), "--test-threads=1"], capture_output=True, text=True, check=False
    )
    return (
        binary.name,
        finished.returncode,
        time.monotonic() - began,
        finished.stdout + finished.stderr,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--filter", default="")
    arguments = parser.parse_args()

    selected = [path for path in binaries() if arguments.filter in path.name]
    print(f"running {len(selected)} test binaries, {arguments.jobs} at a time")

    started = time.monotonic()
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
        for result in pool.map(run, selected):
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
        print(f"{seconds:6.1f}s  {status}  {name:28} {cases.strip()}")

    failures = [result for result in results if result[1] != 0]
    for name, _, _, output in failures:
        print(f"\n=== {name}")
        print("\n".join(output.splitlines()[-25:]))

    print(f"\ntotal {elapsed:.1f}s, {len(failures)} failing binaries")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
