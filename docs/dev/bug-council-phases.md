# MilkRust Remediation Regression Tracker

Started: 2026-05-07.

This tracker covers confirmed findings and their regression coverage. Broad source, dependency, and configuration scans use the repository's existing Semgrep and Trivy workflows.

| # | Name | Status | Owner | Exit criteria |
| --- | --- | --- | --- | --- |
| 1 | Remediation process docs | Done | (agent) | Severity, sibling-search, negative-space, behavior-pinning, and phase docs describe the retained regression process. |
| 2 | Negative-space regression gate | Done | (agent) | `scripts/check-council-negative-space.sh` asserts required project and test wiring without running candidate scanners. |
| 3 | Product regression suite | Done | (agent) | Rust, web, app smoke, pack, and content checks are registered in `scripts/run-bug-council-all-phases.sh`. |
| 4 | All-regressions runner | Done | (agent) | `npm run check:council` runs the remediation gates and product regression checks. |

## Resume rule

1. Read recent product and fix commits plus the remediation ledger.
2. Run the focused regression command for the change.
3. Update this tracker when a regression phase changes.
