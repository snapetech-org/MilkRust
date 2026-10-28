# MilkRust Gap Audit

This audit replaces the older roadmap snapshot, which no longer matched the crates and tools in the repository. It distinguishes shipped code from the remaining work toward MilkDrop compatibility. A parser report is not a rendering-fidelity result.

## Existing foundations

- `milkrust-core` contains preset parsing, expression evaluation, compatibility analysis, frame-state construction and shader translation.
- `milkrust-expr` is a separate crate; expression support must be assessed by its implemented semantics rather than by the old claim that the crate is missing.
- Renderer contracts, a headless renderer, a WebGL2/WASM renderer, a web SDK, a Rust CLI, desktop player/studio shells and folder-based packs exist.
- `content/catalog.json`, `content/third-party/` and `content/community-unlicensed/` record content status and provenance. Community-unlicensed content is excluded from default builds.
- The web pack loader supports lazy manifest and preset-source loading for large directories.

## Work completed on the visualizer compatibility branch

- The preset parser now recognizes numbered classic MilkDrop shader keys (`warp_N` and `comp_N`), removes the per-line backtick marker and combines shader lines in numeric order.
- The parser accepts additional indexed shape, sprite and wave key forms, including `shapecode_N_*`, `shape_N_*`, `spritecode_N_*`, `wave_N_*` and the existing forms.
- `per_frame_init_N` and `frame_init_N` now populate the initialization equation scope before generic frame-equation matching.
- Two-preset frame generation resolves each preset's shader and texture-sampler data from its own document instead of reusing the first preset's shader data for both entries.
- The compatibility CLI now retains every discovered file in the file denominator and emits per-file read, parse and current-subset status plus per-preset diagnostics. I/O failures appear in `fileReadErrors`; non-UTF-8 files appear in `encodingWarnings` and are forced unsupported after a lossy diagnostic parse. Render and visual-fidelity stages are explicitly `not-run` / `not-assessed`; no file disappears from the denominator.
- Blank and comments-only preset files now fail the import and compatibility gates instead of being counted as supported merely because no unsupported syntax was encountered.
- Static compatibility results now enumerate known renderer-feature blockers separately from parser/function/shader-subset findings. In particular, presets with both warp and composite shader bodies are unsupported until those bodies have separate render stages; the diagnostic does not imply a working renderer.
- Multi-state `.milk2` files are also marked unsupported by the current subset until saved blend patterns and transition state are implemented; the generic two-frame crossfade remains an intermediate representation.
- Compatibility JSON now reports per-file read/parse/subset status, per-preset diagnostics, and explicit `renderStatus: not-run` / `fidelityStatus: not-assessed`; geometry statistics are labeled as non-render output. The community-report generator labels static-subset results and records a deterministic SHA-256 fingerprint for each source pack. The checked-in May 2026 report remains a historical snapshot until regenerated on the current branch.
- npm scripts now call the filenames that actually exist under `tools/`.

These changes close the identified false-positive and denominator-loss paths in source. The corpus totals and real renderer outcomes still require execution on the pinned source tree.

## Remaining engine work

- Establish a versioned conformance corpus and emit per-file discovered/read/parse/compile/render/fidelity states, including missing textures and exclusions.
- Compare classic `.milk` state, wave/shape equations, mesh warp, feedback, blur/noise resources, transitions, shader semantics and color/coordinate behavior against a reference renderer.
- Complete and qualify distinct warp and composite shader stages; the current translator handles a subset and cannot be described as a complete HLSL implementation. Static diagnostics now flag presets requiring both stages, but do not compile or render either stage.
- Complete `.milk2` double-preset state, separate render targets, saved blend pattern/progress/direction/random values and transitions from actual MilkDrop 3 samples. The current two-frame container and generic alpha remain an intermediate representation; static compatibility diagnostics now mark multi-state files as blocked on this feature.
- Implement real GPU renderers on each claimed target. The headless batching path and desktop shell are not equivalent to a native full-fidelity player.
- Define safe, bounded support for imported formats. Do not execute downloaded JavaScript or remote shader includes.
- Add independent visual evidence with negative controls; parser acceptance and nonempty output do not prove faithful rendering.

## Remaining packs and app integration

- Extend versioned archive packs with immutable content addresses, dependency handling, schema migration and published compatibility records. The reproducible builder now requires per-file author, license, HTTPS source URL and shipped-notice records, checks full content-file coverage, and emits an integrity index; one CC0 fixture archive exists.
- Build only packs whose redistribution rights are clear. `NOASSERTION` community entries remain source links/import targets unless permission is established; a disclaimer does not confer rights.
- Add a signed remote catalog and broader cleared-pack availability. PlezyNG's first pinned catalog entry verifies the archive and per-file index, bounds extraction, validates notices and activates atomically; catalog updates, cancellation/resume/restart and update rollback remain open.
- PlezyNG has a basic library sheet with one download, link-only sources and local `.milk`/`.milk2` import. Search, favorites, playlists, previews, local-import removal recovery, and per-preset compatibility diagnostics remain open.
- Add an actual playback-owned PCM stream and native surface backend before calling any view live-audio-reactive MilkDrop playback. PlezyNG's existing six scenes currently use analyzed loudness or ambient animation.
- Keep platform support, audio capability, rendering fidelity, distribution status and release availability as separate states.

## Reproducible command map

Root npm scripts now target the checked-in RustyMilk-named tool files. For current pack and compatibility commands see `package.json`, `docs/PRESET_PACKS.md` and `AGENTS.md`. Build, test, scan and release work follows the repository's private ephemeral JIT runner policy.
