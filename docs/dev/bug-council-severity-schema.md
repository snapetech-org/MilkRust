# Bug Council Severity And Confidence

Severity:

| Severity | Meaning |
| --- | --- |
| Critical | Remote code execution, secret exposure, supply-chain compromise, persistent data corruption, or user content destruction. |
| High | Untrusted input reaches filesystem, process, parser, renderer, allocation, cache, or network sinks without a durable guard. |
| Medium | Incorrect lifecycle, stale state, confusing diagnostics, or denial-of-service requiring local/user action. |
| Low | Minor correctness or maintainability issue with narrow blast radius. |

Confidence:

| Confidence | Meaning |
| --- | --- |
| Proven | Reproduced by test, script, crash, or direct static proof. |
| Likely | Strong static evidence with a clear exploit/failure path. |
| Unverified | Scanner or report output needs triage; keep it out of the confirmed-finding ledger until verified. |

Accepted findings must be sibling-searched, fixed with behavior coverage, and pinned in a durable gate.
