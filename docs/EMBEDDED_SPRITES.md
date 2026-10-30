# Saved sprite implementation

The `.milk`/`.milk2` importer now separates `[SPRITEn_BEGIN]`/`[SPRITEn_END]`
records from preset equations. It retains the original records for export and
selects the last definition of an index. Within a block, ordinary saved fields
use their first occurrence while `SpriteName` uses its last occurrence.
Classic `.milk` saved sprites are enabled by default. The first standalone
`sprite` property outside a saved sprite block controls the gate, so `sprite=0`
disables sprites even when the property appears inside `[preset00]`. Records
inside saved sprite blocks do not change that setting. In `.milk2`, a trailing
saved sprite block executes only when the wrapper header explicitly contains
`sprite=1`; duplicate header flags use their last value, and preset-body keys do
not change that gate. A missing flag or `sprite=0` leaves the block
preserved and inert. Disabled blocks are preserved without executing code or
admitting image dependencies.
Unknown active fields, mismatched/nested blocks, non-finite values and external
asset paths fail explicitly. Disabled records are preserved without executing
code or admitting image dependencies. Limits are the 16 preset sprite slots
(indices 1–16) and 64 saved records, 255 numbered equation lines and the existing
source, expression-work and texture budgets. Numbered equations execute
contiguously from line 1 and stop at the first missing or empty record, through
line 255. MilkDrop3's external sprite image
cache can hold 100 assets; native and WebGL hosts admit that same 100-name
limit, subject to the aggregate 32 Mi decoded-pixel budget. That cache capacity
is separate from the 16 slots a preset can address.
`SpriteColorKey` follows MilkDrop's `%X` hexadecimal scan, with leading
whitespace and an optional `0x` prefix; parsing stops at trailing non-hex
text and oversized values wrap to 32 bits. A missing, empty, or unparseable
`SpriteName` produces no sprite. The loader matches the low 24 RGB bits and
uses `0xFFFFFFFF` as its no-key sentinel.

Each sprite has its own persistent scope. init_N executes once, code_N in
contiguous numeric order each frame, with elapsed sprite time, a sprite-local
frame counter, the owning preset's fps and audio bands, and saved blend
progress. EEL a defaults to 1; the per-frame value is clamped to 0..1 then
multiplied by SpriteAlpha without a second clamp. The packed vertex alpha
truncates and wraps to eight bits.

The upstream MilkDrop blendmode value is an EEL variable. It defaults to 0,
is evaluated after each frame's equations, truncated to int, then clamped to
0..4 before drawing. This matches MilkDrop 3's DrawUserSprites path at
https://github.com/milkdrop2077/MilkDrop3/blob/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2/milkdropfs.cpp
and texmgr.cpp. EEL blendmode assignments therefore affect the next sprite
draw; a missing value uses mode 0.

SpriteBlend, SpriteLayer, SpriteAlpha, SpriteBurn, SpriteRot, SpriteSpeed,
SpriteRepeatX and SpriteRepeatY are MilkRust-specific saved-field extensions,
not keys in the upstream MilkDrop 3 DrawUserSprites implementation. SpriteBlend
is a static override: values 0..10 use the optional engine table; modes 5..10
are nonstandard extensions. If SpriteBlend is absent, dynamic upstream
blendmode behavior is used. SpriteLayer selects the engine's ordered feedback
stage. SpriteAlpha multiplies the clamped EEL a value. SpriteBurn=0 disables
burning, SpriteBurn=1 always burns, and any other integer follows EEL burn
(which defaults to 1). Exactly SpriteRot=1 adds time * SpriteSpeed to EEL rot;
exactly SpriteRot=2 subtracts it. Speed is radians per second and uses absolute
host time. SpriteRepeatX/Y multiply the clamped EEL repeat registers. The done
variable retires the sprite after its final drawable frame. A final done &&
burn primary feedback draw uses MilkDrop's 4:3 aspect correction; its secondary
layer burn and display copy use normal screen geometry.
The checked runtime reports errors and resets partially evaluated state. A new
preset resets sprite lifetimes. WebGL retains the evaluated frame while textures
load, preventing short-lived sprites from expiring before their first draw.

Saved image offsets and scale offsets remain separate from script x/y/sx/sy, so a
script assignment does not erase them. MilkDrop clamps mapped EEL position and
scale values before adding saved offsets. Signed scales mirror the quad and
nonzero flipx/flipy values reverse the corresponding UV axis. Image aspect,
rotation, display aspect and repeated UVs use shared Rust geometry. EEL repeat
values clamp to 0.01–100 after equations, then multiply by saved repeat
multipliers. EEL a starts at 1, can be changed by equations, and is clamped to
0..1 before SpriteAlpha multiplication; the product is not clamped before
eight-bit D3D color packing.

SpriteLayer and SpriteBlend are engine-extension fields. Layers 1–4 enter
MilkRust's ordered feedback stages; upstream MilkDrop 3 exposes no SpriteLayer
field and draws user sprites through its final overlay/burn pass. Invalid
engine-extension layers preserve the block but prevent image admission and code
execution.
GIF sprite assets are decoded into a bounded full-canvas RGBA sequence before
upload. The shared decoder applies frame offsets and disposal operations,
composites transparent pixels, and maps zero frame delays to 100 ms. It caps
assets at 120 frames and 16 Mi decoded canvas pixels. Native GL and WebGL select
frames from each sprite's local elapsed time; shader and textured-primitive GIF
samplers use preset elapsed time. Netscape/AnimExts repeat metadata is honored:
without loop metadata the animation plays once and holds its last frame, zero
repeats forever, and a positive value repeats that many times after the first
play. Other animated raster formats remain outside the admitted texture profile.

Both GPU hosts use the same fragment source and per-resource sampler profile,
sampling a distinct destination snapshot for each sprite. They never sample the
current draw attachment. Standard upstream modes 0–4 follow the D3D9
DrawUserSprites table. The optional SpriteBlend extension adds modes 5–10.
Diffuse RGBA bytes are computed in shared Rust code before either GL host binds
them, avoiding driver-dependent shader rounding at byte boundaries:

| Mode | Stage color / alpha | RGB blend factors |
| --- | --- | --- |
| 0 Blend | diffuse × texture / diffuse alpha | source alpha, inverse source alpha |
| 1 Decal | diffuse × texture / 1 | blending disabled |
| 2 Additive | diffuse × texture / 1 | one, one |
| 3 Source color | texture / 1 | source color, inverse source color |
| 4 Color key | diffuse × texture / diffuse alpha × texture alpha | source alpha, inverse source alpha |
| 5 Multiplicative | diffuse × texture / diffuse alpha | destination color, zero |
| 6 Subtractive | diffuse × texture / diffuse alpha | zero, inverse source color |
| 7 Invert | 1 − texture / diffuse alpha × texture alpha | source alpha, inverse source alpha |
| 8 Cut-off | diffuse × texture / diffuse alpha × texture alpha | source alpha, inverse source alpha |
| 9 Darken | saturate(texture + diffuse − 0.5) / diffuse alpha | zero, source color |
| 10 Vivid | saturate(2 × diffuse × texture) / diffuse alpha | one, one |

Modes 1 and 2 premultiply diffuse RGB by alpha before byte conversion. The first five standard modes match the upstream selector; modes 5–10 are engine-only extensions. Mode 3
uses texture RGB as both the source color and blend factor, with white diffuse
color. Mode 7 uses white diffuse color and blends the inverted texture by
combined alpha. Mode 8 uses the adjusted diffuse color and combined alpha.
Mode 9 multiplies the destination by the darken stage color; mode 10 adds the
clamped 2× stage color. Modes 0, 5, 6, 9 and 10 do not use image alpha for
blending. Static-image color keys match opaque source texels by their low 24
RGB bits and replace a match with transparent black before generating the mip
chain. Static images use the shared sampler profile for their resource name, including
independent min/mag/mip filtering and per-axis Wrap/Clamp/Mirror addressing; an
unqualified name defaults to trilinear repeat. Animated GIF frames bypass color
keying and mip generation, and use the dedicated linear, no-mipmap repeat
profile. `0xFFFFFFFF` disables keying; an omitted key is zero. Out-of-range
invalid SpriteBlend extension IDs select mode 0; upstream EEL blendmode clamps to 0..4. GPU filtering and reference-pixel parity remain
unqualified.

Layers 1–4 use shared scene-command markers in native and WebGL: after
warp/blur and before shapes, after shapes, after custom waves, and after the
main wave and borders. Their primary draw enters the owner's feedback stage on
each active frame; a burning sprite draws a second time there on every frame.
On a final `done && burn` frame, the primary feedback draw uses the 4:3
correction and the second draw restores normal geometry. Layer 0 draws over
the final composite; a burning layer-0 sprite also stamps feedback once after
the composite has read it. Its final feedback stamp uses the 4:3 correction;
the display copy restores normal geometry. These rules apply to classic
`.milk` and `.milk2` blocks. A trailing `.milk2` sprite block belongs to
PRESET2. Double presets do not execute sprite equations twice.
Canvas and the generic cross-renderer import gate reject this feature; the
separate WGSL renderer has no saved-sprite pipeline.

Format and behavior references: the [foo_vis_milk2 manual](https://jecassis.github.io/md2-docs/manual.html#sprites)
and its [MPL source](https://github.com/jecassis/foo_vis_milk2/tree/5b44cea79a8678a8f7e516e8c0cce0e4681f299b/vis_milk2),
plus the compatible MIT saved-field specification listed in
`crates/milkrust-core/THIRD_PARTY_NOTICES.md`. MilkRust's parser, runtime,
geometry and GL code are independent implementations.

No build, tests, GPU capture or device run was performed in this source-only
continuation. Core regression source covers bounds, round trips, disabled blocks,
private state, final-frame lifetime, signed transforms, the five upstream modes plus six optional engine extensions and
error recovery. The private browser gate has authored pixel checks for all eleven
modes, 66 alpha/key combinations and the one-frame loading case. These
controls validate implementation arithmetic when run; they are not MilkDrop3
reference captures. Saved sprite fields, UV flips, alpha byte conversion and
source-level layer/burn ordering are implemented; their pixel behavior and
physical-device parity still require reference comparison. A successful import
must not be reported as exact rendering.

## Classic sprite correction sources

The sprite alpha-state, color-key and 8-bit vertex-color rules were checked against
MilkDrop3's published `code/vis_milk2/milkdropfs.cpp` (`DrawUserSprites`) at
`6b39088f789441e18cc1665519ee7ffe439cece5`. That snapshot does not include the
saved sprite-block loader; saved color-key semantics follow the compatible MIT
behavioral specification identified in `crates/milkrust-core/THIRD_PARTY_NOTICES.md`.
The keyed native/WebGL paths now produce transparent-black mipmapped variants;
GIF variants skip keying and mipmaps. The
browser source checks the five upstream modes plus six optional engine extensions and 66 combinations of sprite
blend mode, transparent/partial/opaque image alpha and matching/nonmatching color keys.
Core source checks unwrapped rotation lifetime and scripted numeric selectors.
These are authored controls, not executed results or MD3 captures.
