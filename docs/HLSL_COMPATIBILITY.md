# HLSL compatibility scope

MilkRust still does **not** have a full HLSL compiler. The native and WebGL
paths share a restricted HLSL-like expression/declaration translator. A driver
accepting generated GLSL is not a fidelity result. Full HLSL, full `.milk2`,
and broad community-preset compatibility remain unimplemented and
unqualified.

## Implemented translator subset

The bounded path currently handles a single returned expression, a limited
sequence of initialized scalar/vector declarations and assignments, simple
conditional returns, selected common HLSL intrinsics, and these legacy 2D
sampling forms when the sampler maps to a known MilkDrop texture binding:
`tex2D`/`tex`, `tex2Dlod`, `tex2Dbias`, `tex2Dproj`, and `tex2Dgrad`. The code
preserves HLSL `fmod` truncation for native/WebGL. WebGPU rejects sampling
forms whose sampling semantics are not represented by its current bindings.

Common `float1`, `half`/`halfN`, `min16floatN`/`min10floatN`, `intN`, `uintN`
and `boolN` declarations and constructors now lower to explicit GLSL
scalar/vector types. The half and minimum-precision float forms widen to
32-bit GLSL floats; rounding can differ from a 16-bit HLSL implementation.
WebGPU rejects integer and boolean types until their WGSL constructor and
binding path is implemented. This alias lowering does not supply HLSL's
broader implicit cast, overflow or precision rules.

`analyze_milkrust_shader_compatibility` returns stable issue categories for
common unsupported classes: source limit, preprocessor, structured types,
matrix operations, control flow, arrays/indexing, resource declarations,
unsupported texture intrinsics, or the remaining restricted grammar. These
diagnostics are surfaced in CLI and desktop compatibility reports. They are a
triage aid, not a compiler or qualification claim. A supported result means
only that the current translator generated its bounded output.

The translator is not a general HLSL front end. Macros, arbitrary functions and
scope, arrays, structs, typed matrices and `mul`, loops and general branching,
resource reflection, shader-model semantics, blur/noise/mipmap resources,
exact MilkDrop uniforms and stage behavior remain unsupported. Compiler and
driver hangs also need resource/time containment. Non-Fade `.milk2` spatial
patterns remain rejected rather than approximated.

## Integration preparation and acceptance criteria

The MIT-licensed [Thekla HLSLParser](https://github.com/Thekla/hlslparser) is a
candidate parser, not a complete compatibility solution. Its legacy D3D9 scope
and community-maintained GLSL emitter must be reviewed at a pinned revision.
Before adding it to a shipping runtime, the implementation must:

1. Pin source and transitive dependencies, record every license, and establish
   native plus WebAssembly buildability without relaxing `unsafe_code = forbid`
   across the Rust workspace. If a foreign-language parser is required, isolate
   its FFI behind a separately reviewed safe boundary.
2. Parse HLSL into a typed, bounded intermediate form and map MilkDrop's
   uniforms, texture aliases, sampler state and warp/composite stages explicitly.
   Keep GLSL ES and WGSL resource layouts distinct; reject any resource or
   intrinsic a selected backend cannot represent.
3. Enforce byte, token, nesting, recursion, instruction, allocation and elapsed
   time limits before handing output to a driver. Compilation must be
   cancellable or isolated so hostile presets cannot hang playback.
4. Use a rights-cleared shader corpus and per-case records for source parser
   result, generated source, backend compilation, image comparison, audio
   synchronization and resource behavior. Compare supported targets with an
   approved reference implementation; retain black-frame and translation
   failure cases as failures.
5. Version the compatibility profile and only broaden it when every required
   backend passes the corpus. Keep unsupported constructs visible in the app's
   compatibility report and never silently downgrade a shader-bearing preset.

projectM's open-source HLSL conversion work and
[MilkDrop shader converter](https://github.com/jberg/milkdrop-shader-converter/tree/291c8851d2ed0769fce5d06cf32b39d51d027875)
are useful comparison sources, not drop-in renderer replacements. Their
translation limitations and different runtime/resource models mean parser
integration alone cannot support a claim that all `.milk` or `.milk2` presets
work. No source from the noncommercial MDropDX12 renderer was copied.

No HLSL compiler integration, GPU comparison, full `.milk2` implementation or
qualification was performed in this source-only change. The added type
lowering and issue diagnostics improve a limited subset and make failure
categories visible; the compiler and rendering work remain open.
