#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

runner="scripts/run-bug-council-all-phases.sh"
failed=0

require_literal() {
  local literal="$1"
  local file="$2"
  if rg -q --fixed-strings "$literal" "$file"; then
    printf 'PASS all-phases registration: %s\n' "$literal"
  else
    printf 'FAIL all-phases registration missing: %s\n' "$literal" >&2
    failed=1
  fi
}

require_literal "check-remediation-baseline.sh" "$runner"
require_literal "Focused remediation gates" "$runner"
require_literal "cargo test --workspace" "$runner"
require_literal "npm run test:web" "$runner"
require_literal "npm run test:apps" "$runner"
require_literal "npm run pack:validate" "$runner"
require_literal "npm run content:validate" "$runner"
require_literal "not proof of no bugs" "$runner"

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

printf '\nMilkRust all-phases runner is registered.\n'
