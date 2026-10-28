# Broader cleared pack preparation

`butterchurn-presets@2.4.7` is already vendored under
`content/third-party/butterchurn-presets-2.4.7/` with its upstream MIT notice and
provenance. The source collection's notice is the basis for redistribution; it
does not make unrelated unlicensed MilkDrop archives redistributable.

`tools/convert_butterchurn.py` prepares multiple portable classic packs from the
shaderless numeric-expression subset. It parses a restricted grammar rather
than running JavaScript. Every JSON gets an accounting row, including failures.
Unknown fields, shader code, ambiguous JS/EEL semantics and unsupported syntax
are rejected with reasons. Nothing is silently stripped to increase a count.
Original titles, filename credits, input hashes, immutable source links, license
and transformation records accompany every converted file. Filenames use input
hashes to avoid platform case/reserved-character collisions. Parts default to
200 presets to leave room under the app's 2,500-file and 64 MiB expanded limits.

On a compatible private JIT runner, after the converter is reviewed:

```sh
python3 tools/convert_butterchurn.py --source-revision "$CONTENT_COMMIT" --output "$RUNNER_TEMP/cleared-packs"
python3 -m unittest discover -s tools/tests
# For each generated pack directory, use the existing deterministic builder:
python3 tools/package-rmpack.py "$PACK_DIRECTORY" "$ARCHIVE_PATH"
```

Review the complete `conversion-report.json`, run the declared renderer/corpus
qualification, then commit only cleared archives and their notices, manifests
and reports. Add immutable URLs and measured sizes/hashes to
`content/downloads/catalog.json`. Use `tools/signed_catalog.py` for authenticated
delivery after release trust keys are provisioned. Never copy an unrun candidate
count into `rendered` or `qualified` fields. Converter/report generation and
packaging have **not run in this session**; no broader downloadable pack is
claimed from the recipe alone.

Promotion is now enforced by `tools/catalog_qualification.py`, not just this
document. A broader pack without the complete immutable report described in
[`content/qualification/README.md`](../content/qualification/README.md) fails
catalog preparation. The app receives a signed report link and target list and
shows qualification separately from redistribution clearance.

Shader-bearing converted JSON stays in the source inventory for the shader
track. Its GLSL is not falsely labeled original HLSL. The historical original
MilkDrop and Cream of the Crop collections remain link/import-only until their
per-asset redistribution rights are cleared. Disclaimers do not supply a missing
license. A removal request revokes the signed identity and updates future packs;
existing user-owned copies are not erased.

Default numeric fields are materialized using the pinned Butterchurn reference
`fbac2f6bab62fd9c6a50ebbeb29359c5eb05903e` (`src/visualizer.js`).
The extracted values and upstream MIT notice live in `tools/data/`. This avoids
assuming that omitted JSON fields inherit identical defaults from projectM;
reference-version parity still belongs in conversion qualification.
