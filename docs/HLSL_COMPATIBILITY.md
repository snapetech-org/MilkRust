# HLSL compatibility scope

MilkRust does not yet have a full HLSL compiler. The native and WebGL paths
share a restricted expression/declaration translator, not a complete HLSL
front end. A GL compiler accepting generated text is not a fidelity result.

The current source corrects exact function-token recognition (including
whitespace), named sampler discovery, identifier case in simple conditional
returns, comment token boundaries and HLSL `fmod` truncation. The previous
GLSL `mod` substitution was wrong for negative inputs. Helpers cover scalar
and float2/3/4 operands, including scalar/vector combinations. No GPU comparison
has run. WGSL rejects that helper until its typed overload lowering exists.

Full HLSL remains separate work: a licensed compiler/front end, macros and
scopes, functions, arrays, structs, typed matrices and `mul`, control flow,
sampler/resource reflection, built-in noise/blur/mipmap resources, exact
MilkDrop uniforms and stage behavior. Compiler/driver hangs also require
containment outside the current in-process renderer. Extending token rewrites
does not satisfy those requirements.

Upstream routes reviewed for this work include
[projectM](https://github.com/projectM-visualizer/projectm) (whose own releases
still document shader translation limitations) and the MIT-licensed
[Butterchurn converter](https://github.com/jberg/milkdrop-shader-converter/tree/291c8851d2ed0769fce5d06cf32b39d51d027875).
The latter relies on hlsl2glslfork and glsl-optimizer and is not a drop-in
MilkRust renderer or complete resource/runtime implementation. No source from
the noncommercial MDropDX12 renderer was copied. All non-Fade `.milk2` spatial
patterns remain rejected rather than approximated under a compatibility claim.
