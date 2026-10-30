# MilkDrop 3 double-preset format

The saved wrapper contains two complete single presets between
`[PRESET1_BEGIN]`/`[PRESET1_END]` and `[PRESET2_BEGIN]`/`[PRESET2_END]`. Each
may contain its own `[preset00]`, version header, equations and shader stages.
It is not the older experimental `[preset00]`/`[preset01]` convention.

Header records include `blending_pattern`, `blending_progress`,
`blending_direction`, and `random_1` through `random_5`. MilkRust preserves
unknown header records and trailing embedded extensions through serialization,
while refusing to render unimplemented extensions. MilkDrop 3.2's trailing
`NAME=` display metadata is bounded, preserved, and does not by itself disable
the supported uniform Zoom path. Duplicate names, oversized names, and any
other executable trailer content remains gated. Progress is a frozen blend position; it
does not advance as a transition timer. Each preset retains its own
equation/register state and feedback targets. Stateful runtimes anchor EEL
`time` and sprite playback at source load; both halves of a saved pair start at
the same zero point and keep separate equation state.

MilkDrop retains these header variables across sequential loads. A fresh host
starts with an empty pattern, progress `1.0`, direction `0`, and random values
of `0.0`; a missing key on a later `.milk2` load inherits the runtime's current
value. Stateless parsing uses only those process-start defaults, while the
persistent frame-set runtime resolves absent keys from the preceding `.milk2`
header and updates the carried state when a new wrapper loads. An ordinary
single-preset runtime also captures valid `blending_pattern`,
`blending_progress`, `blending_direction`, and `random_1` through `random_5`
register values after its per-frame equations, retaining omitted and invalid
registers. The next source receives those shared globals before its own
equations run. Hosts that recreate the frame-set runtime can snapshot and
restore the validated `HeaderState`; restoration applies to subsequently
loaded sources. Serialization preserves which optional records were present,
even when rendering uses an inherited value.

The persistent native and WebAssembly preset-load/inspect paths validate a
wrapper against the runtime's carried header state before resetting its
per-preset equations and renderer. Stateless import/pack validation has no
history to consult and therefore rejects a wrapper whose omitted pattern has
no valid process-start value. The app loader treats a source with an incomplete
marker set as an ordinary preset, matching MilkDrop's NotDouble fallback;
direct `milk2::parse` reports the incomplete wrapper as an error.

Direction is a signed integer in the saved header; only `-1` reverses the four
direction-sensitive patterns, while other integer values preserve the normal
orientation. A non-integer direction is rejected. Pattern formulas always use the effective carried `random_1..5` registers.
Before any saved values exist, they use the process-start defaults of `0.0`;
a partial block inherits preceding values for omitted slots. Mask evaluation
does not synthesize midpoint or seeded random values.

Both preset scopes run their init and per-frame equations independently. For
rendering, PRESET2 receives MilkDrop's cosine interpolation of the documented
numeric controls from PRESET1: decay; wave alpha, RGB, position and mystery
parameter; outer/inner border size, RGBA; motion-vector grid, offsets, length
and RGBA; echo zoom/alpha; gamma; and all three blur min/max pairs plus the
first edge-darken value. `wave_mode` remains an integer and is not interpolated.
Echo orientation, waveform dots/thickness/additive/brightness, center darkening,
texture wrapping, invert, brighten, darken and solarize snap at the
composite-shader boundary (or the midpoint when both presets use the same
shader mode). Standard `mousex`/`mousey` coordinates and the date/time
registers (`year` through `totalseconds`) use the cosine mix. `mousedown` and
the shared rising-edge `mouseclick` snap at the same shader-dependent threshold.
The runtime clock derives elapsed time from each source load. Hour, minute,
second and millisecond fields derive from that preset-relative clock unless a
host supplies local clock state. `MilkRustClockState` provides validated local
year, month, day, weekday and wall-clock values. The Rust engine does not read
the system clock; stateless callers and hosts that omit the setter retain zero
calendar fields. The temporary mixed scope is
restored after scene generation, so its values do not become PRESET2's
next-frame equation state. Custom shape fills, outlines and custom waves use linear
`1 - P` / `P` alpha weights for PRESET1 / PRESET2. Outer and inner preset
borders are hidden throughout a saved double preset. In classic presets,
`bWaveDots` and `bUseDots` seed the same `wave_usedots` render control used by
per-frame equations.

`milk2.rs` is an independent bounded parser of the documented wrapper. The
saved-blend and sprite behavior is implemented from the compatible MIT
behavioral specification identified in
`crates/milkrust-core/THIRD_PARTY_NOTICES.md` and the public MilkDrop3 format
documentation. MilkDrop's free-form Created/version and URL preamble lines are
preserved, duplicate key/value records use the last value, and numeric fields
consume the compatible decimal prefix. No noncommercial renderer
implementation is incorporated.

The WebGL and native GL/GLES renderers consume the saved pattern name, progress,
direction and five saved random values. The 30 MD3 `(a,c)` blend-mask formulas
are implemented on the documented 64x48-cell mesh (65x49 vertices), with aspect
correction, saved progress and the random block. These formulas are a clean-room
port of the behavioral implementation/specification in M0Rf30/lactea; its MIT
attribution is retained in `crates/milkrust-core/THIRD_PARTY_NOTICES.md`. Hosts clamp each per-vertex mix and truncate it to MilkDrop's 8-bit D3D9 alpha
byte. Native GL and WebGL upload those 65x49 values as R8 and interpolate the
original triangles in their shaders. The
`all_saved_pattern_meshes_match_reference_vertices` unit control checks sampled
values for all 30 patterns against Lactea's published 4x2 reference table,
including the direction-sensitive variants. It exercises the production
generator on a small grid, not an official MilkDrop3 image capture. The Canvas
fallback rejects spatial patterns instead of silently showing a uniform
cross-fade. Saved sprite blocks execute on native GL/GLES and WebGL; see
[EMBEDDED_SPRITES.md](EMBEDDED_SPRITES.md).

Bounded `NAME=` metadata and well-formed trailing sprite blocks are preserved.
A trailing sprite block executes only when the wrapper header explicitly
contains `sprite=1`; duplicate `sprite` records use the last header value, and
records inside either preset body do not change that gate. An absent flag or
`sprite=0` preserves the block without execution. Unknown flag values,
unmatched/nested blocks and unrelated trailer
records remain gated. The wrapper enforces the same 2 MiB source budget when
called independently of the preset importer.

Within an active sprite block, repeated saved fields use their first value,
except `SpriteName`, whose last occurrence wins. A block with no non-empty
`SpriteName` does not create a sprite. Repeated sprite block indices still use
the last complete block.

The mask arithmetic uses effective carried `random_1..5` values for every
pattern: fresh runtime registers default to zero, and partial headers inherit
prior values. It also follows the `-1` direction rule for the four
direction-aware patterns and preserves the non-square `horizontal` mesh
behavior. Saved sprite handling includes per-owner scopes, flip UVs, saved
offsets/repeats, absolute-time `SpriteSpeed` for `SpriteRot` modes 1 and 2,
saved alpha multiplication, static layer/blend selection, integer burn modes,
and all eleven saved blend modes with default 4 and invalid values selecting
mode 0. A missing pattern fails closed rather than selecting an invented
transition.
These are source-level implementations. GPU triangle interpolation,
compositing, sprite filtering and draw order still need reference captures to
qualify pixel behavior on each renderer. The Canvas and WGSL paths explicitly
reject unsupported spatial masks or saved sprites. Unknown trailing extensions
remain gated.

Stateful `MilkRustRuntime` and `MilkRustFrameSetRuntime` APIs treat their
`time_seconds` argument as a monotonic host clock. On each source load they
reset EEL `time` to zero, start `frame` at the first frame, and anchor embedded
sprite time to that load. The two halves of one `.milk2` start together. Frame
rate still uses differences in the host clock. Stateless frame-builder APIs
continue to take preset-relative time directly.

## Reference-source boundary

The inspected [MilkDrop3 source snapshot at `6b39088`](https://github.com/milkdrop2077/MilkDrop3/tree/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2) documents saved wrapper fields and version-specific pattern changes, but its checked-in renderer source does not consume the saved wrapper. The per-pattern behavioral specification used for the clean-room implementation comes from Lactea's published analysis and compatible MIT implementation; it is not an official MilkDrop3 renderer release. Compare real saved files against the reference renderer before claiming pixel parity.

Saved sprite records are parsed and executed independently in `sprites.rs`.
Unknown fields, malformed blocks and unsupported trailing extensions fail
import. Source support is implemented for the documented saved fields in this
profile; compiler, renderer and device conformance remain unrun.


## Saved blend vertex alpha

The saved-pattern result is clamped per vertex, converted with truncation to an
8-bit D3D9 color alpha value, and then interpolated across the 64x48 mesh
triangles. Native GL and WebGL share the same 65x49 R8 mesh upload and
normalized shader sampling. The source regression checks the reference's byte
count and endpoint/truncation behavior; execution and pixel comparison remain
unverified.


## Raw saved header numbers

The `.milk2` parser follows the compatible `stof` behavior: it consumes the
numeric prefix as a finite 32-bit float, rejects NaN, infinity, and overflow,
and retains finite progress and random values even outside [0, 1]. Persistent
header state applies the same float-representability check. Rendering uses raw
progress for cosine variable interpolation and the PRESET1/PRESET2 scene
weights (`1 - P` / `P`). Only the per-vertex saved-mask result
`a * P + c` is clamped to [0, 1] before its D3D9 alpha byte is truncated.
The reference's ordered clamp saturates positive/negative infinity to the
corresponding endpoint and rejects a NaN mix. Regression sources cover parsing, carried state,
serialization, raw scene/interpolation weights, and out-of-range mask inputs;
they have not run.
