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
- The compatibility CLI now retains every discovered file in the file denominator. Read/encoding failures are returned in `fileReadErrors` and count as unsupported files instead of silently disappearing.
- npm scripts now call the filenames that actually exist under `tools/`.

These changes close the identified false-positive and denominator-loss paths in source. The corpus totals and real renderer outcomes still require execution on the pinned source tree.

## Remaining engine work

- Establish a versioned conformance corpus and emit per-file discovered/read/parse/compile/render/fidelity states, including missing textures and exclusions.
- Compare classic `.milk` state, wave/shape equations, mesh warp, feedback, blur/noise resources, transitions, shader semantics and color/coordinate behavior against a reference renderer.
- Complete and qualify distinct warp and composite shader stages; the current translator handles a subset and cannot be described as a complete HLSL implementation.
- Complete `.milk2` double-preset state, render targets, blend timing and saved transition behavior from actual MilkDrop 3 samples.
- Implement real GPU renderers on each claimed target. The headless batching path and desktop shell are not equivalent to a native full-fidelity player.
- Define safe, bounded support for imported formats. Do not execute downloaded JavaScript or remote shader includes.
- Add independent visual evidence with negative controls; parser acceptance and nonempty output do not prove faithful rendering.

## Remaining packs and app integration

- Evolve folder-only manifests into versioned archive packs with per-file hashes, immutable content addresses, size/count limits, dependencies, licensing notices and schema migration.
- Build only packs whose redistribution rights are clear. `NOASSERTION` community entries remain source links/import targets unless permission is established; a disclaimer does not confer rights.
- Add the archive builder, deterministic catalog generation, trusted catalog signing, app-side verification, bounded extraction, atomic activation, update rollback, resume/restart behavior and offline use.
- Integrate a pack browser and library into PlezyNG with local `.milk`/`.milk2` import, search, favorites, playlist controls, accessibility/focus handling and visible per-preset compatibility diagnostics.
- Add an actual playback-owned PCM stream and native surface backend before calling any view live-audio-reactive MilkDrop playback. PlezyNG's existing six scenes currently use analyzed loudness or ambient animation.
- Keep platform support, audio capability, rendering fidelity, distribution status and release availability as separate states.

## Reproducible command map

Root npm scripts now target the checked-in RustyMilk-named tool files. For current pack and compatibility commands see `package.json`, `docs/PRESET_PACKS.md` and `AGENTS.md`. Build, test, scan and release work follows the repository's private ephemeral JIT runner policy.
