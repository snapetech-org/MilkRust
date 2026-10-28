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
- Static compatibility results now enumerate known renderer-feature blockers separately from parser/function/shader-subset findings. In particular, presets with composite shader bodies or warp shaders combined with per-pixel equations remain blocked in the cross-renderer ledger. The experimental WebGL load gate now opts into its separate-stage source implementation explicitly; it does not change unrun qualification or other renderer targets.
- All `.milk2` files are marked unsupported by the current subset until saved blend patterns and transition state are implemented; the generic two-frame crossfade remains an intermediate representation.
- Compatibility JSON now reports per-file read/parse/subset status, per-preset diagnostics, and explicit `renderStatus: not-run` / `fidelityStatus: not-assessed`; geometry statistics are labeled as non-render output. The community-report generator labels static-subset results and records a deterministic SHA-256 fingerprint for each source pack. The checked-in May 2026 report remains a historical snapshot until regenerated on the current branch.
- npm scripts now call the filenames that actually exist under `tools/`.
- Both Rust expression evaluators now preserve MilkDrop `&&` / `||` short-circuiting and lazy `if()` / ternary branches, use the reference evaluator's 1e-5 boolean threshold, treat `band` / `bor` / `bnot` as boolean helpers, and reserve `&` / `|` for integer bitwise operations. The `^` operator is left-associative exponentiation above multiplication, `invsqrt` follows projectM's double-precision fast inverse-square-root routine, `%` / `mod` use integer remainder, and the standard `^=` / `%=` / `&=` / `|=` assignments are parsed. `if()` lvalue passthrough covers simple variable and megabuf targets. `memset` and projectM's no-op `freembuf` are implemented with bounded memory ranges, and `bxor` is no longer advertised as a MilkDrop built-in. Regression cases are committed but unrun.

These changes close the identified false-positive and denominator-loss paths in source. The corpus totals and real renderer outcomes still require execution on the pinned source tree.

## Remaining engine work

- Expression conformance is still incomplete: generic reference propagation through nested function results and arithmetic, remaining memory-index edge cases, exact RNG behavior, and full precedence/semantic comparison against the reference evaluator still need coverage. Simple `if()` / `memcpy()` / `memset()` assignment targets work, but the numeric evaluator is not a general reference-valued EEL VM.

- Establish a versioned conformance corpus and emit per-file discovered/read/parse/compile/render/fidelity states, including missing textures and exclusions.
- Compare classic `.milk` state, wave/shape equations, mesh warp, feedback, blur/noise resources, transitions, shader semantics and color/coordinate behavior against a reference renderer.
- Qualify the new separate warp/composite WebGL path and implement equivalent native/other-target paths. The current translator still handles only a subset and is not a complete HLSL compiler. The general static report remains conservative; only the explicit WebGL import gate enables the implemented stage path.
- Complete `.milk2` double-preset state, separate render targets, saved blend pattern/progress/direction/random values and transitions from actual MilkDrop 3 samples. The current two-frame container and generic alpha remain an intermediate representation; static compatibility diagnostics now mark multi-state files as blocked on this feature.
- Implement real GPU renderers on each claimed target. The headless batching path and desktop shell are not equivalent to a native full-fidelity player.
- Define safe, bounded support for imported formats. Do not execute downloaded JavaScript or remote shader includes.
- Add independent visual evidence with negative controls; parser acceptance and nonempty output do not prove faithful rendering.

## Remaining packs and app integration

- Extend versioned archive packs with immutable content addresses, dependency handling, schema migration and published compatibility records. The reproducible builder now requires per-file author, license, HTTPS source URL and shipped-notice records, checks full content-file coverage, and emits an integrity index; one CC0 fixture archive exists.
- Build only packs whose redistribution rights are clear. `NOASSERTION` community entries remain source links/import targets unless permission is established; a disclaimer does not confer rights.
- Add a signed remote catalog and broader cleared-pack availability. PlezyNG's first pinned catalog entry verifies the archive and per-file index, bounds extraction, validates notices and activates atomically; signed remote updates and resumable transfer qualification remain open; the app has source for cancel/retry, version-aware updates and replacement recovery.
- PlezyNG PR #55 now contains library selection, search, per-profile favorites/preferences, named playlists with reference import/export, shuffle/lock/timing/transitions/quality/fullscreen, and bounded pack download/import/replacement recovery, a bundled offline CC0 starter, portable path/image/library budgets, and local folder snapshots preserving textures/notices. Previews, broader content distribution, complete diagnostics and executed interaction checks remain open.
- PlezyNG PR #55 contains pinned projectM/mpv extension patches, a playback-owned PCM tap, and a Linux worker/Flutter texture host. The PCM patch composes with the 23 existing common/Linux patches in the active mpv-build source revision. There are no new native artifacts or runtime qualification results. This projectM adapter does not complete the custom MilkRust renderer. The six original scenes still use analyzed loudness or ambient animation.
- Keep platform support, audio capability, rendering fidelity, distribution status and release availability as separate states.

## Reproducible command map

Root npm scripts now target the checked-in RustyMilk-named tool files. For current pack and compatibility commands see `package.json`, `docs/PRESET_PACKS.md` and `AGENTS.md`. Build, test, scan and release work follows the repository's private ephemeral JIT runner policy.

## Source review follow-up — 2026-10-28

Fixed the core memory helper loop-budget constant, resolved assignment targets once (including side effects in memory-call targets), and made all integer-remainder paths non-panicking for zero divisors and signed-minimum overflow. Regression source covers these cases; it has not been executed. This source review does not establish build or runtime conformance.


## Experimental WebGL stages — source implementation, unqualified

Frames now carry independent warp/composite source and texture bindings. WebGL
uses warp on the feedback target (including per-pixel mesh UVs), draws geometry,
and applies composite only to the display. Shader results are no longer mixed
with history a second time. Each preset has its own feedback targets and stage
program caches; preset changes reset them. More than four named shader samplers
are retained, with runtime checks against the GPU's texture-unit limit.

Required translation/compile/link failures stop the preview and surface through
`renderStatusJson`, the web SDK and prototype UI. Missing textures fail explicitly;
images still decoding delay drawing. Named texture uploads have dimension/pixel
and cache limits; pending callbacks, programs, textures and framebuffers are
released on reset/disposal. Image dimensions are checked after browser decode,
so decoder allocation/isolation still needs a stronger preflight boundary.

The general compatibility report still blocks unqualified renderer stages and
saved `.milk2` blends. `validate_milkrust_webgl_import` opts into the experimental
WebGL stage path while retaining unsupported-equation/shader and saved-blend
gates. Canvas preview rejects shaders/meshes. Native GPU rendering, complete HLSL,
blur/noise resources, reference feedback/color behavior and device evidence remain
open. Unit-regression source covers separate stages/resources and gate behavior;
no compiler, test or GPU invocation ran. See [the concrete qualification cases](docs/WEBGL_STAGE_QUALIFICATION.md) and the original stage-order fixture.

Remote pack loading no longer executes JavaScript plugin entries. App-authored
hook objects can still be supplied explicitly through `loadPlugins`. Studio
exports use `NOASSERTION`/local-only metadata rather than assigning CC0 to
arbitrary imported content.
