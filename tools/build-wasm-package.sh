#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-$repo_root/pkg}"

bash "$repo_root/tools/build-hlsl-wasm.sh" "$out_dir"

cargo build \
  --manifest-path "$repo_root/Cargo.toml" \
  -p milkrust-wasm \
  --target wasm32-unknown-unknown \
  --release

wasm_bindgen_bin="${WASM_BINDGEN:-}"
if [[ -z "$wasm_bindgen_bin" ]]; then
  if command -v wasm-bindgen >/dev/null 2>&1; then
    wasm_bindgen_bin="wasm-bindgen"
  elif [[ -x "$HOME/.cargo/bin/wasm-bindgen" ]]; then
    wasm_bindgen_bin="$HOME/.cargo/bin/wasm-bindgen"
  fi
fi

if [[ -z "$wasm_bindgen_bin" ]]; then
  echo "wasm-bindgen CLI is required to generate browser package files." >&2
  echo "Install it with: cargo install wasm-bindgen-cli" >&2
  exit 127
fi

mkdir -p "$out_dir"
"$wasm_bindgen_bin" \
  "$repo_root/target/wasm32-unknown-unknown/release/milkrust_wasm.wasm" \
  --target web \
  --out-dir "$out_dir"

cp "$repo_root/crates/milkrust-wasm/texture_decoder_worker.js" \
  "$out_dir/milkrust_texture_decoder.js"
