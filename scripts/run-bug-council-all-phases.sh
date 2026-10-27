#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

printf '==> Focused remediation gates\n'
scripts/check-bug-council-all-phases.sh
scripts/check-remediation-baseline.sh

printf '\n==> Product verification\n'
cargo test --workspace
npm run test:web
npm run test:apps
npm run pack:validate
npm run content:validate

printf '\nAll focused remediation and product regression checks passed.\n'
