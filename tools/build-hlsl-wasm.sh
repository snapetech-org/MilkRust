#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-$repo_root/pkg}"
emxx="${EMXX:-em++}"
if ! command -v "$emxx" >/dev/null 2>&1; then
  echo 'Emscripten 3.1.64 is required on the private build runner (EMXX may name its em++).' >&2
  exit 127
fi
if ! "$emxx" --version | head -n 1 | grep -F '3.1.64' >/dev/null; then
  echo 'The HLSL browser compiler is pinned to Emscripten 3.1.64.' >&2
  exit 1
fi
mkdir -p "$out_dir"
vendor="$repo_root/crates/milkrust-hlsl/vendor/hlslparser"
sources=("$repo_root/crates/milkrust-hlsl/src/bridge.cpp")
for unit in Engine CodeWriter HLSLTokenizer HLSLParser HLSLTree GLSLGenerator; do
  sources+=("$vendor/src/$unit.cpp")
done
"$emxx" "${sources[@]}" -I "$vendor/src" -std=c++11 -O2 -fexceptions \
  -sDISABLE_EXCEPTION_CATCHING=0 -sMODULARIZE=1 -sEXPORT_ES6=1 \
  -sEXPORT_NAME=createMilkRustHlslCompiler -sENVIRONMENT=web,worker,node \
  -sFILESYSTEM=0 -sALLOW_MEMORY_GROWTH=1 -sINITIAL_MEMORY=16777216 \
  -sMAXIMUM_MEMORY=67108864 -sSTACK_SIZE=2097152 -sABORTING_MALLOC=0 \
  '-sEXPORTED_FUNCTIONS=["_milkrust_hlsl_compile","_malloc","_free"]' \
  '-sEXPORTED_RUNTIME_METHODS=["UTF8ToString","HEAPU8"]' \
  -o "$out_dir/milkrust_hlsl.js"
cp "$vendor/LICENSE" "$out_dir/HLSLParser-LICENSE.txt"
cp "$vendor/UPSTREAM.json" "$out_dir/HLSLParser-UPSTREAM.json"
