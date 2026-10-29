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
  entry points with TEXCOORD0/COLOR0. Unknown input/output semantics fail.
- Separate warp/composite passes, three separable blur levels, deterministic
  original 2D/3D noise resources, mipmap generation, and per-binding sampler
  filtering/addressing on native GL/GLES and WebGL2. Noise and blur algorithms
  are independent implementations and still need reference comparisons.
- Scalar frame values, q1–q64 and packed q vectors, texture dimensions, aspect,
  deterministic preset/frame randoms and roaming vectors shared by both hosts.

## Limits and errors

Source is bounded to 128 KiB at import, 256 KiB after preprocessing; tokens,
macro expansions, conditional depth, compiler recursion, cumulative allocation,
operations and elapsed compilation time are bounded. Diagnostics are capped at
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
private ephemeral JIT runners. Native cross builds now require a target C++11
compiler and runtime in addition to Rust. Browser runners require Emscripten
3.1.64. Preserve every parser, driver, black-frame and image-difference failure
in the qualification report. Do not promote broader packs based only on source
compilation or catalog schema validation.
