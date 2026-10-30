# MilkRust implementation and qualification ledger

Updated 2026-10-30. This is the current ledger for PR #20. Older snapshots remain
in git history. Source implementation, successful builds, visual conformance and
release delivery are separate states. The user has now requested build work; the
repository policy requires private ephemeral JIT runners, and this workspace has
no runner-dispatch control. No build, test, pack generation, GPU capture or device
run is recorded for this checkpoint.

## Implemented in source

- Standard numbered `warp_N`/`comp_N` shaders, initialization ordering, indexed
  shape/wave/sprite keys, bounded input and explicit read/encoding/parse failures.
  MilkDrop's four custom shape/wave slots are enforced: inert out-of-range
  records remain round-trippable; active or equation-backed out-of-range
  objects fail before frame construction. Legacy double-preset detection uses
  exact section-marker lines, so marker text in a comment does not change the
  imported format.
  The corpus denominator retains unreadable and unsupported files.
- Saved sprites have bounded independent equations/lifetimes, relative texture
  dependencies, transforms and all eleven MilkDrop 3 blend modes (default mode 4;
  invalid selectors use mode 0), final-frame retirement and shared native/WebGL
  draw markers. Layers 1–4 enter feedback at the warp, shape, custom-wave, and
  main-wave/border stages. Burning layer sprites draw twice at their feedback
  stage; layer 0 draws over the composite and stamps feedback once. The final
  `done && burn` primary feedback draw uses the 4:3 correction. Saved alpha
  multiplies clamped EEL `a`; burn semantics follow the saved integer; layer and
  blend are static selectors. `SpriteRot` modes 1 and 2 add/subtract absolute
  `time * SpriteSpeed`. `SpriteName` uses its last saved occurrence; other saved
  fields use their first. Repeated code lines run contiguously through line 255.
  Both hosts admit the documented 100-name image cache under the aggregate
  pixel budget. Color keys use the legacy hexadecimal scan. Static keyed
  images are converted to transparent-black derivatives before mip generation;
  GIFs bypass color keying and mip generation. These source paths still need
  reference-pixel and GPU filtering qualification.
  WebGL retains frames during image admission.
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
- Global `Texture1D`/`Texture2D`/`Texture3D` and legacy
  `sampler`/`sampler2D`/`sampler3D` arrays with literal bounds 1–15 expand
  into separately named bindings; grouped resource declarators and array
  fields on global resource structs are supported. Literal element accesses
  select one binding. Side-effect-free dynamic indices dispatch modern
  `Sample`/`SampleLevel`/`SampleBias`/`SampleGrad`/`Load` and standalone
  `GetDimensions` calls across those names; legacy sampler arrays dispatch
  supported `tex*` intrinsics when used as the first argument. Out-of-range
  dynamic reads return zero (or zero dimension outputs), and dispatch code is
  capped at 512 KiB. Dynamic accesses with calls or side effects, function-local
  arrays, and nested resource-bearing structs fail closed.
- The bounded WGSL expression path lowers floating scalar/vector HLSL `fmod` to
  signed WGSL remainder; its integer/boolean, advanced sampling and broader
  shader-language limits remain explicit.
- Scoped typedefs, generic numeric spellings, grouped/brace/inferred declarations,
  flattened aggregate initialization/casts and lexical typed constant evaluation
  now cover arrays within structs, matrix/vector selections and arithmetic within
  the profile. User names are isolated from GLSL keywords/intrinsics. Readonly
  initializers use valid GLSL storage, and writes to readonly values fail.
- Sprite modes 0–10 follow the saved D3D9 texture-stage and blend-factor table;
  the missing `SpriteBlend` default is 4 and invalid saved selectors use 0.
  `SpriteLayer` and `SpriteBlend` are static fields, not EEL selectors. Invalid
  layers are inert. SpriteSpeed uses absolute time only for saved `SpriteRot`
  modes 1 and 2; saved `SpriteAlpha` multiplies the clamped EEL alpha without a
  second clamp; saved burn mode controls whether EEL `burn` applies. The keyed
  static-image key-before-mip derivatives and the GIF-specific no-key/no-mip
  path are implemented; reference-pixel and physical-renderer qualification
  remain open.
- All 24 legacy rotation matrices, constant registers `_c0.._c13`, canonical
  coordinate/roam/mip helpers and matched blur range encoding/decoding are bound
  in native and WebGL hosts. Regression sources are present but unrun.
- Shared writable shader globals, coherent packed/scalar q aliases, original UV
  and animated corner hue inputs, explicit legacy entry semantics and actual
  named-texture dimensions are wired through both renderer hosts. User preamble
  globals initialize after that environment, with bounded array/struct defaults.
- Three blur levels use MilkDrop's paired-tap horizontal/vertical weights,
  source-texel offsets, nested range transforms and first-level edge darkening in
  native/WebGL; safe nondegenerate bounds avoid the historical zero-span typo.
  Deterministic original 2D/3D noise, mipmaps and per-binding filtering/wrapping
  are also present. q registers, frame uniforms, texture sizes and random/roam
  vectors are bound. Blur pixel parity and generated-noise reference comparison
  remain open; missing resources and compiler/driver errors do not become a placeholder.
- A bounded `.milk2` wrapper parser, round-trip metadata preservation, two
  independent states, all 30 saved-pattern formulas with per-fragment 64x48
  triangle interpolation from a packed 32-bit vertex field, and the documented sprite
  field/runtime paths for native and WebGL. Both equation scopes run independently;
  PRESET2 uses a temporary cosine-mixed render scope with shader-dependent boolean
  snapping, while persistent registers remain per-preset. Custom shape fills,
  outlines and waves receive the saved linear blend weights, and preset outer/inner
  borders are suppressed. Sprite layers are ordered within the
  feedback scene through explicit markers in both GPU renderers. Persistent
  frame-set runtimes carry omitted header fields between sequential `.milk2`
  loads with process-start defaults of empty pattern, progress `1.0`, direction
  `0`, and random values zero. Ordinary single-preset scopes carry valid
  writes to those shared globals across soft cuts, and the runtime exposes a
  validated snapshot/restore API for host recreation. Pattern formulas use
  the effective carried random registers directly: process-start masks use the
  zero-valued defaults, and partial headers inherit prior values for omitted
  slots. Mask evaluation never invents midpoint or seeded fallback values. A
  missing pattern fails closed rather than inventing a transition.
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

## Aggregate correction — 2026-10-30

Static review found that the integer/Boolean matrix aggregate constructor
advanced the flattened input cursor twice. It now consumes each matrix exactly
once, including matrices followed by other struct fields. Per-fragment default
initialization now constructs these emulated matrices from typed zero rows.
The aggregate control covers signed/unsigned/Boolean matrices, trailing fields,
flattening, casts and global defaults. These controls are authored and unrun.

## Multidimensional numeric arrays — 2026-10-30

HLSL arrays now retain up to 16 dimensions under a 4096-element storage budget.
The ES300 emitter represents inner rows with structs, including row slices,
indexed writes, typedefs, function parameters/returns, initialization/casts,
and arrays of structs/matrices. Generated type definitions precede helpers;
grouped variables and loop initializers keep independent row types. Native
and browser numerical controls are authored but unrun. Opaque resources still
follow the separate binding limits listed below.

## Public corpus source inventory — 2026-10-29

A filesystem inventory of `content/community-unlicensed` found 14,576 preset
files. All decode losslessly under the current UTF-8, BOM-marked UTF-16, and
Windows-1252 policy: 14,539 UTF-8, 23 Windows-1252, and 14 UTF-16. The
inventory found 36 distinct call names in recognized equation fields; every
name is present in the EEL function allowlist. The highest observed custom
wave/shape index is 4; its records occur in one preset and both index-4 objects
are disabled. MilkDrop 3 has four slots, indexed 0–3. The tree contains 94 JPEG,
12 TGA, and one PNG image; no other image extensions. Its single mixed-case
preset suffix is `.MILk`, which exposed an app import/native-load mismatch and
is now accepted case-insensitively by PlezyNG's shared native host.

This inventory is lexical source evidence only. The full CLI compatibility
report still must run on the private JIT runner to exercise MilkRust's parser
and typed shader frontend across every file. It does not compile shaders,
render frames, resolve external texture packs, or establish fidelity.

## Still open

- First compiler/link/build checks on the required private JIT runners,
  native/WebGL driver execution, corpus captures and reference-image comparisons.
  Source acceptance or a nonempty frame is not visual conformance. None of the
  new platform/preset cells is qualified.
- Reference captures for MilkDrop shader resources and `.milk2` mesh
  interpolation/compositing, per-frame variable blending, weighted custom
  objects, border suppression, all eleven sprite blend modes/filtering and
  layer ordering. These are source paths pending reference-image comparison.
  The typed HLSL
  profile now lowers common `Texture1D`/`Texture2D`/`Texture3D` sampling methods and direct
  resource parameters/fields, including the static offset overloads supported
  by GLSL ES 3.00, offset `Load` with defined out-of-range zero results, and
  base/mip-level `GetDimensions` with full mip-chain level counts. `Texture1D`
  maps to the center row of a 2D image and needs a one-row asset for 1D parity.
  It still rejects texture types outside those three dimensions, integer/non-color texture
  element types, resource arrays in function scopes or nested resource-bearing
  structs, initialized resource instances, resource-bearing struct parameters,
  comparison methods and gather methods. Global static `SamplerState` definitions
  support all eight standard point/linear min/mag/mip `Filter` combinations and
  independent U/V/W Wrap, Clamp or Mirror addressing. Anisotropic filtering, border
  addressing, dynamic state expressions, and custom local/struct state initialization
  remain outside the profile. Global legacy effect-file `sampler_state` blocks
  bind one `Texture = <identifier>` and support independent Point/Linear min and
  mag filters plus None/Point/Linear mip filtering and per-axis Wrap/Clamp/Mirror.
  Only direct `tex*` first arguments are supported. Grouped declarations and
  state escape through helpers or assignments remain explicit failures.
  Dynamic array dispatch is limited to side-effect-free indices, supported
  sampling/load methods, standalone `GetDimensions`, and legacy `tex*` calls
  with the array element as their first argument. Direct `SamplerState` fields
  are accepted as method arguments but use resource prefixes. Global initialized
  states lower through per-resource aliases; parameter and struct-field state
  propagation is not implemented. The compiler remains a
  bounded pixel-shader profile, not all HLSL shader models. The separate WGSL
  path remains limited.
- Complete format/EEL/texture behavior for every corpus file, including other legacy
  encodings, other executable extensions and unsupported formats. The current
  community-tree inventory found no unhandled text encoding or equation call,
  but CLI parser/shader analysis and missing external texture-pack accounting
  remain unrun. Animated PNG/WebP and additional unsupported historical image
  formats remain explicit import failures.
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


## 2026-10-30 MilkDrop blur-kernel source update

Native GL and WebGL now implement MilkDrop's paired-tap horizontal and
vertical blur kernels from the compatible reference shader sources, including
source-texture texel offsets, per-level range normalization, and first-level
edge darkening. Safe bounds avoid the reference implementation's zero-span
repair typo. No build or pixel comparison was run; filter parity remains open.


## 2026-10-30 dynamic resource-array lowering

Global bounded texture/sampler arrays now support grouped declarations, 1D/2D/3D texture bindings, literal indices, and side-effect-free dynamic selection without indexing opaque GLSL values. Dynamic texture method calls are expanded across each named binding; legacy sampler arrays expand supported `tex*` calls with the array element as the first argument. Standalone dynamic `GetDimensions` uses a bounded branch, and out-of-range dynamic access produces zero. A 512 KiB generated-dispatch budget limits nested expansion. Function-local arrays, nested resource-bearing structs, advanced sampler states and unsupported HLSL resource forms remain explicit compiler failures. The source lock and handoff are synchronized in PlezyNG PR #82. No build or test was run under the source-only restriction.

## 2026-10-30 legacy sampler-state lowering

Static global legacy effect-file `sampler_state` blocks now lower bound texture identifiers and independent Point/Linear min and mag filters, None/Point/Linear mip filtering, and per-axis Wrap/Clamp/Mirror addressing into the same per-resource host aliases used by modern `SamplerState`. Each omitted filter field defaults to linear; omitted axes default to wrap. The compiler removes the effect initializer and rewrites only direct first arguments to supported `tex*` calls. Grouped declarations, unsupported sampler properties, dynamic state, and sampler-state variables used outside direct sampling remain explicit failures. This code change is recorded in the HLSL compatibility profile; platform qualification remains pending.


## 2026-10-30 WebGL saved-sprite sampling profiles

The WebGL saved-sprite pass now applies the same per-texture minification,
magnification, mip, and U/V/W address profile as native GL, including
Wrap/Clamp/Mirror. Animated sprites retain their dedicated linear, no-mipmap,
repeat profile. This closes a source-path mismatch; sprite filtering and
addressing still require reference captures. The change is unbuilt and unrun.


## 2026-10-30 saved-sprite transform and repeat semantics

Sprite `repeatx`/`repeaty` now clamp the equation value first and then multiply by
the saved `SpriteRepeatX`/`SpriteRepeatY`. Saved position and scale offsets are
also applied after their equation values are clamped, matching the pinned
behavioral source. The existing regression expectation was updated; execution
and reference-frame comparison remain pending.


## 2026-10-30 MilkDrop 3 saved-sprite enable flag

Classic `.milk` saved sprites retain the default-on behavior, with
`sprite=0` disabling execution. Trailing sprites in a `.milk2` wrapper now
require an explicit `sprite=1`; a missing flag or `sprite=0` leaves the
blocks preserved but inert. Both the import path and direct saved-blend
validation apply the same rule, including BOM-prefixed wrapper headers. The change follows the pinned compatible
behavioral source in Lactea's MIT-licensed parser. No build, test or reference
capture has run yet.
