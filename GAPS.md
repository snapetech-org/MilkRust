# MilkRust implementation and qualification ledger

Updated 2026-10-29. This is the current ledger for PR #20. Older snapshots remain
in git history. Source implementation, successful builds, visual conformance and
release delivery are separate states. No build, test, CI dispatch, pack generation,
GPU capture or device run was performed for this source-only continuation.

## Implemented in source

- Standard numbered `warp_N`/`comp_N` shaders, initialization ordering, indexed
  shape/wave/sprite keys, bounded input and explicit read/encoding/parse failures.
  The corpus denominator retains unreadable and unsupported files.
- Stateful EEL execution with shared work/depth budgets, lazy branches, checked
  failure propagation, one-time assignment target resolution and non-panicking
  integer remainders. CLI, desktop and browser callers consume checked frames.
- Native C ABI and GL/GLES renderer; independent feedback/state for saved pairs;
  warp, geometry and composite stages; named textures; bounded PCM analysis and
  RGBA capture. WebGL2 has the same stage/resource model and explicit failures.
- Typed legacy HLSL compilation through pinned MIT HLSLParser, compiled natively
  with target C++ and in browsers through pinned Emscripten. Macros, helpers,
  structures, arrays, matrices, control flow and legacy sampling are implemented
  behind bounded preprocessing/compiler boundaries. See
  [the exact profile and limits](docs/HLSL_COMPATIBILITY.md).
- All 24 legacy rotation matrices, constant registers `_c0.._c13`, canonical
  coordinate/roam/mip helpers and matched blur range encoding/decoding are bound
  in native and WebGL hosts. Regression sources are present but unrun.
- Three blur levels, deterministic original 2D/3D noise, mipmaps and per-binding
  filtering/wrapping; q registers, frame uniforms, texture sizes and random/roam
  vectors. Missing resources and compiler/driver errors do not become a placeholder.
- A bounded `.milk2` wrapper parser, round-trip metadata preservation, two
  independent states and deterministic masks for all documented pattern names.
  [Format and reference-source limits](docs/MILK2_FORMAT.md) explain what those
  independently authored masks do and do not establish.
- Deterministic archives with integrity indexes and per-file provenance/notices;
  a signed catalog publisher, quorum/sequence/expiry/revocation handling,
  qualification gates, protected-environment signing configuration and public
  trust-root tooling. No production credentials are committed.
- A pinned CaYaDev collection adds 55 unchanged upstream presets with explicit
  CC BY 4.0 terms, per-file attribution and verified SHA-256/Git blob provenance.
  Its builder preserves those origins and creates pending qualification records.
- Two new original CC0 study collections contain 24 presets across eight families.
  The candidate builder pins source URLs and creates pending per-preset/target
  evidence inventories. It does not create passing reports or promote downloads.
  Historical and converted community candidates retain their unresolved rights.
- Private JIT build entrypoints include native C++ and browser compiler delivery.
  Regression sources cover the typed compiler, resource bindings, unsafe input,
  initialization retry and browser render failures; they remain unrun.

## Still open

- First compiler/link/build checks, native/WebGL driver execution, corpus captures
  and reference-image comparisons. Source acceptance or a nonempty frame is not
  visual conformance. None of the new platform/preset cells is qualified.
- Exact MilkDrop HLSL environment and noise/blur/reference semantics, remaining
  legacy emitter gaps and shader-model coverage. Modern DirectX resources are
  outside the legacy pixel-shader profile; the separate WGSL path remains limited.
- Reference-equivalent `.milk2` masks and embedded executable extensions. The
  available upstream source snapshot does not implement the saved wrapper; the
  independent masks are not a port of its unavailable saved-pattern renderer.
- Complete format/EEL/texture behavior for every corpus file, including legacy
  encodings, sprite extensions and unsupported formats. Preserve every exception.
- Compiler/driver hang containment, browser decoder allocation isolation,
  context-loss/recovery, long-run resource/performance and accessibility evidence.
- Qualified broader archives, rights review for historical collections, production
  key custody, committed public roots, signed publication and installed-app checks.

## App integration

PlezyNG PR #82 carries platform host and regular runtime-packaging source, paired
with this PR. Linux/Windows/Apple/Android still require newly built mpv artifacts
with the ordered PCM/session API; the app must pin their actual verified digests.
Tizen consumes Samsung decoded packets. webOS owns decoding/output for its bounded
SoLoud music route and retains NDL fallback. No resulting app artifact is claimed
working merely because these sources or build recipes exist.

The original PlezyNG plan and current status live under
`docs/plezyng/visualizations/` in that repository. All executable work must use
compatible private ephemeral JIT runners; missing capacity never authorizes a
hosted fallback. Keep untrusted PR work isolated and secret-free.
