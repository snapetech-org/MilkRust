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
constant bounds of 1–4096 elements; integer overflow and division by zero in
constant evaluation return errors across the ABI. Diagnostics are capped at
8 KiB and compiler output at 1 MiB (renderer input remains 256 KiB). A bounded
per-thread cache avoids repeat native compilation. GPU loop execution shares a
16,384-iteration counter per fragment and discards fragments exceeding it.
These limits do not sandbox a graphics driver or certify arbitrary GPU code.

Unknown semantics, unsupported resource kinds, macro stringification, include
files, unsupported shader models and remaining emitter gaps fail rather than
becoming a fallback scene. This is a legacy pixel-shader profile, not all modern
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
before creating an HTML image. PNG/JPEG/BMP/static GIF/WebP use the browser codec;
legacy DDS (DXT1/3/5 and masked RGB24/32) and TGA (truecolor/grayscale, raw/RLE)
use bounded Rust decoding and direct RGBA upload. Both paths use 4096-per-side,
8-megapixel-per-image and 32-megapixel-per-preset limits. Pixel capacity is reserved
before asynchronous loads and returned on disposal, including upload failures.
DDS/TGA rows are oriented consistently with native uploads, and named samplers
receive actual dimensions. EXIF dimension swaps preserve the same pixel budget.
Animated raster containers are explicitly rejected by this static texture path;
the browser codec and graphics driver still are not process-isolated. CPU and
browser upload regression sources have been added but not executed.

The WASM engine owns removable context-loss/restoration listeners. It requests
browser recovery, pauses equation/frame work while lost, and recreates all GPU
handles and textures on the first frame after restoration. Preset text, assets
and CPU equation state survive; feedback history restarts because the old GPU
contents no longer exist. SDK callers receive a recoverable waiting state and
a clearing update. The private browser integration source now exercises actual
`WEBGL_lose_context` restoration and nonblank rendering; it remains unrun.
