# Saved sprite implementation

The `.milk`/`.milk2` importer now separates `[SPRITEn_BEGIN]`/`[SPRITEn_END]`
records from preset equations. It retains the original records for export,
selects the last definition of an index, and honors the envelope's `sprite=0`.
Unknown active fields, mismatched/nested blocks, non-finite values and external
asset paths fail explicitly. Disabled records are preserved without executing
code or admitting image dependencies. Limits are 16 active sprites, 64 saved
records, six-digit indices, 1024 numbered equation lines and the existing source,
expression-work and texture budgets.

Each sprite has its own persistent scope. `init_N` executes once, `code_N` in
numeric order each frame, with elapsed sprite time, frame, fps, audio bands and
saved blend progress. `done` retires after one final drawable/burnable frame.
The checked runtime reports errors and resets partially evaluated state. A new
preset resets sprite lifetimes. WebGL retains the evaluated frame while textures
load, preventing short-lived sprites from expiring before their first draw.

Saved image offsets and scale offsets remain separate from script `x/y/sx/sy`,
so a script assignment does not erase them. Signed scales and flip variables
mirror the quad. Image aspect, rotation, display aspect and repeated UVs are
shared Rust geometry. SpriteSpeed advances script rotation once by speed/fps
before each draw. Saved SpriteRot=1 is treated as the observed neutral UI default;
other saved values retain the initial-angle mapping. Non-default UI rotation
semantics still need a MilkDrop3 reference comparison.

Both GPU hosts use the same fragment source, sampling a distinct destination
snapshot for each sprite. They never sample the current draw attachment. Keyed
or fully transparent pixels preserve the destination. The saved blend IDs have
explicit arithmetic: alpha, decal, additive, source-color, color-key,
multiplicative, destination times inverse source, inverted source, cut-off,
darken and double-additive vivid. The six extended names do not all have their
conventional image-editor meaning. Color keys apply to each source texel before the shared manual bilinear sample,
with normal RGBA interpolation and repeated coordinates. This removes key-color
bleed caused by comparing only the already filtered sample. Alpha/filter raster
parity still requires reference comparison.

Burning layers 1–4 enter each preset's feedback before its composite shader.
Burning layer 0 stamps feedback after composite, becoming visible through the
next frame's history. Non-burning sprites compose once over the combined screen,
ordered by saved layer. Double presets do not execute sprite equations twice.
Canvas and the generic cross-renderer import gate reject this feature; the
separate WGSL renderer has no saved-sprite pipeline.

Format and behavior references: the [foo_vis_milk2 manual](https://jecassis.github.io/md2-docs/manual.html#sprites)
and its [MPL source](https://github.com/jecassis/foo_vis_milk2/tree/5b44cea79a8678a8f7e516e8c0cce0e4681f299b/vis_milk2),
plus the saved-field and arithmetic observations in
[MDropDX12](https://github.com/shanevbg/MDropDX12/tree/564407e127c4bef5c5dfbb23fb8f5d1d8afb6120/src/mDropDX12).
This implementation contains independently written parsing, runtime, geometry
and GL code, not copied renderer source from the noncommercial project. It does
not incorporate the Milkwave source (whose license text also includes a
noncommercial statement).

No build, tests, GPU capture or device run was performed in this source-only
continuation. Core regression source covers bounds, round trips, disabled blocks,
private state, final-frame lifetime, signed transforms, modes and error recovery.
The private browser gate has authored pixel checks for all 11 modes and the
one-frame loading case. These controls validate implementation arithmetic when
run; they are not MilkDrop3 reference captures. Exact masks, rotation controls,
layer/burn interactions, alpha/key filtering and physical-device parity remain
conformance gates. A successful import must not be reported as exact rendering.
