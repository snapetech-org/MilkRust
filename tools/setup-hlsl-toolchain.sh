#!/usr/bin/env bash
set -euo pipefail
: "${RUNNER_TEMP:?A private ephemeral JIT runner is required}"
: "${GITHUB_RUN_ID:?A private JIT run ID is required}"
: "${GITHUB_JOB:?A private JIT job is required}"
: "${GITHUB_PATH:?GitHub Actions path file is required}"
: "${GITHUB_ENV:?GitHub Actions environment file is required}"
if [[ "${RUNNER_ENVIRONMENT:-}" != self-hosted ]]; then
  echo 'HLSL toolchain installation is restricted to the private runner.' >&2
  exit 1
fi
sdk="$RUNNER_TEMP/milkrust-emsdk-$GITHUB_RUN_ID-$GITHUB_JOB"
revision=0b3bcbc3b005cbb811d48e497eabcc6846d43001
if [[ -e "$sdk" || -L "$sdk" ]]; then
  echo 'Refusing to overwrite a toolchain directory.' >&2
  exit 1
fi
git -c protocol.file.allow=never clone --no-checkout https://github.com/emscripten-core/emsdk.git "$sdk"
git -C "$sdk" checkout --detach "$revision"
[[ "$(git -C "$sdk" rev-parse HEAD)" == "$revision" ]]
"$sdk/emsdk" install 3.1.64
"$sdk/emsdk" activate 3.1.64
source "$sdk/emsdk_env.sh" >/dev/null
printf '%s\n' "$sdk/upstream/emscripten" "$(dirname "$EMSDK_NODE")" >> "$GITHUB_PATH"
printf 'EMSDK=%s\nEMXX=%s\n' "$sdk" "$sdk/upstream/emscripten/em++" >> "$GITHUB_ENV"
