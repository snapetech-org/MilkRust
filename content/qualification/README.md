# Pack promotion evidence

No qualification reports exist yet. No empty example report is a pass.

`tools/catalog_qualification.py` is called by the catalog preparation step.
It checks immutable evidence for the exact archive, every preset and every
target in `all-native-hosts-v1`: Linux x64/arm64, Windows x64/arm64, macOS
x64/arm64, iOS/tvOS arm64, Android arm64/armv7/x64, webOS arm64 and Tizen ARM.
Tizen's current TV package is 32-bit ARM; the x64 emulator is not device proof.
Each report also pins engine/host revisions, artifact hashes, physical GPU and
driver, private JIT run URL, rights review and source/evidence digests. A target
matrix change is a reviewed protocol/policy change in both MilkRust and PlezyNG.

The sole fixture exception pins the exact existing two-preset CC0 archive
digest and still reports `sample-fixture-only`. Renaming a different archive or
adding a license disclaimer cannot use this exception.

Reports go in `reports/<id>.json`; referenced evidence goes in
`evidence/<id>.json`. Use the validator's field names and types. The synthetic
unit cases in `tools/tests/test_catalog_qualification.py` illustrate the schema
but must never be copied into release evidence. Capture digests identify
retained private JIT artifacts; review the actual captures and measurements
before accepting a report. Metadata checks cannot prove a renderer ran.

Every preset's evidence must pass rendering (at least 300 frames with pinned
audio and capture digests), a reviewed comparison against an identified
reference, audio synchronization, resource limits and lifecycle recovery. Each
check records its method; qualitative review is explicit rather than a made-up
universal image-difference threshold. Missing, skipped, unsupported, failed,
converted-only and not-run results block promotion. The engine revision in a
report defines its scope; future engines/devices need requalification.

After review, commit the evidence first. Add a `qualification` pin with
`revision` (40 hex commit), `path` and `sha256` to the pack entry, and set its
`compatibilityStatus` to `qualified`. The publisher verifies the report from
that commit. Archive publication and signing remain separate reviewed steps.
There is no converter-to-production automatic promotion.
