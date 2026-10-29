# Custom-object state and ordered drawing

Custom shapes and waves now have separate frame and point VM state. Each object
runs init once, retains private variables/memory, resets display parameters from
its serialized base, receives current read-only inputs and preset q registers,
and restores t1–t8 from init for each frame/shape instance. Point code starts each
frame with the frame's q/t inputs; private point variables persist between points
and frames. q mutations do not escape an object. Explicit gmegabuf writes return
to the enclosing preset. Runtime fields are not serialized into exported source.
Errors propagate through the checked frame budget. Canonical boolean values take
precedence over their saved aliases, so setting a flag to zero can disable it.

Shapes use up to 1024 instances with `instance`, at most 100 sides, top-down EEL
centers, the classic quarter-pi angular offset and aspect-correct geometry/UVs.
Their two colors/alphas form a radial gradient on both solid and textured fills.
Thick outlines use portable offset strokes. The scene has an explicit command
order, interleaving each fill and outline before custom waves, the classic wave,
sprites, center fade and screen borders. Native GL and WebGL execute that same
order. A scene's aggregate geometry remains bounded at 262144 vertices.

Motion vectors now interpolate the already evaluated feedback grid. Their dx/dy
controls move the vector lattice, length scales the actual feedback displacement,
and a pixel-sized minimum keeps still trails visible. Both hosts draw them into
previous feedback before the warp, rather than into the post-warp scene. Pixel
code executes once per grid vertex, including when motion vectors are enabled.

The WebGL review also corrected a buffer-binding error: consecutive colored
primitives now upload positions to the position buffer, not the color buffer left
bound by the preceding draw. Textured gradients share the existing color buffer;
shader sampler bindings are cleared before ordinary textured shapes.

Authored regression source covers instance positions/order, texture gradients,
private frame/point variables, init temporaries, q isolation, global memory and
feedback-derived motion trails. The browser compiler/driver gate includes a
colored shape beneath a textured radial-gradient shape to detect regrouped draw
order. No tests/builds/captures were executed. These are implementation contracts;
reference images, platform compilation and device raster fidelity are unmeasured.
The independent classic geometry follows observations from the MPL foo_vis_milk2
snapshot recorded in WAVEFORMS.md.
