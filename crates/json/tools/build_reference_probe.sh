#!/bin/sh
# Builds the reference probe used by tests/reference_parity.rs.
#
# Compiles tools/reference_probe.cpp against the flatbuffers sources vendored in
# the pinned reference checkout. Nothing under references/ is written to: the
# binary lands in target/json-probe/reference_probe.
#
# Usage: crates/json/tools/build_reference_probe.sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
workspace=$(cd "$here/../../.." && pwd)
flatbuffers="$workspace/references/Cataclysm-DDA/src/third-party"
output="$workspace/target/json-probe"

if [ ! -d "$flatbuffers/flatbuffers" ]; then
    echo "vendored flatbuffers not found under $flatbuffers" >&2
    exit 1
fi

mkdir -p "$output"
exec "${CXX:-c++}" -std=c++17 -O2 \
    -I "$flatbuffers" \
    -I "$workspace/references/Cataclysm-DDA/src" \
    "$here/reference_probe.cpp" \
    "$flatbuffers/flatbuffers/idl_parser.cpp" \
    "$flatbuffers/flatbuffers/util.cpp" \
    -o "$output/reference_probe"
