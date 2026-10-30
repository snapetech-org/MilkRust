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
range in both hosts. The blur filter now uses MilkDrop's paired-tap horizontal
and vertical weights, source-texel offsets, nested range transforms and first-
level edge darkening. Nested/reversed ranges are made finite and nondegenerate;
this deliberately avoids the divide-by-zero typo in historical min/max repair.
Built-in 2D/3D noise now uses MilkDrop's physical dimensions, quality-specific
cubic expansion and one-level texture setup in native GL and WebGL. Each renderer now supplies runtime-clock entropy, and the generator reseeds and
shuffles each row/slice while preserving a deterministic entry point for offline
tools. Its portable xorshift stream is not the reference process's CRT rand and
performance-counter sequence, so generated noise still requires reference-image
comparison. Blur and full shader pixel parity remain open; source implementation
is not visual evidence.
New regression sources are unrun under the source-only restriction.

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
but unrun. Numeric/aggregate arrays support up to 16 dimensions and 4096 total
array elements. Inner dimensions lower to canonical structs containing one
array, preserving row slices, indexed writes, typedefs, grouped declarations,
aggregate casts, defaults and function parameter/return types without emitting
an ESSL array-of-arrays. Only the outer extent can be inferred. Array element
resources use the separate bounded binding profile below.
Common `Texture1D<T>`, `Texture2D<T>` and `Texture3D<T>` declarations with `float`, `half`,
`min16float`, `min10float`, or their 1–4 component vector forms lower to the
renderer's legacy RGBA sampler bindings. Sampling and `Load` results keep the
declared component width. Minimum-precision scalar, vector and matrix aliases
(`min16float`, `min10float`, `min16int`, `min12int`, `min16uint`) widen to the
matching `float`, `int` or `uint` types; their minimum precision is not retained.
`Sample`, `SampleLevel`, `SampleBias`, `SampleGrad`, `Load`, and both base-level
and mip-level `GetDimensions` lower to matching legacy operations. The five
`Texture2D.Gather`, `GatherRed`, `GatherGreen`, `GatherBlue`, and
`GatherAlpha` methods return the four base-level texels in D3D component order.
They use explicit LOD-zero reads so GLSL ES 3.00 can preserve address modes while
ignoring min/mag filtering as required by Gather. `Load`
honors its optional static integer offset and returns zero for invalid mip or
texel coordinates, matching the object-method contract. The mip-level
`GetDimensions` overload also reports the complete mip-chain length; both hosts
generate full mip chains for these bound resources. Static integer offsets on
`Sample`, `SampleLevel`, and `SampleBias` use GLSL ES 3.00 offset lookups; the
offset overload of `SampleGrad` uses GLSL ES 3.00 `textureGradOffset`.
Direct texture parameters and direct texture fields in global structs are
supported; fields bind as `sampler_<object>_<field>` unless their name already
begins with `sampler_`. Global `SamplerState` declarations with static
initializers accept all eight standard point/linear `Filter` enum combinations,
plus `AddressU/V/W = Wrap`, `Clamp`, or `Mirror`. Generated bindings preserve independent
min, mag and mip filtering and each address axis. Declared state names and direct
declared `SamplerState` fields remain valid method arguments; fields use the
resource's default prefix. Legacy effect-file `sampler`, `sampler2D`, and
`sampler3D` globals may initialize with `= sampler_state {}`. That subset
accepts `Texture = <identifier>` (or a `sampler_*` variable as a fallback),
independent `MinFilter = Point|Linear`, `MagFilter = Point|Linear`, and
`MipFilter = None|Point|Linear` properties, plus `AddressU/V/W = Wrap`,
`Clamp`, or `Mirror`. Unspecified filter fields default to linear; unspecified address fields
default to wrap. These legacy states lower to generated aliases when used as the
first argument to a supported `tex*` intrinsic. Grouped global declarations retain
each initializer independently. Dynamic state, assignment escapes, local or struct initializers, and
anisotropic filtering and border addressing remain unsupported. `Texture1D` sampling lowers to a 2D lookup at
the center of the bound image row; use a one-row asset when the original shader
expects true 1D storage. Global `Texture1D`/`Texture2D`/`Texture3D`
and legacy `sampler`/`sampler2D`/`sampler3D` arrays with literal bounds
from 1 through 15 lower to separately named bindings. Grouped resource
declarators and direct array fields on globally declared resource structs are
supported. Literal indices bind directly. Side-effect-free dynamic integer
indices dispatch over each binding for texture `Sample`, `SampleLevel`,
`SampleBias`, `SampleGrad`, all five 2D `Gather` methods, `Load`, standalone
`GetDimensions`, and legacy
`tex*` calls where the array access is the first intrinsic argument. A
dynamic index outside the declared bound returns zero (or zero dimension
outputs). Dynamic dispatch has a 512 KiB expansion budget and rejects function
calls, assignments and increment/decrement in index expressions. Direct assets
use `<resource>_<index>` names (for example, `textures_0`); struct-field assets
use `<object>_<field>_<index>`.
`TextureCube`, `Texture*Array`, multisample resources, arrays inside function
scopes or nested resource-bearing structs, initialized resource instances,
resource-bearing struct parameters and dynamic comparison-state propagation
remain outside the profile. Texture2D GatherCmp, GatherCmpRed, SampleCmp and
SampleCmpLevelZero use the bounded static comparison profile described below;
other comparison resource forms remain outside. The separate
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
budgets. Multidimensional numeric arrays, direct global sampler
fields, and resource arrays on global declarations or direct fields of
global resource structs are supported within the compiler budgets. Literal
indices select one named binding. Side-effect-free dynamic integer indices
dispatch supported sampling, load and dimension calls over bounded arrays;
legacy sampler arrays dispatch when used as the first argument of a supported
`tex*` intrinsic. Out-of-range dynamic indices yield zero. The expansion
budget is 512 KiB. Arrays in function scopes or nested resource-bearing structs,
initialized resource instances, resource-bearing struct parameters, non-2D/3D
texture kinds, anisotropic filtering, border addressing and dynamic sampler-state expressions
remain unsupported. Direct
`Texture2D`/`Texture3D` float/half scalar and vector declarations and
parameters lower through the same sampler model. Sampling methods support the documented static integer offsets;
`GetDimensions` accepts the base-level and mip-level output forms, with the
NumberOfLevels result based on the full mip chain generated by both hosts.
Global static sampler definitions bind settings through generated per-resource
aliases. Sampler parameters and struct fields retain the resource prefix and
do not propagate custom filter/address profiles.
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

## Multidimensional array layout — 2026-10-30

The typed AST retains HLSL rank and extents; the GLSL emitter uses one array
per generated row struct. Index expressions are emitted once, including an
`inout` row slice. Canonical names match complete inner shapes, so copies and
function boundaries agree. Types are emitted in dependency order before
helpers, and grouped declarations/loop initializers retain their own row type
and lexical scope. Constant bounds and total storage are checked before GPU
compilation. The `arrays.hlsl` control covers rows, three dimensions, arrays of
structs/matrices, inferred/typedef dimensions, casts, returns, defaults and
side-effecting indices. It is registered in native and browser compiler checks,
but remains unrun under the source-only restriction.

Representation follows ESSL 3.00 sections 4.1.8–4.1.9 and 6.1: arrays and structs
can contain one another, and explicitly sized arrays can cross function
boundaries. It does not enable opaque resource arrays through numeric layout.

## Static resource helpers

Helper functions with direct `SamplerState`, `SamplerComparisonState`, `Texture1D/2D/3D` or legacy
`sampler/sampler2D/sampler3D` input parameters are specialized at their global
bindings before texture methods are lowered when the source declares initialized
sampling state. Without initialized state, the existing typed opaque-parameter
path, including ordinary overloads, is preserved. Nested helper calls preserve the
actual texture and static state, including legacy `sampler_state` blocks. Ordinary
value arguments remain function arguments and are evaluated once. Global fields
and literal-indexed global resource arrays can be arguments; existing static
filter/address aliases and resource limits still apply. Forward declarations
retain the specialized signatures at their original source positions.

The pass permits at most 64 helper specializations and 512 KiB of generated
source. Opaque arguments must resolve to a global binding without side effects.
Resource helpers may overload their ordinary numeric parameters or use different
arities. For any one arity, their opaque parameter types and positions must agree;
the generated functions retain one overload name and the typed compiler selects
the numeric signature. Overloads may forward to another overload of the same
name. A bounded AST call-graph pass follows resolved function/forward-declaration
identities and rejects actual direct or indirect recursion before GLSL generation.
Prototypes match the corresponding definition regardless
of parameter names and remain at their original source positions. Mixed
ordinary/resource overload sets and overloads that change opaque types/positions
still fail explicitly. Recursive helpers, output/array/default opaque parameters,
local/dynamic opaque arguments, and substitutions that capture existing body or
value-parameter names fail explicitly. Resource-bearing struct parameters,
state assignment/arithmetic and dynamically selected state still require a
separate binding model. This is a bounded pixel-shader profile, not every HLSL
resource feature.

Native regression sources cover binding reflection, nested forwarding, legacy
state, literal resource arrays, prototypes, capture, recursion and expansion
limits. The browser gate has authored pixel controls distinguishing point-clamp
from point-wrap and checking that numeric argument side effects occur once.
Neither compiler nor GPU controls have been executed for this checkpoint.

## Grouped resource declarations and numeric helper overloads

Global texture/sampler groups are split before resource lowering. Each declarator
keeps its own array extent, register annotation and state initializer. This also
handles a default `SamplerState` followed by initialized states, and separate
legacy `sampler_state` blocks in one declaration. Initializer/property bodies are
not split at their internal punctuation. Existing resource/expansion limits and
unsupported property diagnostics remain in force.

The native compiler cases cover grouped state and resource arrays, numeric
helper overloads, different arities, prototypes with different parameter names,
nested forwarding and rejected opaque layouts. Browser pixel controls distinguish
clamp/wrap state, the selected numeric overload and single evaluation of value
arguments. JavaScript syntax and whitespace were checked; compiler, browser and
GPU execution remain pending on compatible private ephemeral JIT runners.

Comparison-state declarations also trigger specialization and grouped-declaration
normalization. The generated Gather/GatherCmp/SampleCmp/SampleCmpLevelZero calls participate
in binding reflection, so their filter/address aliases reach both renderer hosts.
Literal and side-effect-free dynamic Texture2D resource-array selection supports
these comparison methods, retaining scalar results for SampleCmp/SampleCmpLevelZero and
four-component results for GatherCmp. Added pixel controls check Gather component
order, comparison before bilinear filtering and comparison-state forwarding.


## Bounded Texture1D/Texture2D comparison sampling

`Texture2D.GatherCmp` and `GatherCmpRed`, plus `SampleCmp` and
`SampleCmpLevelZero` on Texture1D/Texture2D, are implemented for global,
statically initialized `SamplerComparisonState` profiles. Gather targets
Texture2D. Texture1D comparison sampling uses the center row of a 2D image and
needs a one-row asset for parity. The bounded profile parses the supported
`COMPARISON_MIN_MAG_*` point/linear filter combinations, with
NEVER, LESS, EQUAL, LESS_EQUAL, GREATER, NOT_EQUAL, GREATER_EQUAL and ALWAYS
comparison functions. Gather compares the red channel at four base-level texels
in the same component order used by the bounded ordinary Gather helper and honors
the static U/V/W address profile. Gather accepts a literal int2 offset. Texture1D
comparison samples accept an integer literal offset; Texture2D comparison samples
accept a literal int2.

SampleCmp and SampleCmpLevelZero compare the red channel of each fetched texel
before filtering. SampleCmp derives LOD from texture-coordinate derivatives,
selects the configured minification or magnification filter, and applies the
configured point or linear mip selection. SampleCmpLevelZero uses mip zero.
The compiler rejects comparison states without an explicit ComparisonFunc,
ordinary texture methods passed a comparison state, and comparison calls without
a static comparison state. Dynamic comparison state, Texture3D/cube comparison methods and broader resource
forms remain unsupported. This is a bounded source path, not a reference-pixel
claim; no build, compiler test, render, or device check was run for this update.
