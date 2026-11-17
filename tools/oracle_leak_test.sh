#!/usr/bin/env bash
# Proves the leak guarantee: **a build cannot outlive the call that started it.**
#
# Round 169 left two compiler trees running on the build machine, each with its parent re-parented to init, because a killed wrapper and an aborted tool call both
# end the *client* and not the *build*. `tools/oracle_sync.sh` answers that with three layers; this script attacks the hardest one.
#
#   tools/oracle_leak_test.sh
#
# The test starts a real build, waits until the compiler is actually running on the far side, then **SIGKILLs the wrapper's whole process group** - which is the
# aborted-tool-call case, where no `trap` on the wrapper can run at all. Only the watchdog on the server can save it, so a pass here is the proof the user asked
# for: kill the client, and the far side goes quiet by itself.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE="${ORACLE_REMOTE:-cyc@elitedesk}"
DIR="${ORACLE_DIR:-cve-test}"
PATTERN="cmake --build build --target cataclysm"

running() {
    ssh -o BatchMode=yes -o ConnectTimeout=10 "$REMOTE" \
        "ps -eo args | grep -F '$PATTERN' | grep -v grep | wc -l" 2>/dev/null || echo 0
}

cd "$ROOT"

# **Give the build something to do.** A tree that is already built makes `cmake --build` a no-op, and a no-op cannot be attacked: touching one source on the far side
# recompiles that file and relinks, which is a real compiler running for a minute or two. The file's contents do not change.
ssh -o BatchMode=yes "$REMOTE" "touch $DIR/oracle/src/src/harness.cpp" 2>/dev/null || true

echo "leak test: starting a build in its own session"
# **A new session, so the kill below is a kill of the wrapper alone.** macOS has no `setsid(1)`, so the session is made with Perl's `setsid(2)` and the wrapper is
# `exec`ed into it - the pid that comes back is the one to kill.
perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' ./tools/oracle_sync.sh --dump-vehicle-drive /tmp/leak-test.json >/tmp/leak-test.log 2>&1 &
wrapper=$!

for _ in $(seq 1 120); do
    if [ "$(running)" -gt 0 ]; then break; fi
    if ! kill -0 "$wrapper" 2>/dev/null; then
        echo "leak test: the build finished before it could be attacked" >&2
        exit 2
    fi
    sleep 2
done

if [ "$(running)" -eq 0 ]; then
    echo "leak test: no compiler ever appeared; nothing was proven" >&2
    exit 2
fi

echo "leak test: compiler is running; SIGKILLing the wrapper's whole group (the abort case)"
kill -KILL -- -"$wrapper" 2>/dev/null || true

start=$(date +%s)
for _ in $(seq 1 60); do
    if [ "$(running)" -eq 0 ]; then
        echo "leak test: PASS - the far side stopped by itself after $(( $(date +%s) - start ))s"
        exit 0
    fi
    sleep 1
done

echo "leak test: FAIL - the compiler is still running $(( $(date +%s) - start ))s after the client died" >&2
ssh -o BatchMode=yes "$REMOTE" "ps -eo pid,ppid,etimes,args | grep -F '$PATTERN' | grep -v grep | cut -c1-120" >&2 || true
exit 1
