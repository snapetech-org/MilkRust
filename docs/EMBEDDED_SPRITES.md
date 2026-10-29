# Saved sprite implementation

The `.milk`/`.milk2` importer now separates `[SPRITEn_BEGIN]`/`[SPRITEn_END]`
records from preset equations. It retains the original records for export and
selects the last definition of an index. Within a block, the first occurrence
of each saved field wins, including `SpriteName`. `.milk` and `.milk2` sprite
blocks execute by default; only `sprite=0` disables them. Disabled blocks are
preserved without executing code or admitting image dependencies.
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
`SpriteColorKey` follows base-0 unsigned integer syntax: `0x` values are
hexadecimal, other leading-zero values are octal, and values without a prefix
are decimal. The value is retained; the inspected loader matches the low 24
RGB bits and uses `0xFFFFFFFF` as its no-key sentinel.

Each sprite has its own persistent scope. `init_N` executes once, `code_N` in
contiguous numeric order each frame, with elapsed sprite time, a sprite-local
frame counter, the owning preset's fps and audio bands, and saved blend
progress. `SpriteAlpha`, `SpriteBurn`, `SpriteBlend`, `SpriteLayer`, and
`SpriteRepeatX/Y` seed their matching EEL registers before `init_N`; equations
can replace those values. `SpriteSpeed` appends `rot=rot+speed/fps` after the
per-frame equations. `SpriteRot` is retained as saved metadata but has no
effect in the inspected classic loader. `done` retires the sprite after its
final drawable frame. When `burn` is nonzero, the classic renderer draws the
sprite into feedback on each active frame and then draws it over the display.
On a final `done && burn` frame, the primary feedback draw uses MilkDrop's 4:3
aspect correction; the display copy restores normal screen geometry.
The checked runtime reports errors and resets partially evaluated state. A new
preset resets sprite lifetimes. WebGL retains the evaluated frame while textures
load, preventing short-lived sprites from expiring before their first draw.

Saved image offsets and scale offsets remain separate from script `x/y/sx/sy`,
so a script assignment does not erase them. Position offsets join EEL
coordinates before the final clip-space clamp; size offsets join the EEL scales
before the final scale clamp. Signed scales mirror the quad and nonzero
`flipx`/`flipy` values reverse the corresponding UV axis. Image aspect,
rotation, display aspect and repeated UVs are shared Rust geometry. `repeatx`
and `repeaty` clamp to 0.01–100 after equations. EEL `burn` defaults to the
saved `SpriteBurn` value (1 when omitted), and equations can change it.
`layer` starts from `SpriteLayer` and rounds then clamps to 0–4 for `.milk2`
and 0–1 for classic `.milk` each frame.
`blendmode` starts from `SpriteBlend` (default 0), is truncated and clamped to
0–10 each frame. EEL `a` starts from `SpriteAlpha` (default 1), can be changed
by equations, and is clamped to 0–1 before 8-bit color packing.

GIF sprite assets are decoded into a bounded full-canvas RGBA sequence before
upload. The shared decoder applies frame offsets and disposal operations,
composites transparent pixels, and maps zero frame delays to 100 ms. It caps
assets at 120 frames and 16 Mi decoded canvas pixels. Native GL and WebGL select
frames from each sprite's local elapsed time; shader and textured-primitive GIF
samplers use preset elapsed time. Netscape/AnimExts repeat metadata is honored:
without loop metadata the animation plays once and holds its last frame, zero
repeats forever, and a positive value repeats that many times after the first
play. Other animated raster formats remain outside the admitted texture profile.

Both GPU hosts use the same fragment source, sampling a distinct destination
snapshot for each sprite. They never sample the current draw attachment. The
eleven modes follow MilkDrop 3's `DrawUserSprites` table. Diffuse RGBA bytes are
computed in shared Rust code before either GL host binds them, avoiding
driver-dependent shader rounding at byte boundaries:

| Mode | Stage color / alpha | RGB blend factors |
| --- | --- | --- |
| 0 Blend | diffuse × texture / diffuse alpha | source alpha, inverse source alpha |
| 1 Decal | diffuse × texture / 1 | blending disabled |
| 2 Additive | diffuse × texture / 1 | one, one |
| 3 Source color | texture / 1 | source color, inverse source color |
| 4 Color key | diffuse × texture / diffuse alpha × texture alpha | source alpha, inverse source alpha |
| 5 Multiplicative | diffuse × texture / diffuse alpha | destination color, zero |
| 6 Subtractive | diffuse × texture / diffuse alpha | zero, inverse source color |
| 7 Invert | 1 − (diffuse × texture) / discard when combined alpha < 0.004 | one, zero |
| 8 Cut-off | zero / discard when source alpha < 0.004 | zero, zero |
| 9 Darken | saturate(0.5 + diffuse × texture) / discard when combined alpha < 0.004 | zero, source color |
| 10 Vivid | diffuse × texture, drawn twice | one, one |

Modes 1, 2, 6 and 10 premultiply diffuse RGB by alpha before byte conversion. Mode 3
uses texture RGB as both the source color and the blend factor, with white
diffuse color. Mode 7 uses the diffuse tint and discards fully transparent
samples. Mode 8 clears the destination for nontransparent texels and leaves
keyed or transparent texels unchanged. Mode 9 uses the diffuse tint and
discards fully transparent samples. Modes 0, 5, 6 and 10 do
not use image alpha for blending. Color keys match opaque source
texels by their low 24 RGB bits before the shared manual bilinear sample. The
matching texel's alpha is cleared while its RGB is retained. `0xFFFFFFFF`
disables keying; an omitted key is zero. Out-of-range per-frame blend IDs clamp
to mode 0 or 10.
Alpha/filter raster parity
remains unqualified.

Milk2 layers 1–4 use shared scene-command markers in native and WebGL: after
warp/blur and before shapes, after shapes, after custom waves, and after the
main wave and borders. Their primary draw enters the owner's current feedback
stage each frame. On a final `done && burn` frame, MilkDrop draws an additional
feedback stamp with the uncorrected geometry. Layer 0 draws over the final
composite; a nonzero `burn` register also stamps feedback once on each active
frame, with the same additional final stamp. Classic `.milk` layers 0–1 use the
overlay/burn path; their final primary feedback draw gets the 4:3 correction,
and the display copy restores normal geometry.
Milk2 layers 1–4 are visible through the composite and are not drawn again as a
display overlay. This is the implemented source behavior; exact MilkDrop 3
layer/burn parity remains open until reference captures are compared. A trailing `.milk2` sprite block
belongs to PRESET2. Double presets do not execute sprite equations twice.
Canvas and the generic cross-renderer import gate reject this feature; the
separate WGSL renderer has no saved-sprite pipeline.

Format and behavior references: the [foo_vis_milk2 manual](https://jecassis.github.io/md2-docs/manual.html#sprites)
and its [MPL source](https://github.com/jecassis/foo_vis_milk2/tree/5b44cea79a8678a8f7e516e8c0cce0e4681f299b/vis_milk2),
plus the compatible MIT saved-field specification listed in
`crates/milkrust-core/THIRD_PARTY_NOTICES.md`. MilkRust's parser, runtime,
geometry and GL code are independent implementations.

No build, tests, GPU capture or device run was performed in this source-only
continuation. Core regression source covers bounds, round trips, disabled blocks,
private state, final-frame lifetime, signed transforms, all eleven modes and
error recovery. The private browser gate has authored pixel checks for all eleven
modes, 66 alpha/key combinations and the one-frame loading case. These
controls validate implementation arithmetic when run; they are not MilkDrop3
reference captures. Saved sprite fields, UV flips, alpha byte conversion and
source-level layer/burn ordering are implemented; their pixel behavior and
physical-device parity still require reference comparison. A successful import
must not be reported as exact rendering.

## Classic sprite correction sources

The classic alpha-state and 8-bit vertex-color rules were checked against
MilkDrop3's published `code/vis_milk2/milkdropfs.cpp` (`DrawUserSprites`) at
`6b39088f789441e18cc1665519ee7ffe439cece5`. That snapshot does not include the
saved sprite-block loader; sprite color-key handling follows the inspected
WIC loader path, which compares RGB and clears alpha before filtering. The
browser source checks all eleven modes and 66 combinations of sprite
blend mode, transparent/partial/opaque image alpha and matching/nonmatching color keys.
Core source checks unwrapped rotation lifetime and scripted numeric selectors.
These are authored controls, not executed results or MD3 captures.
