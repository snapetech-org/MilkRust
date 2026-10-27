#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0

require_file() {
  local path="$1"
  if [[ -f "$path" ]]; then
    printf 'PASS negative-space: %s exists\n' "$path"
  else
    printf 'FAIL negative-space: %s missing\n' "$path" >&2
    failed=1
  fi
}

require_pattern() {
  local pattern="$1"
  local path="$2"
  local label="$3"
  if rg -n --pcre2 --hidden --glob '!.git/**' --glob '!node_modules/**' "$pattern" "$path" >/dev/null; then
    printf 'PASS negative-space: %s\n' "$label"
  else
    printf 'FAIL negative-space: %s\n' "$label" >&2
    failed=1
  fi
}

require_file "scripts/run-bug-council-all-phases.sh"
require_file "scripts/check-remediation-baseline.sh"

require_pattern '"check:council"' "package.json" "package script runs all phases"
require_pattern 'unsafe_code = "forbid"' "Cargo.toml" "Rust unsafe posture remains explicit"
require_pattern 'content:validate' "package.json" "content catalog validation remains registered"
require_pattern 'Focused remediation gates' "scripts/run-bug-council-all-phases.sh" "runner is limited to focused remediation and regression checks"

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

printf '\nAll negative-space gate checks passed.\n'
