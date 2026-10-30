# HLSL compiler and rendering profile

The native GL/GLES and WebGL2 paths now have a typed legacy HLSL compiler:
`milkrust-hlsl` vendors Thekla HLSLParser at
`4f4aefdc52ec49910a7542da4e772b9a0c246d7b` under MIT. The original expression
translator remains a fast path and the separate WGSL path remains restricted.
This source integration is not a build, device qualification, or universal
community-preset compatibility result.

## Implemented source

- A safe Rust API around one exception-contained C++ ABI; the core crate keeps
  `unsafe_code = forbid`. Native builds compile the pinned source with `cc`.
- The browser builds the same source using pinned Emscripten 3.1.64 and loads
  its ES module before shader imports. `tools/build-wasm-package.sh` includes
  the compiler JS/WASM, license, and provenance in the package.
- Token-based object/function macros, conditional compilation, `defined`,
  integer expressions, continuation lines, token pasting, and nested arguments.
  Includes and filesystem/network access are unavailable.
- Typed functions, overloads, structures, arrays/indexing, scalar/vector/matrix
  expressions, implicit conversions, if/else, for/while/do loops, switch/case,
  break/continue, discard and legacy sampling intrinsics. Local fixes cover
  explicit block emission, empty/expression loop initializers, bare returns,
  const globals and HLSL `fmod` sign semantics.
- MilkDrop `shader_body` wrappers with helper preambles, and explicit pixel
  entry points with TEXCOORD0 UV pairs, TEXCOORD1 polar coordinates and
  COLOR0 hue input/output. Unknown input/output semantics fail.
- Separate warp/composite passes, three separable blur levels, deterministic
  original 2D/3D noise resources, mipmap generation, and per-binding sampler
  filtering/addressing on native GL/GLES and WebGL2. Noise and blur algorithms
  are independent implementations and still need reference comparisons.
- Scalar frame values, q1–q64 and packed q vectors, texture dimensions, aspect,
  deterministic preset/frame randoms and roaming vectors shared by both hosts.

## Limits and errors

Source is bounded to 128 KiB at import, 256 KiB after preprocessing; tokens,
macro expansions, conditional depth, compiler recursion, cumulative allocation,
operations and elapsed compilation time are bounded. Array declarations require
constant one-dimensional bounds of 1–4096 elements; integer overflow and division by zero in
constant evaluation return errors across the ABI. Diagnostics are capped at
8 KiB and compiler output at 1 MiB (renderer input remains 256 KiB). A bounded
per-thread cache avoids repeat native compilation. GPU loop execution shares a
16,384-iteration counter per fragment and discards fragments exceeding it.
These limits do not sandbox a graphics driver or certify arbitrary GPU code.

## Macro stringification

Function-like macros can stringify a parameter with `#`. The preprocessor keeps
token-boundary whitespace through macro argument capture, trims the argument's
outer whitespace, collapses internal whitespace runs to one space, and escapes
quotes and backslashes in the resulting string token. Stringification cannot be
combined with `##`; malformed uses fail. `#error` arguments receive ordinary
macro expansion before the bounded diagnostic is returned. Token expansion and
stringified output share the existing depth, token and 256 KiB byte limits.

Unknown semantics, unsupported resource kinds, include files, unsupported
shader models and remaining emitter gaps fail rather than becoming a fallback
scene. This is a legacy pixel-shader profile, not all modern
DirectX HLSL. The WGSL path does not inherit the new compiler. Exact reference
parity for noise, blur, uniforms and `.milk2` patterns remains unqualified.

## Private JIT acceptance

No build or test was executed during this source-only implementation. Run the
crate tests, native host/GL corpus and browser WASM compiler corpus on approved
private ephemeral JIT runners. Desktop contexts require OpenGL 3.3 (macOS selects 4.1); GLES hosts require ES 3.
Native cross builds now require a target C++11
compiler and runtime in addition to Rust. Browser runners require Emscripten
3.1.64. Preserve every parser, driver, black-frame and image-difference failure
in the qualification report. Do not promote broader packs based only on source
compilation or catalog schema validation.

## MilkDrop environment bindings (2026-10-29 source update)

The native and WebGL hosts now bind all 24 `rot_s/d/f/vf/uf/rand1..4`
`float4x3` matrices as column-major GLSL `mat3x4`, including the fourth-row
translation. Their deterministic random stream is seeded per preset; animated
families follow the legacy `Rx * translation * Rz * Ry` composition and rate
curve. They are not expected to reproduce another process's random seed.

The typed path supplies `_c0.._c13`, packed q vectors, mip aliases, the four
MilkDrop PI spellings, `GetMain`, `GetPixel`, `lum`, lowercase sampling aliases,
and uppercase sampler addressing prefixes. `sampler_fc_main` now keeps its
own clamp binding. Roaming vectors use the legacy rates and phases; `rad` is
normalized to the aspect-corrected screen corner and `ang` wraps to `[0, 2pi)`.
Shaders using these names bypass the older scalar-expression path.

Blur levels normalize their encoded range and `GetBlur1..3` decode the same
range in both hosts. Nested/reversed ranges are made finite and nondegenerate;
this deliberately avoids the divide-by-zero typo in historical min/max repair.
The current Gaussian kernel and generated noise still require reference-image
comparisons. These changes supply missing environment behavior, not evidence
that every legacy shader is visually conformant. New regression sources are
unrun under the source-only instruction.

Algorithm/reference: `jecassis/foo_vis_milk2`, `vis_milk2/milkdropfs.cpp`
(`ApplyShaderParams`, `UvToMathSpace`) and `state.cpp` (`RandomizePresetVars`),
plus MilkDrop's `include.fx`. The implementation here uses its own seeded RNG
and independently expressed matrix arithmetic.

## Preset text decoding

Native `milkrust_load`, the CLI and the browser pack loader accept UTF-8 (with
optional BOM), BOM-marked UTF-16LE/BE and legacy Windows-1252. The encoded and
UTF-8-decoded budgets are each 2 MiB. Malformed surrogates, undefined code-page
bytes and binary controls fail explicitly. The CLI records the chosen legacy
encoding without removing the file from its denominator. A decoding failure
remains a separate failed entry. Other historical encodings are not guessed.
PlezyNG validates the same encodings before its include/path checks and retains
original archive bytes, preserving integrity receipts. New decoder regressions
have not been executed during this source-only continuation.

## Shared fragment state and texture bindings

Helpers and `shader_body` now share `uv`, `uv_orig`, `x`, `y`, `rad`, `ang`,
`ret` and `hue_shader`. Built-in shader registers are initialized into writable
per-fragment storage before calling the user entry. `q1..q64` alias the matching
components of `_qa.._qp`, so assignments through either spelling stay coherent.
The original screen UV remains separate from warped feedback coordinates.
Four animated corner hues are interpolated from the original UV for both hosts.
Explicit legacy entry points accept float4 TEXCOORD0 (warped/original UV),
float2 TEXCOORD1 (radius/angle), and float3/4 COLOR0 (hue).

Named samplers resolve to the underlying asset filename when the native host
loads dependencies. Both renderers bind actual `texsize_<asset>` dimensions,
including dimension-only references; `texsize_main` aliases the feedback size.
Texture uploads use consistent bottom-up GL orientation. Program-local uniform
location caches avoid repeating hundreds of driver queries each frame. Added
compiler and browser integration cases are source only; no execution or
reference-image result is implied.

## Embedded browser textures

The WebGL path validates MIME, strict base64, encoded size and raster dimensions
before creating a browser codec input. PNG/JPEG/BMP/WebP decode inside a
disposable Web Worker; an 8-second timeout terminates a stalled worker, decoded
dimensions are checked again, and the transferred ImageBitmap is uploaded without
copying its pixels through the renderer thread. The web package stages a static
worker and uses it when it shares the page's origin, allowing strict CSP
deployments to avoid `blob:`; a rejected static worker falls back to the blob
worker when policy permits. Direct wasm-bindgen users can configure a same-origin
URL or use the bounded blob-worker fallback. Legacy DDS
(DXT1/3/5 and masked RGB24/32) and TGA (truecolor/grayscale, raw/RLE) use
bounded Rust decoding and direct RGBA upload. Both paths use 4096-per-side,
8-megapixel-per-image and 32-megapixel-per-preset limits. Pixel capacity is reserved
before asynchronous loads and returned on disposal, including upload failures.
DDS/TGA rows are oriented consistently with native uploads, and named samplers
receive actual dimensions. EXIF dimension swaps preserve the same pixel budget.
GIF uses the shared bounded Rust animation decoder; animated PNG and WebP are
rejected by the static texture path.
Worker termination cannot contain browser-process OOM or native codec work that
escapes the worker, and it does not isolate the graphics driver. CPU and browser
upload regression sources have been added but not executed.

The WASM engine owns removable context-loss/restoration listeners. It requests
browser recovery, pauses equation/frame work while lost, and recreates all GPU
handles and textures on the first frame after restoration. Preset text, assets
and CPU equation state survive; feedback history restarts because the old GPU
contents no longer exist. SDK callers receive a recoverable waiting state and
a clearing update. The private browser integration source now exercises actual
`WEBGL_lose_context` restoration and nonblank rendering; it remains unrun.

## User global initializers

The typed compiler now distinguishes the generated host environment from user
preamble globals through a reserved AST marker. User numeric/struct/array globals
are writable per-fragment storage, with initializers evaluated after coordinates,
hue and host registers have been populated. Uninitialized user globals are zeroed,
including bounded arrays and struct members. This avoids illegal GLSL uniform
initializers and unbound custom variables. Provable integer constants retain
constant array-bound semantics; ordinary HLSL local readonly values are not
incorrectly forced into GLSL compile-time constants. The additional compiler and
browser regression sources have not been executed.

## Numeric lowering checkpoint

The typed parser and both renderer compiler builds now share numeric shape
rules. Vector comparisons truncate to the common width and convert component
types; vector logical operations and integer/unsigned/Boolean conditionals use
component selection. `~` is bitwise complement. Matrix constructors consume
HLSL row order, scalar-to-matrix conversion fills every component, and `*`/`*=`
are componentwise. Float/half matrices support every 1×1 through 4×4 shape;
integer matrices support dimension-compatible `mul` and `transpose`; Boolean
matrices support `transpose`; and Boolean, signed, and unsigned matrix values
use typed row storage for componentwise operators. One-row and one-column
matrices preserve their HLSL shape while lowering to GLSL scalar/vector
storage. `mul` uses HLSL row/column-vector rules and all legal shape products.
Rectangular row and column bounds are distinct. These rules follow Microsoft's
legacy [`mul`](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-mul)
and [`transpose`](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-transpose)
type tables; MilkRust's half-matrix overloads are retained as an extension.

Matrix element masks and indexed rows support reads, assignments, compound
updates and prefix/postfix increments. Generated helpers evaluate the matrix
and indices once; repeated write destinations and invalid/mixed naming sets
are rejected. Function `out`/`inout` arguments use local copy-in/copy-out
storage when a matrix selection or type conversion requires it. Index values
are retained across the call, and disjoint outputs into the same matrix are
written back individually.

The authored `tests/fixtures/numeric.hlsl` control checks numerical results
through the browser integration entrypoint, including non-symmetric matrices,
swizzle writes, side effects, aliased matrix outputs and converting `inout`
arguments. Compiler rejection cases accompany it. These sources have **not
been compiled or executed** in the source-only continuation. The checkpoint
closes these specific emitter gaps; it is not a claim of all HLSL models or
reference conformance. Rules were checked against Microsoft's
[per-component math documentation](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-per-component-math)
and [conversion specification](https://microsoft.github.io/hlsl-specs/specs/4-Conv.html).

Boolean, signed and unsigned multi-row/multi-column matrices use typed GLSL row
structures. Conditional selection preserves the GLSL ES scalar-Boolean
short-circuit rule for both native matrix types and those row structures.
One-row and one-column forms lower to scalar/vector storage.

## Legacy operators and intrinsics checkpoint

Remainder, shifts and their compound forms now reach the parser/emitter with
unsigned decimal/hex literals and checked constant operations. Cast precedence
and nested conditionals are explicit. The lexer rejects oversized identifiers
before copying, rejects overflowing numeric literals, and prints finite floats
with round-trip precision and a fixed decimal locale; small shader constants
are no longer rounded to zero by six-decimal formatting.

Legacy intrinsic lowering includes reciprocal/reciprocal-square-root,
multiply-add, any/all, ties-to-even round, truncation, logarithm-base-ten,
finite checks, bit reinterpretation, and scalar/vector/matrix component math.
`modf`, `frexp` and `sincos` declare their output parameters correctly. `frexp`
uses an ES 3.0-compatible exponent/mantissa split, including a subnormal
normalization path. User-defined overloads remain separate from intrinsics.

The numerical `intrinsics.hlsl` browser control and native compiler cases are
committed but unrun. Driver handling of subnormals remains target-dependent;
these implementations do not turn the source-only checkpoint into executed
conformance evidence. Microsoft's intrinsic index and individual function
contracts are the semantic references, not another renderer's source code.

## Rescan and sampling checkpoint

Macro suppression now travels with individual tokens, and replacement tokens
rescan together with the remaining input. Object aliases and returned function
macros can consume the following invocation without re-expanding recursive
names. Unused arguments are not expanded, and empty pasted arguments use
placemarkers. Token lookahead uses byte slices so non-ASCII input adjacent to
operators returns a diagnostic instead of slicing inside UTF-8. The known
`pack_matrix` pragma is accepted because matrices use logical values and no
user-provided raw constant-buffer layout; unknown pragmas still fail.

Legacy volume projected/bias/gradient samples, explicit gradient overloads,
texture dimensions and texel fetch have GLSL ES paths. Constant 2D LOD offsets
are emitted at the call site, retaining the offset's constant-expression status
and evaluating coordinates once. Dimension-only sampler references are included
in resource discovery. Native compiler, preprocessor and browser source cases
are added; no execution is claimed.

## Declarations and constant expressions checkpoint

The parser accepts scoped `typedef` aliases (including bounded array aliases),
`vector<T,N>` and `matrix<T,R,C>` spellings for supported numeric shapes, their
float defaults, and scalar-width-one names. `unsigned`/`unsigned int` and
`dword` aliases map to `uint`. It reads numeric/struct brace
initializers, nested matrix rows, inferred array extents, grouped global/local
declarations, `row_major`/`column_major`, and positive constant `[unroll(n)]`
hints. Arrays keep their own declarator shape instead of leaking it into the
next comma-separated variable.

Constant evaluation resolves each identifier to its lexical declaration, so
local and shadowed constants work in bounds. Casts, scalar constructors,
conditionals, uint arithmetic and finite float32 intermediates are typed;
signed overflow, invalid shifts, cyclic definitions and nonconstant bounds
fail within the compiler's existing budgets. GLSL array extents are emitted as
validated integers, independent of whether a readonly local is a GLSL constant.
The `declarations.hlsl` pixel control and malformed-input cases are authored
but unrun. Fixed arrays have one dimension; array aliases preserve their extent,
and arrays inside structure fields remain supported. A second dimension,
including one appended to an array typedef, is rejected because [ESSL 3.00
forbids arrays of arrays](https://registry.khronos.org/OpenGL/specs/es/3.0/GLSL_ES_Specification_3.00.pdf).
Common `Texture1D<T>`, `Texture2D<T>` and `Texture3D<T>` declarations with `float`, `half`,
`min16float`, `min10float`, or their 1–4 component vector forms lower to the
renderer's legacy RGBA sampler bindings. Sampling and `Load` results keep the
declared component width. Minimum-precision scalar, vector and matrix aliases
(`min16float`, `min10float`, `min16int`, `min12int`, `min16uint`) widen to the
matching `float`, `int` or `uint` types; their minimum precision is not retained.
`Sample`, `SampleLevel`, `SampleBias`, `SampleGrad`, `Load`, and both base-level
and mip-level `GetDimensions` lower to matching legacy operations. `Load`
honors its optional static integer offset and returns zero for invalid mip or
texel coordinates, matching the object-method contract. The mip-level
`GetDimensions` overload also reports the complete mip-chain length; both hosts
generate full mip chains for these bound resources. Static integer offsets on
`Sample`, `SampleLevel`, and `SampleBias` use GLSL ES 3.00 offset lookups; the
offset overload of `SampleGrad` uses GLSL ES 3.00 `textureGradOffset`.
Direct texture parameters and direct texture fields in global structs are
supported; fields bind as `sampler_<object>_<field>` unless their name already
begins with `sampler_`. Declared `SamplerState` names and direct declared
`SamplerState` fields are accepted as method arguments, while filtering and wrapping still come from
MilkDrop sampler prefixes. Sampler-state expressions and custom state
definitions remain unsupported. `Texture1D` sampling lowers to a 2D lookup at
the center of the bound image row; use a one-row asset when the original shader
expects true 1D storage. Global `Texture1D` and `Texture2D` arrays with
decimal literal bounds from 1 through 15 lower to separately named bindings
when each access uses a constant decimal index and a supported resource method.
Their image assets use `<resource>_<index>` names (for example, `textures_0`).
`TextureCube`, `Texture*Array`, multisample resources, `Texture3D` arrays,
dynamically indexed or grouped texture arrays, arrays inside structs or
function scopes, initialized resource instances, nested resource-bearing
structs, and resource-bearing struct parameters remain outside the profile. The separate
WGSL path remains limited; within its floating scalar/vector expression subset,
HLSL `fmod` now lowers to WGSL's signed remainder operator. Integer/boolean and
the separately rejected sampling operations still stay outside that path.

Semantic references: Microsoft's [minimum-precision types](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/using-hlsl-minimum-precision), [vector types](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-vector),
[user-defined types](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-user-defined),
[loop attributes](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-for),
[texture `Load`](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-to-load)
and [`GetDimensions`](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx-graphics-hlsl-to-getdimensions).
Offset lookup lowering follows the [OpenGL ES 3.00 shading language](https://registry.khronos.org/OpenGL/specs/es/3.0/GLSL_ES_Specification_3.00.pdf).

## Identifier and indexed-selection checkpoint

Before GLSL generation, a bounded AST pass gives colliding user declarations,
struct types/fields, arguments, and references unique names. It protects GLSL
keywords, intrinsic names, reserved prefixes and double-underscore spellings;
HLSL intrinsic call objects keep their original identity. Ordinary value
identifiers no longer pass through the intrinsic-function mapper, so variables
named `frac`, `clip` or `tex2D` remain variables. Entry-point lookup follows the
same renaming. The name rules follow GLSL ES 3.00 sections 3.8–3.9.

Dynamic matrix component reads now allow scalar replication and chained
swizzles, with indices evaluated once. Single-component writes remain writable;
repeated destinations still fail. Boolean scalar swizzles have typed helpers.
The identifier pixel control and extended numeric/rejection cases are source
only and have not run.

## Readonly initializer lowering

HLSL `const` and `static const` declarations no longer force GLSL `const` when
matrix constructors, scalar swizzles or helper calls make their lowered
initializer a runtime expression. Those globals initialize in declaration order
inside the generated fragment entry, after the MilkDrop adapter supplies the
shared environment. Locals retain their normal declaration location. Direct
scalar/vector constant expressions retain GLSL const, including named texture
LOD offsets, while array bounds are independently folded from the HLSL AST.
Deferred array initializers convert each element to its declared type. The
existing declaration and sampling controls now exercise these cases; they
remain unrun.


## Legacy aggregate initialization and casts

Brace initializers now flatten all numeric source values, regardless of brace
nesting, and fill vectors, matrix rows, array elements and struct fields in HLSL
order. Array extents can be inferred from the flattened element count. Struct
fields may themselves contain arrays or structs. Initializers require exactly
the destination's scalar count; they do not use the later C++-style partial
zero-initialization proposal. Every expanded source expression is captured once
before its components are used. Numeric scalar/vector constructors keep a direct
constant-expression path for named texture offsets.

Explicit aggregate casts support numeric scalar splats (including `(Struct)0`),
array/struct flattening and truncation, and per-component scalar conversion.
Same-type struct/array copies preserve their type instead of generating an
invalid GLSL scalar constructor. Aggregate expansion is limited to 4096 scalar
components, in addition to the existing depth, operation, temporary and output
budgets. Single-dimensional arrays and direct global sampler fields in structs
are supported within the compiler budgets. Direct `Texture2D`/`Texture3D`
float/half scalar and vector declarations and parameters lower through the same
sampler model. Sampling methods support the documented static integer offsets;
`GetDimensions` accepts the base-level and mip-level output forms, with the
NumberOfLevels result based on the full mip chain generated by both hosts.
`SamplerState` method arguments may be declared names or direct declared struct fields;
MilkRust does not emulate per-object filter/address state. Dynamically indexed
or grouped texture arrays, `Texture3D` arrays, arrays inside structs or
function scopes, initialized resource instances, nested resource-bearing
structs, resource-bearing struct parameters, non-2D/3D texture kinds and
custom sampler-state definitions remain unsupported.
`aggregates.hlsl` covers nested fields, arbitrary brace grouping,
row-major reconstruction, inferred arrays, copies, scalar splats and
single-evaluation side effects; the native and browser cases are unrun.

Semantic references: Microsoft's [legacy aggregate rules](https://microsoft.github.io/hlsl-specs/proposals/0005-strict-initializer-lists/)
and [elementwise/splat conversions](https://microsoft.github.io/hlsl-specs/specs/4-Conv.html).


Constant array bounds now also resolve vector swizzles, matrix masks/row
indices, array elements and nested struct fields, including flattened aggregate
initializers and typed component-wise arithmetic. Readonly selections propagate
their qualifier; bounds still require provable values. Out-of-range constant
indices, cycles, overflow and expansion limits produce diagnostics. Direct
constant vector swizzles remain GLSL constant expressions for texture offsets.
The aggregate fixture and invalid-input cases include these paths; they have
not been executed.

Assignments, increment/decrement, and `out`/`inout` arguments now validate that
the destination is a writable variable or selection. Readonly struct fields,
array/vector/matrix selections and `const` parameters remain protected even
when their GLSL declarations need runtime initialization. Dedicated invalid
write cases are included in the unrun native suite.
