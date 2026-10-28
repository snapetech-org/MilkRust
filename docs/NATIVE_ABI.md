# Native MilkRust renderer and C ABI v1

`milkrust-native` implements a native GL/GLES renderer over the existing bounded
Rust parser/evaluator and independently translated warp/composite stages. It
exports `include/milkrust.h` as a cdylib/staticlib. No browser, JavaScript runtime,
second audio decoder or geometry-count-only renderer is involved.

The host supplies a dedicated current OpenGL 3.2 or GLES 3.0 context and a GL
symbol loader. Creation, loading, texture upload, PCM consumption, rendering and
destruction all occur on its render thread. Handles are monotonic integers in a
thread-local registry; stale/wrong-thread/reentrant calls fail. C input buffer
lifetimes/alignment remain the foreign caller's responsibility. Rust unwinding
is caught at exported session calls. The workspace's unsafe ban remains intact
outside this boundary crate; GL and foreign-pointer operations are localized.

The GPU graph is feedback-read → warp/per-pixel mesh → custom textured shapes,
filled shapes/waves/lines → feedback-write → final composite into the supplied
output FBO. Two-preset documents own separate state and feedback pairs. Composite
output is never recycled into feedback. Missing textures, translation failures,
GL allocation/compile/link/draw failures stop the session with a bounded error.
No procedural replacement is drawn for an unsupported preset. Shader compilation
and GPU-driver hangs still require process/watchdog containment in the host;
catching Rust panics is not GPU fault containment.

PCM is interleaved 44.1 kHz stereo, at most 4096 frames per call, produced by the
existing same-player native tap and host resampler. The engine uses a fixed 512
sample Hann-window FFT with bounded storage and generation reset. Relative band
normalization is implemented, but exact MilkDrop reference audio filtering and
stereo-wave equivalence are not qualified. Render time must be monotonic and
paused by the host while hidden. No per-frame PCM or pixel payload crosses Dart.

Assets are uploaded as decoded top-down RGBA8; the host must resolve them only
under a validated pack root and perform header/decode limits. The renderer caps
textures at 4096 per side, 8 Mi pixels each, 64 named assets and 32 Mi pixels total.
Each frame is limited to 262144 vertices across at most two layers. Output uses
the same side/pixel bounds. Each layer owns two RGBA feedback surfaces; this is
additional GPU memory, not part of the named-image budget. PlezyNG's 720p tier
is substantially below this maximum.

The native stage gate currently matches the implemented experimental WebGL
subset, including saved uniform Fade only. Full HLSL, blur/noise resources,
reference fixed-function details, spatial `.milk2` masks, embedded extensions,
complete audio equivalence, compiler containment and fidelity remain open.
See GAPS.md. This is a native renderer source implementation, not a qualified
native MilkDrop-equivalent release.

No native crate build, regression run or GPU capture has been executed. Future
build/test/packaging work must run on compatible private ephemeral JIT runners.
The explicit workspace lock entry adds only the local crate; no third-party
runtime dependencies were introduced. Platform artifacts and install/ABI checks
remain required before enabling a release capability.

`milkrust_render_rgba` is the offscreen host path: the engine owns the output
framebuffer and returns tightly packed top-down RGBA8 into an exactly sized host
buffer. This lets Windows, Apple, and Android texture hosts share the same
bounded render worker while retaining their own GL context and texture lifetime.
The FBO rendering entry point remains available for zero-copy integrations.
Neither path has been compiled or exercised in this source-only session.
