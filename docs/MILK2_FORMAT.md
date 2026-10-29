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
the supported uniform Fade path. Duplicate names, oversized names, and any
other executable trailer content remains gated. Progress is a frozen blend position; it
does not advance as a transition timer. Each preset retains its own
equation/register state and feedback targets.

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
is retained in `crates/milkrust-core/THIRD_PARTY_NOTICES.md`. Hosts rasterize the
vertex alpha field to a cached R8 texture for native and browser shaders. The
Canvas fallback rejects spatial patterns instead of silently showing a uniform
cross-fade. Saved sprite blocks execute on native GL/GLES and WebGL; see
[EMBEDDED_SPRITES.md](EMBEDDED_SPRITES.md).

Bounded `NAME=` metadata and well-formed trailing sprite blocks disabled by
`sprite=0` or an omitted sprite flag are preserved without execution. A trailing
sprite executes only with `sprite=1`. Unknown flag values, unmatched/nested
blocks and unrelated trailer records remain gated. The wrapper enforces the same
2 MiB source budget when called independently of the preset importer.

The mask arithmetic includes the `-1` direction rule for the four
direction-aware patterns and the non-square `horizontal` mesh behavior. Saved
sprite handling includes per-owner scopes, flip UVs, saved offsets/repeats,
rotation modes, byte-wrapped alpha, layer and burn ordering, and the five
classic saved blend modes. These are source-level implementations. R8 rasterization,
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
