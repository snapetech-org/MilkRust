# Pack qualification evidence

Catalog promotion uses two separate gates: redistribution rights and runtime
compatibility. A source license or a successful conversion does not satisfy the
runtime gate. A disclaimer does not substitute for permission to redistribute.

`tools/catalog_qualification.py` accepts a pack only when its immutable report
is pinned by commit and SHA-256 in `content/downloads/catalog.json`. A complete
report uses `schemaVersion: 1`, `status: qualified`, and
`policy: all-native-hosts-v1`. It identifies the exact pack ID, version and
archive SHA-256, includes approved rights-review evidence, and covers every
target below exactly once:

| Target | Required physical device |
| --- | --- |
| `linux-x64` | Linux x64 |
| `linux-arm64` | Linux ARM64 |
| `windows-x64` | Windows x64 |
| `windows-arm64` | Windows ARM64 |
| `macos-x64` | macOS x64 |
| `macos-arm64` | macOS ARM64 |
| `ios-arm64` | iOS ARM64 |
| `tvos-arm64` | tvOS ARM64 |
| `android-arm64` | Android ARM64 |
| `android-armv7` | Android ARMv7 |
| `android-x64` | Android x64 |
| `webos-arm` | webOS ARM |
| `tizen-arm` | Tizen ARM |

For every preset on every target, the report must pin the engine and host
revisions and artifacts, identify the physical device, and link a private
ephemeral JIT run. Per-preset evidence covers at least 300 rendered frames,
reference-output comparison, audio synchronization, resource limits and app
lifecycle. Evidence files are immutable JSON under
`content/qualification/evidence/`; the report is immutable JSON under
`content/qualification/reports/`. Both are bound to the exact archive and
preset hashes.

The verifier checks structure and identity only. Reviewers must inspect retained
run artifacts, confirm actual pixels and behavior, and approve the rights basis
before setting a report to `qualified`. The verifier cannot inspect a display,
prove that a rights review was sound, or replace device testing. All jobs that
build, render or package content must use the approved private ephemeral JIT
runner policy in `AGENTS.md`.

The sole qualification exception is the exact two-preset authored CC0 fixture
archive already pinned in the catalog, which remains labeled
`sample-fixture-only` with no qualified targets. A different digest, a rename,
or a license disclaimer cannot use that exception.

## Candidate status

`tools/build_cleared_pack_candidates.py` can create deterministic, explicitly
non-promotable review archives from the shaderless Butterchurn subset. Every
asset remains `NOASSERTION`; the upstream package's MIT declaration is retained
as evidence but does not substitute for the per-asset rights-holder review.
The separate `Prepare preset pack review candidates` workflow uploads them as
short-lived review artifacts. Candidate metadata always reports
`rights-review-required`, zero rendered and zero qualified presets. It does not
change the signed download catalog, so these archives cannot appear in the app
until rights and compatibility gates are completed and reviewed.
