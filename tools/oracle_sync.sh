#!/usr/bin/env bash
# Keeps the oracle build on the server in step with the working copy here, and records dumps there.
#
# Every oracle compile and every dump runs on `cyc@elitedesk`; nothing heavy runs on the Mac. The server gets the **already-patched** source tree by rsync rather
# than the patch plus a copy of the reference: `oracle/apply_harness_patches.py --verify` already proves that tree is the pinned checkout plus exactly one patch, so
# the patch is a statement about the tree, not a second way to build it. (Applying the patch remotely drifted twice - a stale copy, then a hunk that applied to the
# wrong place - and each drift cost a ten-minute build that failed.)
#
#   tools/oracle_sync.sh                     sync the source and build
#   tools/oracle_sync.sh <flag> [args...]    sync, build, then run the oracle with those arguments
#
# Paths passed as arguments are resolved on the server, so use absolute server paths or relative ones from ~/cve-test.
#
# ## A build must never outlive the call that started it
#
# Round 169 leaked two `cmake --build` trees onto the build machine: a killed wrapper and an aborted tool call both left the remote compiler running with its parent
# re-parented to init, and it kept five cores busy for minutes after nobody was waiting for it. **A compile that no one is waiting for is a leak**, so three layers
# make one impossible:
#
#   1. **The remote watchdog.** The build runs in its own process group; a watcher polls the SSH session that started it and, the moment that session is gone - the
#      client was killed, the tool call was aborted, the link dropped - tears the whole group down. This is the layer that covers the cases a local `trap` cannot.
#   2. **The local trap.** `EXIT`, `INT`, `TERM` and `HUP` all run the reaper, so an ordinary Ctrl-C or a killed job does not wait for the watchdog's poll.
#   3. **The ceiling and the preflight.** `timeout` bounds any single build, and every run reaps leftovers before starting, so a machine-level accident (an OOM kill
#      of the client, a yanked network cable) cannot leave a build behind for longer than the ceiling.
#
# `BUILD_TIMEOUT` and `WATCH_POLL` exist so the layer can be tested: `tools/oracle_leak_test.sh` kills a wrapper on purpose and proves the far side goes quiet.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE="${ORACLE_REMOTE:-cyc@elitedesk}"
DIR="${ORACLE_DIR:-cve-test}"
BUILD_TIMEOUT="${BUILD_TIMEOUT:-1800}"
WATCH_POLL="${WATCH_POLL:-1}"

# The exact command line of a build this script started, so the reaper can never touch anything else on the machine.
BUILD_PATTERN="cmake --build build --target cataclysm"

reap() {
    ssh -o BatchMode=yes -o ConnectTimeout=10 "$REMOTE" \
        "pkill -TERM -f '$BUILD_PATTERN' 2>/dev/null; sleep 2; pkill -KILL -f '$BUILD_PATTERN' 2>/dev/null" \
        >/dev/null 2>&1 || true
}

# Layer 2: any way this script ends, the far side stops building.
trap reap EXIT INT TERM HUP

# Layer 3a: nothing of ours may already be running. A build whose parent is gone is a leak by definition, and this run is not going to wait for it.
ssh -o BatchMode=yes -o ConnectTimeout=10 "$REMOTE" \
    "ps -eo pid,ppid,args | grep -F '$BUILD_PATTERN' | grep -v grep | awk '\$2 == 1 { print \$1 }' | xargs -r kill -TERM 2>/dev/null || true" \
    >/dev/null 2>&1 || true

ssh -o BatchMode=yes "$REMOTE" "mkdir -p $DIR/oracle/src"
rsync -a --no-times --delete --exclude build "$ROOT/oracle/src/" "$REMOTE:$DIR/oracle/src/"

# Layers 1 and 3b: sent as a script rather than a quoted command line, so the far side reads as a script does.
read -r -d '' REMOTE_BUILD <<'REMOTE_EOF' || true
set -euo pipefail
cd __DIR__/oracle
# gcc 16 added -Wsfinae-incomplete, which the pinned tree trips; the oracle only has to run, not to be warning-clean.
sed -i 's/"-Werror -Wall -Wextra/"-Wall -Wextra/' src/CMakeLists.txt
[ -d src/.git ] || ( cd src && git init -q . && git add -A -f >/dev/null && git -c user.name=o -c user.email=o@o commit -qm oracle && git tag -f 0.I-1 )
cmake -S src -B build -G Ninja -DTILES=OFF -DCURSES=ON -DSOUND=OFF -DLOCALIZE=OFF -DTESTS=OFF -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS=-DCATA_HARNESS > /tmp/oracle-cmake.log 2>&1

# The session that started this build. When it goes away the client is gone, and nothing is waiting for the compiler any more.
session=$PPID

# **Job control puts the build in its own process group**, so the teardown below reaches the compiler and everything under it, and nothing else on the machine.
set -m
timeout --kill-after=30 __BUILD_TIMEOUT__ cmake --build build --target cataclysm -j 5 > /tmp/oracle-build.log 2>&1 &
build=$!
set +m

# The watchdog gets its own session, because the hangup that kills everything else when the SSH channel closes would kill it too. It watches the session that started
# the build and tears the build's group down when that session is gone.
setsid bash -c '
trap "" HUP
while kill -0 "$1" 2>/dev/null; do sleep "$3"; done
echo "oracle: the session that started this build is gone; stopping the compiler" >&2
kill -TERM -- "-$2" 2>/dev/null || true
sleep 3
kill -KILL -- "-$2" 2>/dev/null || true
' _ "$session" "$build" __WATCH_POLL__ > /tmp/oracle-watchdog.log 2>&1 &
guard=$!

rc=0
wait "$build" || rc=$?
kill "$guard" 2>/dev/null || true
if [ "$rc" != 0 ]; then
    echo "oracle: build failed, see /tmp/oracle-build.log" >&2
    exit "$rc"
fi
echo oracle built
REMOTE_EOF

REMOTE_BUILD=${REMOTE_BUILD//__DIR__/$DIR}
REMOTE_BUILD=${REMOTE_BUILD//__BUILD_TIMEOUT__/$BUILD_TIMEOUT}
REMOTE_BUILD=${REMOTE_BUILD//__WATCH_POLL__/$WATCH_POLL}
printf '%s\n' "$REMOTE_BUILD" | ssh -o BatchMode=yes "$REMOTE" "bash -s"


if [ "$#" -gt 0 ]; then
    ssh -o BatchMode=yes "$REMOTE" "cd $DIR && TERM=xterm ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir \$PWD/references/Cataclysm-DDA/data/ --world harness-fixed --seed 12345 $*"
fi
