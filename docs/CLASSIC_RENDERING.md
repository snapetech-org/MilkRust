# Classic preset rendering source

Native GL/GLES and WebGL now share the fixed-function display controls in
`classic.rs`: video echo zoom/alpha/orientation, additive gamma gain, brighten,
darken, solarize and invert, in legacy order. They run only when a preset has no
composite shader. Display output never replaces feedback history. The serialized
`fGammaAdj` key now initializes EEL `gamma`. Feedback honors clamp/wrap in both
the regular and mesh paths. Darken-center emits a feathered black primitive;
waves and shapes retain additive flags, and the ordinary waveform honors dots.

`warp.rs` supplies feedback coordinates for every parsed preset, including those
without per-pixel equations. It includes radial zoom exponent, stretch and center,
animated warp, rotation, translation and aspect correction. Native and browser
entrypoints provide actual render dimensions before equation evaluation. EEL sees
top-down `y` and corner-normalized radius; the resulting UVs are converted to the
common bottom-up GL convention. OpenGL does not need D3D9's half-texel offset.
The default grid is 48 by 36, bounded to 128 per side. Each grid vertex is
evaluated once before triangle expansion, avoiding duplicate expression work on
shared vertices. Invalid coordinates and equation errors propagate through the
checked frame API. Frame `fps` follows the supplied render clock.

The transform order and legacy display arithmetic were checked against
[`foo_vis_milk2` at 5b44cea](https://github.com/jecassis/foo_vis_milk2/blob/5b44cea79a8678a8f7e516e8c0cce0e4681f299b/vis_milk2/milkdropfs.cpp)
(`WarpedBlit`, `DarkenCenter`, `ShowToUser_NoShaders`). Rust geometry and the
single-pass GLSL arithmetic are independently expressed. Finite limits replace
singular divisions and unbounded brightness/UVs; therefore pathological inputs
are not promised to reproduce legacy undefined behavior.

Source regression cases cover uniform propagation, aspect/identity, transform
coordinates, equation rejection, additive/dot flags and center fade. The private
browser gate also probes fixed-function gamma/invert after an HLSL warp. No
build, test, GPU capture or device qualification has been run in this source-only
continuation. General waveform-mode, stereo, line-thickness, stage ordering and
reference-image conformance still require review; these changes close specific
missing controls rather than certifying every community preset.
