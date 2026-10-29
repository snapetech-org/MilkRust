# Broader cleared pack preparation

`butterchurn-presets@2.4.7` is vendored under
`content/third-party/butterchurn-presets-2.4.7/` with its upstream MIT notice
and pinned provenance. That license does not apply to unrelated, historically
unlicensed MilkDrop collections. Those remain link-only or user-local imports;
a disclaimer does not create redistribution rights.

`tools/convert_butterchurn.py` prepares portable classic packs from a restricted
shaderless numeric-expression subset. It parses a bounded grammar rather than
running JavaScript. Every discovered JSON gets an accounting row, including
failures. Unknown fields, shader code, ambiguous JS/EEL semantics and
unsupported syntax are rejected with reasons. Titles, filename credits, input
hashes, immutable source links, license notices and transformation records are
retained. Hash-derived filenames avoid platform case/reserved-character
collisions. Pack parts default to 200 presets to leave room under the app's
file and expanded-byte limits.

## Candidate generation

`tools/build_cleared_pack_candidates.py` runs the converter and deterministic
`.rmpack` packager, verifies the result digests, and emits a separate
`candidate-catalog.json`. It refuses incomplete source accounting or any
converter report that claims rendering or qualification. The command does not
modify `content/downloads/catalog.json` and never executes preset code.

Run it only on an approved private ephemeral JIT runner:

```sh
python3 tools/build_cleared_pack_candidates.py \
  --source-revision "$CONTENT_COMMIT" \
  --output "$RUNNER_TEMP/cleared-pack-candidates"
```

The manual main-only `Prepare cleared preset pack candidates` workflow runs on
the private JIT runner and uploads archives plus complete accounting as a
short-lived review artifact. It has no production credentials and only read
repository permission. Candidate packs report `converted-unqualified`, zero
rendered, zero qualified, and `publicationEligible: false`. They do not appear
in the app's in-app downloads.

Review the full candidate catalog and conversion report, then complete an
independent rights/attribution review and per-preset renderer qualification.
Only after each gate is approved should immutable archives and evidence be
committed, their measured URLs/sizes/hashes added to
`content/downloads/catalog.json`, and the signed catalog publisher used. The
qualification contract, all required host targets, and evidence requirements
are defined in [`content/qualification/README.md`](../content/qualification/README.md)
and enforced by `tools/catalog_qualification.py`. Conversion acceptance is not
render compatibility. Never copy an unrun candidate count into `rendered` or
`qualified` fields.

Candidate generation, rendering, qualification and signed promotion have not
run in this source-only session. The existing in-app download catalog still
contains only the small authored CC0 sample pack and link-only entries. The
historical MilkDrop and Cream of the Crop collections remain link/import-only
until their per-asset redistribution rights are established.

Butterchurn shader-bearing JSON stays in the source inventory for the separate
shader-compatibility track. Its GLSL is not falsely labeled original HLSL.
Default numeric fields are materialized using the pinned Butterchurn reference
`fbac2f6bab62fd9c6a50ebbeb29359c5eb05903e` (`src/visualizer.js`) and the
extracted values and upstream MIT notice in `tools/data/`.
