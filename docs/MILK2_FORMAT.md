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
equation/register state and feedback targets.

Each `.milk2` file supplies an independent saved-blend state. Missing pattern
is empty and fails closed; missing progress defaults to `0.5`, direction to
`1`, and each omitted random register to `0`. Loading a new file never inherits
the preceding file's header values. Serialization preserves whether each
optional header record was present. Direction is a signed integer in the saved
header; only `-1` reverses the four direction-sensitive patterns, while other
integer values preserve the normal orientation. A non-integer direction is
rejected. When the header has no random records at all, analytic patterns use
the reference renderer's midpoint random inputs (`0.5`); plasma uses its
deterministically seeded legacy random sequence. In a partial random block,
omitted registers remain zero.

`milk2.rs` is an independent bounded parser of the documented wrapper. The
saved-blend and sprite behavior is implemented from the compatible MIT
behavioral specification identified in
`crates/milkrust-core/THIRD_PARTY_NOTICES.md` and the public MilkDrop3 format
documentation. No noncommercial renderer implementation is incorporated.

The WebGL and native GL/GLES renderers consume the saved pattern name, progress,
direction and five saved random values. The 30 MD3 `(a,c)` blend-mask formulas
are implemented on the documented 64x48 vertex grid with aspect correction,
saved progress and the random block. These formulas are a clean-room port of the
behavioral implementation/specification in M0Rf30/lactea; its MIT attribution
is retained in `crates/milkrust-core/THIRD_PARTY_NOTICES.md`. Hosts upload the
32-bit fixed-point vertex alpha values as packed RGBA8UI and interpolate the
original triangles in the native and browser shaders. The
Canvas fallback rejects spatial patterns instead of silently showing a uniform
cross-fade. Saved sprite blocks execute on native GL/GLES and WebGL; see
[EMBEDDED_SPRITES.md](EMBEDDED_SPRITES.md).

Bounded `NAME=` metadata and well-formed trailing sprite blocks disabled by
`sprite=0` are preserved without execution. Trailing sprites execute by
default, including when the `sprite` flag is omitted. Unknown flag values, unmatched/nested
blocks and unrelated trailer records remain gated. The wrapper enforces the same
2 MiB source budget when called independently of the preset importer.

Within an active sprite block, repeated saved fields use their first value.
Repeated sprite block indices still use the last complete block.

The mask arithmetic includes the no-random fallback, `-1` direction rule for
the four direction-aware patterns and the non-square `horizontal` mesh
behavior. Saved
sprite handling includes per-owner scopes, flip UVs, saved offsets/repeats,
fps-scaled `SpriteSpeed`, EEL-seeded alpha/repeat/burn/layer/blend controls,
dynamic layer selection, and all eleven saved blend modes with default 0 and
dynamic selectors clamped to 0–10. `SpriteRot` is retained but inert in the
inspected classic loader. A missing pattern fails closed rather than selecting
an invented transition.
These are source-level implementations. GPU triangle interpolation,
compositing, sprite filtering and draw order still need reference captures to
qualify pixel behavior on each renderer. The Canvas and WGSL paths explicitly
reject unsupported spatial masks or saved sprites. Unknown trailing extensions
remain gated.

## Reference-source boundary

The inspected [MilkDrop3 source snapshot at `6b39088`](https://github.com/milkdrop2077/MilkDrop3/tree/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2) documents saved wrapper fields and version-specific pattern changes, but its checked-in renderer source does not consume the saved wrapper. The per-pattern behavioral specification used for the clean-room implementation comes from Lactea's published analysis and compatible MIT implementation; it is not an official MilkDrop3 renderer release. Compare real saved files against the reference renderer before claiming pixel parity.

Saved sprite records are parsed and executed independently in `sprites.rs`.
Unknown fields, malformed blocks and unsupported trailing extensions fail
import. Source support is implemented for the documented saved fields in this
profile; compiler, renderer and device conformance remain unrun.
