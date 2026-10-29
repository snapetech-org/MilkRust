# MilkRust implementation and qualification ledger

Updated 2026-10-29. This is the current ledger for PR #20. Older snapshots remain
in git history. Source implementation, successful builds, visual conformance and
release delivery are separate states. No build, test, CI dispatch, pack generation,
GPU capture or device run was performed for this source-only continuation.

## Implemented in source

- Standard numbered `warp_N`/`comp_N` shaders, initialization ordering, indexed
  shape/wave/sprite keys, bounded input and explicit read/encoding/parse failures.
  The corpus denominator retains unreadable and unsupported files.
- Saved sprites have bounded independent equations/lifetimes, relative texture
  dependencies, transforms and all eleven MilkDrop 3 blend modes (default mode 4),
  burn/display layers and keyed
  texels before linear filtering. Both hosts admit the documented 100-name image
  cache under the aggregate pixel budget. Sprite color keys follow base-0
  unsigned integer parsing. WebGL retains frames during image admission.
  See [sprite semantics and reference limits](docs/EMBEDDED_SPRITES.md).
- Classic display controls and full feedback-coordinate transforms reach both
  hosts. All nine classic wave modes, stereo PCM, bounded custom point inputs,
  point colors, portable dots/thick strokes and source regressions are present.
  See [waveforms](docs/WAVEFORMS.md) and [classic controls](docs/CLASSIC_RENDERING.md).
- Custom-object frame/point state is isolated; init t registers, private variables,
  q inputs/global memory, shape instancing and texture gradients have explicit
  paths. Ordered drawing and feedback-derived motion vectors are shared across
  native/WebGL. See [state and drawing](docs/SCENE_STATE.md).
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
- Typed numeric lowering covers HLSL matrix orientation/splats/component math,
  rectangular linear algebra, writable selections, converting function outputs,
  vector logic and legacy intrinsics. Bounded numeric/identifier parsing and
  round-trip float output have authored numerical controls; execution is unrun.
- The bounded WGSL expression path lowers floating scalar/vector HLSL `fmod` to
  signed WGSL remainder; its integer/boolean, advanced sampling and broader
  shader-language limits remain explicit.
- Scoped typedefs, generic numeric spellings, grouped/brace/inferred declarations,
  flattened aggregate initialization/casts and lexical typed constant evaluation
  now cover arrays within structs, matrix/vector selections and arithmetic within
  the profile. User names are isolated from GLSL keywords/intrinsics. Readonly
  initializers use valid GLSL storage, and writes to readonly values fail.
- Saved sprite controls use the compatibility defaults and ordering: missing
  `SpriteBurn` means always burn, non-default `SpriteRot` uses absolute host
  time, and saved position/scale/repeat/alpha clamps precede their modifiers.
  Native and WebGL receive the same CPU-quantized D3D vertex bytes.
- Sprite modes 0–10 follow the saved D3D9 texture-stage and blend-factor table;
  the table default is 4 and out-of-range frame selectors map to 0. Modes 0, 5,
  6, 9 and 10 ignore image alpha; 4, 7 and 8 use it. Full ARGB color keys clear
  matching texels before filtering, and vertex colors follow legacy byte
  quantization. Rotation remains unwrapped for equations.
- All 24 legacy rotation matrices, constant registers `_c0.._c13`, canonical
  coordinate/roam/mip helpers and matched blur range encoding/decoding are bound
  in native and WebGL hosts. Regression sources are present but unrun.
- Shared writable shader globals, coherent packed/scalar q aliases, original UV
  and animated corner hue inputs, explicit legacy entry semantics and actual
  named-texture dimensions are wired through both renderer hosts. User preamble
  globals initialize after that environment, with bounded array/struct defaults.
- Three blur levels, deterministic original 2D/3D noise, mipmaps and per-binding
  filtering/wrapping; q registers, frame uniforms, texture sizes and random/roam
  vectors. Missing resources and compiler/driver errors do not become a placeholder.
- A bounded `.milk2` wrapper parser, round-trip metadata preservation, two
  independent states, all 30 saved-pattern formulas with per-fragment 64x48
  triangle interpolation from a packed 32-bit vertex field, and the documented sprite
  field/runtime paths for native and WebGL. Saved blend header registers carry
  across double-preset loads; first-load defaults are an unset pattern, progress
  1, direction 0 and zero random values. Missing first-load patterns fail closed.
  Formula provenance and the remaining pixel-qualification boundary are in
  [the format notes](docs/MILK2_FORMAT.md).
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
- WebGL admits raster headers before codec allocation, reserves aggregate
  pixel capacity before asynchronous loading, and decodes legacy DDS/TGA through
  bounded Rust code. Context restoration recreates GPU resources from retained
  source/assets. PNG/JPEG/BMP/WebP decode now runs in a disposable Web Worker
  with an 8-second termination timeout, bitmap dimension checks, and transferred
  ImageBitmap upload. The web SDK packages a static worker and configures it only
  when it is same-origin with the page; a rejected static worker falls back to a
  blob worker when CSP allows it. Direct wasm-bindgen users may configure a
  same-origin URL or use the blob-worker fallback. Animated GIF uses the shared
  bounded Rust decoder in native and WebGL: full-canvas compositing, disposal,
  per-frame delays and sprite-local or preset-time selection, capped at 120
  frames and 16 Mi decoded canvas pixels per asset. Netscape/AnimExts repeat
  metadata controls finite, single-play or infinite playback consistently across
  both renderer hosts.
  Native/browser decoder, upload and recovery regressions are unrun.
- Private JIT build entrypoints include native C++ and browser compiler delivery.
  Regression sources cover the typed compiler, resource bindings, unsafe input,
  initialization retry and browser render failures; they remain unrun.

## Still open

- First compiler/link/build checks, native/WebGL driver execution, corpus captures
  and reference-image comparisons. Source acceptance or a nonempty frame is not
  visual conformance. None of the new platform/preset cells is qualified.
- Reference captures for MilkDrop shader resources and `.milk2` mesh interpolation/compositing,
  all eleven sprite blend modes/filtering and layer
  ordering. The typed HLSL
  profile now lowers common `Texture2D`/`Texture3D` sampling methods and direct
  resource parameters/fields, including the static offset overloads supported
  by GLSL ES 3.00, offset `Load` with defined out-of-range zero results, and
  base/mip-level `GetDimensions` with full mip-chain level counts. It still
  rejects non-2D/3D resources, integer/non-color texture
  element types, resource arrays or arrays of arrays, initialized/nested
  resource structs, resource-bearing struct parameters, custom sampler-state
  definitions, comparison methods and gather methods. Declared direct
  `SamplerState` fields are accepted as method arguments; custom per-object
  filtering/addressing remains outside the binding model. The compiler remains a
  bounded pixel-shader profile, not all HLSL shader models. The separate WGSL
  path remains limited.
- Complete format/EEL/texture behavior for every corpus file, including other legacy
  encodings, other executable extensions and unsupported formats. UTF-8, BOM-marked UTF-16
  and Windows-1252 now have bounded decoders; preserve every remaining exception.
  Animated PNG/WebP and additional unsupported historical image formats remain
  explicit import failures.
- OS-level compiler/driver process/device hang containment, browser-process OOM
  isolation for native codec internals, recovery qualification, long-run
  resource/performance and accessibility evidence. Worker termination only bounds JS-side decoder
  waiting; it cannot sandbox browser-process memory failure or a wedged GPU driver.
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
