#!/usr/bin/env bash
# Builds the instrumented reference (the "oracle").
#
#   1. copies the pinned checkout into oracle/src (disposable: the build writes
#      generated headers into the source tree, so it must not be the pinned one)
#   2. adds the harness translation units and applies the source patches
#   3. configures and builds the curses-only binary with CATA_HARNESS defined
#
# Usage: oracle/build.sh [--clean]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$ROOT/references/Cataclysm-DDA"
ORACLE="$ROOT/oracle"
PINNED_COMMIT="7b2efa5cea38e4d4d97dd0e63b28b9148623da59"

if [ "${1:-}" = "--clean" ]; then
    rm -rf "$ORACLE/src" "$ORACLE/build"
fi

# The oracle is only comparable against the pinned revision.
actual="$(git -C "$REF" rev-parse HEAD)"
if [ "$actual" != "$PINNED_COMMIT" ]; then
    echo "reference is at $actual, expected $PINNED_COMMIT" >&2
    exit 1
fi
if [ -n "$(git -C "$REF" status --porcelain)" ]; then
    echo "reference checkout is dirty; refusing to build from it" >&2
    exit 1
fi

# --delete so the copy is exactly the pinned revision plus the patches.
rsync -a --delete --exclude=.git --exclude=lang "$REF/" "$ORACLE/src/"

python3 "$ORACLE/apply_harness_patches.py"

# ncurses on macOS: the wide-character declarations need _XOPEN_SOURCE_EXTENDED,
# which the project's Makefile defines (Makefile:628) but its CMake does not.
# Without it imtui-impl-ncurses.cpp fails with "use of undeclared identifier
# 'waddwstr'".
cmake -S "$ORACLE/src" -B "$ORACLE/build" -G Ninja \
    -DTILES=OFF -DCURSES=ON -DSOUND=OFF -DLOCALIZE=OFF -DTESTS=OFF \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_CXX_FLAGS="-D_XOPEN_SOURCE_EXTENDED -DCATA_HARNESS" \
    -DCURSES_INCLUDE_PATH=/usr/local/opt/ncurses/include \
    -DCURSES_LIBRARY=/usr/local/opt/ncurses/lib/libncursesw.dylib

cmake --build "$ORACLE/build" --target cataclysm -j "$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

echo "oracle: $ORACLE/build/src/cataclysm"
