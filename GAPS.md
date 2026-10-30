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
  dependencies and transforms. The upstream dynamic blendmode uses five modes,
  defaults to 0, and truncates/clamps each frame to 0..4. Six optional
  SpriteBlend modes, SpriteLayer stages and other saved sprite controls are
  MilkRust extensions, not upstream MilkDrop 3 fields. Final-frame retirement,
  shared native/WebGL drawing, feedback layering, color keys, and animated
  sprite lifetimes are implemented; reference-pixel and GPU filtering
  qualification remain open. See
  [sprite semantics and reference limits](docs/EMBEDDED_SPRITES.md).
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
- Standard EEL blendmode follows upstream dynamic semantics; SpriteBlend modes
  5–10 and SpriteLayer are separate MilkRust extensions. SpriteSpeed, saved alpha
  and burn modifiers are also labeled extensions. Key-before-mip static-image
  color keys and the GIF-specific no-key/no-mip path are implemented; reference
  pixels and physical-renderer qualification remain open.
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
  2D/3D noise uses the reference texture dimensions and quality-specific X/Y/Z
  cubic expansion order. Built-in noise remains single-level as in MilkDrop;
  each renderer supplies clock entropy, and the shared generator reseeds and
  shuffles rows/slices. A deterministic entry point remains available to offline
  callers; per-binding filtering/wrapping is also present. q registers, frame uniforms, texture sizes
  and random/roam vectors are bound. Blur pixel parity and generated-noise pixel comparison remain open: the
  reference uses its process CRT random stream and performance-counter samples,
  while MilkRust now uses portable renderer-local clock entropy and per-row
  reseed/shuffle;
  missing resources and compiler/driver errors do not become a placeholder.
- A bounded `.milk2` wrapper parser, round-trip metadata preservation, two
  independent states, all 30 saved-pattern formulas with per-fragment 64x48
  triangle interpolation from the reference's clamped, truncated 8-bit vertex alpha, and the documented sprite
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
  same-origin URL or use the blob-worker fallback. Animated GIF, APNG and animated WebP use bounded shared Rust decoders
  in native and WebGL: full-canvas compositing, disposal, per-frame delays and
  sprite-local or preset-time selection, capped at 120 frames and 16 Mi decoded
  canvas pixels per asset. Netscape/AnimExts, APNG play counts and WebP loop
  counts control finite, single-play or infinite playback consistently across
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
  objects, border suppression, the five upstream sprite blend modes, optional
  engine extensions/filtering and layer ordering. These are source paths pending reference-image comparison.
  The typed HLSL
  profile now lowers common `Texture1D`/`Texture2D`/`Texture3D` sampling methods and direct
  resource parameters/fields, including the static offset overloads supported
  by GLSL ES 3.00, offset `Load` with defined out-of-range zero results, and
  base/mip-level `GetDimensions` with full mip-chain level counts. `Texture1D`
  maps to the center row of a 2D image and needs a one-row asset for 1D parity.
  It still rejects texture types outside those three dimensions, integer/non-color texture
  element types, resource arrays in function scopes or nested resource-bearing
  structs, initialized resource instances, and resource-bearing struct
  parameters with local/dynamic arguments. Direct resource-bearing struct helper
  parameters specialize only when their argument is an unambiguous global
  struct instance. Comparison methods outside the bounded Texture2D
  `GatherCmp`/`GatherCmpRed`/`GatherCmpGreen`/`GatherCmpBlue`/`GatherCmpAlpha` and Texture1D/Texture2D
  `SampleCmp`/`SampleCmpLevelZero` paths remain unsupported. Static `SamplerComparisonState` declarations lower supported
  comparison filters, all eight comparison functions and U/V/W
  Wrap/Clamp/Mirror modes. GatherCmp component variants compare four selected-channel texels
  in D3D order and accepts an optional literal int2 offset. SampleCmp and
  SampleCmpLevelZero compare before filtering. SampleCmp selects point/linear
  minification or magnification filters from derivatives and applies point or
  linear mip selection. Both require a static comparison state; other texture
  dimensions beyond Texture1D/Texture2D and reference-pixel parity remain open.
  The five ordinary `Texture2D.Gather` methods also read the four
  base-level texels in D3D order; reference-pixel comparison remains open.
  Global static `SamplerState` definitions
  support all eight standard point/linear min/mag/mip `Filter` combinations and
  independent U/V/W Wrap, Clamp or Mirror addressing. A global static state may
  alias another initialized state, including a comparison state, with its full
  profile preserved. Anisotropic filtering, border addressing, dynamic state
  expressions, and custom local/struct state initialization remain outside the
  profile. Global legacy effect-file `sampler_state` blocks
  bind one `Texture = <identifier>` and support independent Point/Linear min and
  mag filters plus None/Point/Linear mip filtering and per-axis Wrap/Clamp/Mirror.
  Direct `tex*` first arguments and statically specialized resource helpers are
  supported. Grouped global declarations retain independent initializers. State
  assignment remains an explicit failure.
  Dynamic array dispatch is limited to side-effect-free indices, supported
  sampling/load methods, standalone `GetDimensions`, and legacy `tex*` calls
  with the array element as their first argument. Direct `SamplerState` fields
  are accepted as method arguments but use resource prefixes. Global initialized
  states lower through per-resource aliases; direct helper parameters preserve static global state through bounded specialization;
  dynamic or assigned struct-field state propagation is not implemented. The compiler remains a
  bounded pixel-shader profile, not all HLSL shader models. The separate WGSL
  path remains limited.
- Complete format/EEL/texture behavior for every corpus file, including other legacy
  encodings, other executable extensions and unsupported formats. The current
  community-tree inventory found no unhandled text encoding or equation call,
  but CLI parser/shader analysis and missing external texture-pack accounting
  remain unrun. Additional unsupported historical image formats remain explicit import failures.
- Desktop helper process containment is implemented in PlezyNG PR #82, with
  fault-regression sources and platform packaging; execution is unverified.
  Mobile/TV local-driver and device-wide GPU hang containment, browser-process OOM
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
blocks preserved but inert. MilkDrop's Created/version and URL preamble is
accepted and retained; duplicate header records use their last value, and
numeric fields consume the compatible decimal prefix. Both import and direct
saved-blend validation apply the same sprite rule, including BOM-prefixed
wrapper headers. The change follows the pinned compatible source in Lactea's
MIT-licensed parser. No build, test or reference capture has run yet.


## 2026-10-30 real-file MilkDrop 3 header and fallback semantics

MilkDrop's free-form Created/version and URL preamble is accepted and retained.
Repeated key/value header records use the last value, and `blending_progress`,
`blending_direction`, and random fields consume the Windows CRT-compatible
numeric prefix. The app loader recognizes a `.milk2` wrapper only when all four
envelope markers exist; incomplete wrappers fall back to ordinary preset
parsing. Direct wrapper parsing remains strict. No build, test or reference
capture has run.


## 2026-10-30 saved-mask reference controls and sprite-header consistency

The saved-mask generator now accepts its grid dimensions so the 64x48-cell
production field and a small reference fixture execute the same formula code.
A committed unit control checks sampled (a,c) values for all 30 patterns against
the published Lactea 4x2 reference table, including the four direction-sensitive
patterns and the non-square horizontal case. This checks the mask formulas; it
is not an official MilkDrop3 renderer capture or pixel-parity result.

The .milk2 sprite gate now uses the last duplicate sprite value in the
wrapper header, consistent with the saved-header parser, and ignores sprite
records inside either preset body. Classic .milk keeps its first-value,
default-on behavior. Regression sources cover both branches; the private build
and test workflow has not produced a run for these commits.


## 2026-10-30 classic sprite disable record

The classic `.milk` gate now scans ordinary preset records through `[preset00]`
while ignoring records inside saved sprite blocks. The first standalone
`sprite` record controls the gate; no record keeps the classic default-on
behavior. The `.milk2` gate remains limited to the wrapper header and uses its
last duplicate value. Extraction-level regression sources cover both rules;
they have not run.


## 2026-10-30 saved-pattern vertex alpha fidelity correction

MilkDrop computes `clamp(a * progress + c, 0, 1)` at each saved mesh
vertex, truncates `mix2 * 255` to the D3D9 color alpha byte, then interpolates
those normalized bytes across the 64x48 triangles. The previous packed 32-bit
field retained precision the reference path discards. Core now emits a 65x49
R8 field, and native GL and WebGL sample it through normalized `sampler2D`
lookups before matching triangle interpolation. A source control checks the
byte count and clamp/truncation endpoints; private build, execution and
reference-image comparison remain pending.


## 2026-10-30 raw finite MilkDrop 3 header values

`.milk2` progress and random registers now preserve all finite values representable as
the format's float, including values outside [0, 1], through parsing, carry state,
render validation, and serialization. The saved-mask path clamps only
`a * progress + c` at each vertex; cosine variable blending and the two scene
weights use raw progress. Non-finite fields and float overflow in input are rejected; an overflowing
per-vertex mix saturates through the reference's ordered clamp, while a NaN mix
fails closed. Regression sources cover raw positive and
negative values, state carry, serialization, and mask endpoints; no build,
test, or reference capture has run for this change.


## 2026-10-30 saved-preset host inputs

MilkRust now exposes the standard `mousex`, `mousey`, `mousedown`, and `mouseclick` EEL host variables while retaining its existing underscore-style input aliases. A runtime samples one rising-edge `mouseclick` for all active preset scopes. The frame-clock fallback also supplies `year` through `totalseconds` (zero calendar fields, derived time-of-day). MilkDrop 3 cosine-mixes mouse positions and clock fields and snaps button values at its shader-dependent threshold; the temporary blend values are restored after scene generation. Source regressions cover input binding, click edges, interpolation and snap behavior; they have not run.


## 2026-10-30 per-preset runtime clock

Stateful single-preset and frame-set runtimes now anchor EEL `time`, frame count,
and embedded sprite playback at each source load. Both halves of a saved
`.milk2` start at the same zero point while retaining independent equation
state; frame-rate estimation continues to use host-clock deltas. Regression
sources cover initial time/frame values, successive frames and source changes.
They have not run. The calendar fields (`year`, `month`, `day`, `weekday`) remain
zero unless a host supplies local clock state. MilkRust exposes validated
`MilkRustClockState` injection in its runtime, native C ABI, and WebAssembly API.
PlezyNG's native adapter supplies the device's local clock; the browser SDK
refreshes it from the user's local `Date` before each frame. Direct WASM/stateless
callers that omit the setter retain zero calendar fields and the deterministic
preset-relative fallback.


## 2026-10-30 native mouse host input

The native C ABI now accepts validated per-frame pointer state and latches rising button edges through one successful render, including a down/up pair between frames. Coordinates are normalized and clamped to [0, 1], button state must be 0 or 1, and non-finite inputs fail. Native renders pass the stored input into MilkRust's shared equation runtime, so the existing `mousex`, `mousey`, `mousedown`, and rising-edge `mouseclick` bindings are available to a host. The shared PlezyNG adapter and Flutter visualizer now route pointer state from inline and fullscreen surfaces through Android, Apple, Linux, and Windows hosts. Coalesced motion deltas accumulate until the next rendered frame, then are consumed once. Quick button-down edges are latched for one rendered frame so short taps still raise `mouseclick`; pausing clears held or pending input. Native regression execution and device input checks remain pending. The browser SDK also validates/clamps pointer fields, accumulates motion deltas, and latches short button-down edges until the next render.

## 2026-10-30 static resource helper specialization

Resource and sampler-state helper inputs now specialize at global bindings before
resource lowering. Nested calls retain static point/linear/address profiles and
legacy `sampler_state` bindings, while ordinary arguments stay single-evaluation
function inputs. Literal-indexed global resource arrays, direct fields and forward
prototypes use the same path. Expansion is capped at 64 instances and 512 KiB;
recursion, ambiguous overloads, dynamic opaque inputs and identifier capture fail
explicitly. A sampler declaration loop also now declares its changing cursor
mutable, correcting a source-level Rust compilation error. Native and browser
regression sources were added; compiler and pixel execution remain unrun.

PlezyNG's desktop containment source is committed at
`eb8e9f857826cab1e8d1a538eb423b933a3bfeb2`: Linux MilkRust/projectM and Windows/macOS
MilkRust run inside packaged helpers with bounded native IPC and process
termination. Mobile/TV local drivers and device-wide GPU/kernel failure remain
separate limits. This is not a claim of tested recovery or universal completion.


## 2026-10-30 classic noise texture quality generation

Built-in noise textures now use the compatible physical dimensions, random-value ranges,
packed color layout, and separable cubic interpolation order from MilkDrop's
`AddNoiseTex` and `AddNoiseVol` paths. Medium/high 2D noise expands in X then Y;
high 3D noise expands in X, Y, then Z. Renderers now provide runtime-clock entropy, and the shared generator reseeds
and shuffles each row/slice before the existing quality-specific interpolation.
The deterministic wrapper remains available to offline callers. MilkRust uses a
portable xorshift stream rather than the reference process's CRT rand and
performance-counter sequence, so generated pixels are not claimed bit-identical. Builds and GPU
reference captures remain pending.


## 2026-10-30 bounded APNG decoding

Native PNG decoding now uses the shared Rust decoder; WebGL static PNG remains
on its disposable browser worker, while animated PNG uses the same bounded Rust
decoder on both hosts. APNG inspection validates the declared frame count,
canvas/subframe bounds and decoded-pixel budget before decode. The compositor
implements full-canvas Source/Over blending, None, Background and Previous
disposal, frame delays, poster-frame exclusion and finite/single-play/infinite
play counts. The static-PNG check no longer scans compressed image bytes for an
`fdAT` substring.

The source path and dependency/license records are committed. Native/browser
decoder, upload, malformed-input and reference-pixel checks remain unrun, so
this does not establish APNG rendering qualification or pixel identity.


## 2026-10-30 bounded animated WebP decoding

Animated WebP is now inspected against canvas, frame-count and aggregate
decoded-pixel limits before the shared Rust decoder allocates frame images.
The Rust decoder composites full-canvas frames, preserves frame durations and
maps finite/infinite WebP loop metadata to the common animation clock. Native
hosts route static and animated WebP through the new ABI; WebGL routes animated
WebP through the same upload and timing path as GIF and APNG, while static WebP
remains in the disposable browser worker.

The decoder, ABI and import paths are source-complete. Decoder, upload,
malformed-input, disposal/loop and reference-pixel checks remain unrun; no
platform or visual qualification is claimed.

## 2026-10-30 grouped state and resource-helper overloads

Global texture/sampler groups now retain separate array extents and static state
initializers through normalization. Resource helpers support numeric overloads
and different arities; overloads of a given arity must have identical opaque
parameter types/positions. Numeric resolution remains in the typed compiler.
Forward declarations match the correct definition despite different parameter
names, nested and same-name overload forwarding preserve state, and numeric
arguments are evaluated once. A bounded typed call-graph pass resolves forward
declarations and rejects direct/indirect recursion before GLSL generation.
Recursion, opaque-layout ambiguity, mixed ordinary/resource overload sets
and existing binding/expansion limits still produce explicit diagnostics.
Native compiler and browser pixel regression sources are added and unrun.

PlezyNG PR #82 also implements private Android/Android TV services, packaged
Tizen/webOS helpers and iOS/tvOS lifecycle/interruption recovery. Mobile Apple
driver calls remain in-process; device-wide GPU/kernel faults and physical
recovery remain qualification limits. No private build or test run is recorded.


## 2026-10-30 bounded 1D/2D comparison-sampling source path

The typed compiler lowers all five Texture2D GatherCmp component variants, plus
Texture1D/Texture2D SampleCmp and SampleCmpLevelZero, for statically initialized
SamplerComparisonState declarations. Texture1D uses the existing center-row
mapping and requires a one-row asset for parity. The bounded comparison filter
supports point/linear profiles and all eight comparison functions with
Wrap/Clamp/Mirror addressing. GatherCmp variants compare the selected channel
from four base-level texels in D3D order and accept a literal int2 offset.
SampleCmp and SampleCmpLevelZero compare before point/bilinear filtering;
SampleCmp selects min/mag filters from derivatives and point/linear mip
selection. Texture1D comparison samples accept a scalar integer offset and
Texture2D comparison samples accept literal int2 offsets. Texture3D/cube
comparison methods and pixel-reference qualification remain open. No
compiler/build/test/render was run for this source update.

## 2026-10-30 Texture1D comparison-sampling continuation

Texture1D `SampleCmp` and `SampleCmpLevelZero` now lower through the existing center-row mapping, accept their scalar integer offset form and share the bounded comparison filter/LOD implementation with Texture2D. The source requires a one-row backing image for parity. Gather remains Texture2D-only; Texture3D/cube comparison calls remain explicit errors. No compiler/build/test/render was run for this source update.


## 2026-10-30 upstream sprite blend contract correction

The pinned MilkDrop 3 reference initializes the per-sprite EEL blendmode to 0,
then truncates its value to int and clamps it to 0..4 in DrawUserSprites. MilkRust
now evaluates that EEL value after each frame's sprite equations, so code can
change the standard mode dynamically. SpriteBlend and modes 5..10 remain an
explicit MilkRust extension, separate from the upstream compatibility claim.
The source behavior is in the upstream
[texmgr.cpp](https://github.com/milkdrop2077/MilkDrop3/blob/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2/texmgr.cpp)
and [milkdropfs.cpp](https://github.com/milkdrop2077/MilkDrop3/blob/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2/milkdropfs.cpp).
Regression source covers default 0, dynamic assignment, fractional truncation,
and under/overflow clamping. Source changes are unbuilt; private JIT and pixel
qualification remain outstanding.
