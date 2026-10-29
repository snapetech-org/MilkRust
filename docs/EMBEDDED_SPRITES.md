# Saved sprite implementation

The `.milk`/`.milk2` importer now separates `[SPRITEn_BEGIN]`/`[SPRITEn_END]`
records from preset equations. It retains the original records for export and
selects the last definition of an index. Within a block, the first occurrence
of each saved field wins except that the last `SpriteName` wins. Ordinary
`.milk` files execute blocks
by default; a trailing `.milk2` sprite block executes only when its envelope
contains `sprite=1`. `sprite=0` and an omitted flag preserve trailing blocks
without executing them or admitting their image dependencies.
Unknown active fields, mismatched/nested blocks, non-finite values and external
asset paths fail explicitly. Disabled records are preserved without executing
code or admitting image dependencies. Limits are the 16 preset sprite slots
(indices 1–16) and 64 saved records, 1024 numbered equation lines and the existing
source, expression-work and texture budgets. MilkDrop3's external sprite image
cache can hold 100 assets; native and WebGL hosts admit that same 100-name
limit, subject to the aggregate 32 Mi decoded-pixel budget. That cache capacity
is separate from the 16 slots a preset can address.
`SpriteColorKey` follows base-0 unsigned integer syntax: `0x` values are
hexadecimal, other leading-zero values are octal, and values without a prefix
are decimal before the alpha byte is forced opaque.

Each sprite has its own persistent scope. `init_N` executes once, `code_N` in
numeric order each frame, with elapsed sprite time, frame, fps, audio bands and
saved blend progress. `done` retires after one final drawable/burnable frame.
The checked runtime reports errors and resets partially evaluated state. A new
preset resets sprite lifetimes. WebGL retains the evaluated frame while textures
load, preventing short-lived sprites from expiring before their first draw.

Saved image offsets and scale offsets remain separate from script `x/y/sx/sy`,
so a script assignment does not erase them. Signed scales mirror the quad and
nonzero `flipx`/`flipy` values reverse the corresponding UV axis. Image aspect,
rotation, display aspect and repeated UVs are shared Rust geometry. Saved
`SpriteRot=1` adds absolute host time times
`SpriteSpeed`; `SpriteRot=2` subtracts it, and other values leave EEL `rot`
alone. Script rotation remains persistent and unwrapped. `SpriteRepeatX/Y`
multiply the EEL repeat registers, and saved position/size offsets remain
independent from EEL. `SpriteBurn=0` disables feedback burn, `1` always burns,
and other values follow EEL `burn`; an absent field defaults to `1`, so it
always burns unless a non-default static mode selects the EEL register.
Invalid layers skip both sprite code and drawing. The EEL `blendmode` register
starts from `SpriteBlend` (default 4), is read each frame and truncated to an
integer. Values outside 0–10 select mode 0.
`SpriteAlpha` multiplies script alpha before legacy 8-bit packing without
clamping the multiplied alpha, including byte wrap outside 0–1.

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
eleven modes follow MilkDrop 3's `DrawUserSprites` table:

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
| 8 Cut-off | folded diffuse × texture / diffuse alpha × texture alpha | source alpha, inverse source alpha |
| 9 Darken | saturate(texture + diffuse − 0.5) / diffuse alpha | zero, source color |
| 10 Vivid | saturate(2 × texture × diffuse) / diffuse alpha | one, one |

Diffuse vertex color is quantized on the CPU to the exact byte that the legacy
D3D vertex path stores; both GL hosts pass the same normalized byte values to
the shared shader, avoiding backend-specific float-to-byte rounding. Modes 1
and 2 premultiply diffuse RGB by alpha before byte conversion. Mode 3 uses
texture RGB as both the source color and the blend factor, with white diffuse
color. Mode 7 also uses white diffuse RGB; mode 8 folds each diffuse channel
around 0.5 before multiplying by texture. Modes 0, 5, 6, 9 and 10
ignore image alpha. Color keys match opaque source
texels (ARGB, not RGB alone) before the shared
manual bilinear sample and replace the whole matching texel with transparent
black. Other texels retain their RGBA values. The saved RGB key is promoted to
opaque ARGB, matching MilkDrop's `0xff000000 | ck` texture-loader argument. This
follows D3DX texture-loading semantics; it corrects the earlier alpha-only key
replacement. Out-of-range per-frame blend IDs select mode 0.
Alpha/filter raster parity
remains unqualified.

Burning layer 1 is inserted after warp/blur and before shapes, layer 2
after shapes, layer 3 after custom waves, and layer 4 after the main wave and
borders. Shared scene-command markers keep the native and WebGL insertion
points aligned. These sprites enter their owner's feedback before its composite
shader. Burning layer 0 stamps feedback after that owner's composite, becoming
visible through the next frame's history. Layers 1–4 are shown through
their owner's composite and receive no separate screen overlay. A burning sprite
on those layers is drawn twice into feedback at its stage; a non-burning one is
drawn once. Only layer 0 receives a separate post-composite screen draw, and a
burning layer-0 sprite also stamps feedback once. A trailing `.milk2` sprite
block belongs to PRESET2. Double presets do not execute sprite equations twice.
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
`6b39088f789441e18cc1665519ee7ffe439cece5`. Its `texmgr.cpp` supplies an opaque
ARGB key to `D3DXCreateTextureFromFileEx`, whose
[documented behavior](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dxcreatetexturefromfileex)
replaces matching pixels with transparent black and includes alpha in the
match. The browser source checks all eleven modes and 66 combinations of sprite
blend mode, transparent/partial/opaque image alpha and matching/nonmatching color keys.
Core source checks unwrapped rotation lifetime and scripted numeric selectors.
These are authored controls, not executed results or MD3 captures.
