# Focused Regression Gate

This gate checks required project and regression wiring. Broad source, dependency, and configuration scans use Semgrep and Trivy.

| Boundary | Required anchor |
| --- | --- |
| All-phases runner | `scripts/run-bug-council-all-phases.sh` |
| Package entrypoint | `check:council` in `package.json` |
| Remediation baseline | `scripts/check-remediation-baseline.sh` |
| Rust unsafe posture | `unsafe_code = "forbid"` in `Cargo.toml` |
| Content catalog validation | `content:validate` in `package.json` |

Every new trust boundary should add a row here and a matching assertion in `scripts/check-council-negative-space.sh`.
