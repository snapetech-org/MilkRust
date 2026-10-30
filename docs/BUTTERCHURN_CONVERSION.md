# Butterchurn declarative conversion

The converter accepts the numeric equation strings and generated GLSL in the
vendored Butterchurn 2.4.7 JSON format. It emits ordinary MilkDrop text for the
MilkRust native/WebGL engine. It never evaluates JavaScript, imports a module,
opens an include, or executes a shader during conversion.

Implementation: tools/convert_butterchurn.py, tools/butterchurn_equations.py,
tools/butterchurn_glsl.py and crates/milkrust-expr/src/butterchurn.rs.
The candidate workflow and the private-JIT Python/Rust regression entrypoints
include these paths.

## Equations

The numeric profile covers scoped a.field and a["field"] state, function-scoped
numeric locals, EEL memory indexing, arithmetic, assignment/compound assignment,
prefix/postfix updates, comparisons, 32-bit bitwise operations and shifts,
numeric Math operations, ternaries, operand-valued short-circuit logic, blocks,
if/else, for/while/do, break and continue.

Assignments resolve memory indices before the right-hand expression. Postfix
updates capture the old value once. Sequences use bounded balanced trees and
capture the final value independently of EEL exec2's first-argument return.
Locals are initialized once per invocation; a bare var declaration inside a
loop does not reset the preceding iteration's value. An uninitialized local
has the numeric NaN representation. This is a numeric profile, not JavaScript's
object, undefined, string/coercion or prototype model. Local names are encoded
to preserve case; state names that collide under EEL case folding fail.

The bcjs* intrinsics preserve exact numeric equality, nonzero/NaN truth,
floating remainder, division, ToUint32/ToInt32, masked shifts, signed zero,
Math rounding, domain errors and intermediate infinities/NaNs. Existing EEL
helpers retain their existing semantics. Math calls use separate numeric
intrinsics; a converted call to ifcond evaluates its arguments eagerly as a
JavaScript call does. Finite-value guards still apply when the engine consumes
render parameters.

The numeric references are ECMA-262's Number, ToUint32/ToInt32 and Math
operations: https://tc39.es/ecma262/2024/ . This implementation does not add
a JavaScript engine. Host APIs, dynamic property names, closures, downloaded
functions, randomness from Math.random and arbitrary objects fail conversion.

Each equation section is limited to 64 KiB, 16,384 tokens, 64 parser levels,
2,048 locals and 2,048 generated temporaries, with 256 KiB converted output.
The existing EEL work, recursion and loop limits still govern execution.

## Shaders

Generated GLSL is parsed and lowered to the existing typed legacy HLSL compiler.
Scalar/vector multiplication stays componentwise. GLSL matrix columns are
represented as HLSL rows, so typed multiplication reverses its operands inside
mul; column indexing, rectangular products, diagonal scalar constructors,
matrix resizing, and compound multiplication preserve that representation.

The conversion covers numeric types/constructors, declarations, structs,
functions, array indexing, control flow, static 2D/3D samplers, texture/LOD/bias/
gradient calls, mod, mix, vector comparisons, Boolean selection and derivatives.
GLSL vector/matrix equality reduces to a scalar result; equal/notEqual builtins
retain componentwise results. Blur scale/bias, PI and the Butterchurn rad/ang
initialization are supplied explicitly. Custom numeric uniforms without a host
binding fail. The typed compiler remains the authority for semantic validity,
resource availability and the bounded native/WebGL shader profile.

Both input and translated shader source are bounded to 128 KiB, source tokens
to 32,768 and parser nesting to 64. There is exactly one shader_body per stage.
Preprocessor directives/includes, arbitrary entry points and unsupported
syntax receive explicit accounting failures. Unknown function/resource
semantics are diagnosed by the typed compiler during qualification/load.

## Delivery and accounting

Every discovered JSON gets an accounting row. Successful conversion records
original SHA-256, converted SHA-256, retained shader stages and the required
butterchurn-numeric-v1 profile. Warp/composite sections and PSVERSION metadata
are emitted; no shader is silently removed. Converted is not compiled,
rendered or visually qualified.

Candidate manifests declare requiredEngine: milkrust and the conversion
profile. The same requirement is stored in a comment at the top of each .milk
file so ZIP export and loose-file re-import preserve engine selection.
PlezyNG rejects unknown requirements and does not offer projectM for these
presets. Classic fragment editing remains available for converted .milk files.

Candidate archives preserve original names, source URLs, source revision,
package MIT notice and per-preset attribution. Underlying preset rights remain
NOASSERTION/review-required until independently cleared. Conversion never
promotes a download, invents qualification results or changes a signing root.

Source regression cases are authored. No converter run, tests, builds or GPU
reference comparisons were executed for this completion checkpoint.
