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
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE="cyc@elitedesk"
DIR="cve-test"

ssh "$REMOTE" "mkdir -p $DIR/oracle/src"
rsync -a --no-times --delete --exclude 'build' "$ROOT/oracle/src/" "$REMOTE:$DIR/oracle/src/"

ssh "$REMOTE" "set -e
cd $DIR/oracle
# gcc 16 added -Wsfinae-incomplete, which the pinned tree trips; the oracle only has to run, not to be warning-clean.
sed -i 's/\"-Werror -Wall -Wextra/\"-Wall -Wextra/' src/CMakeLists.txt
[ -d src/.git ] || ( cd src && git init -q . && git add -A -f >/dev/null && git -c user.name=o -c user.email=o@o commit -qm oracle && git tag -f 0.I-1 )
cmake -S src -B build -G Ninja -DTILES=OFF -DCURSES=ON -DSOUND=OFF -DLOCALIZE=OFF -DTESTS=OFF -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_FLAGS=-DCATA_HARNESS > /tmp/oracle-cmake.log 2>&1
cmake --build build --target cataclysm -j 5 > /tmp/oracle-build.log 2>&1
echo oracle built"

if [ "$#" -gt 0 ]; then
    ssh "$REMOTE" "cd $DIR && TERM=xterm ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir \$PWD/references/Cataclysm-DDA/data/ --world harness-fixed --seed 12345 $*"
fi
