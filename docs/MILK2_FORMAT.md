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

`milk2.rs` is an independent bounded parser of these format facts. The wrapper
layout was observed in MDropDX12's `ParseMilk2File` and `SaveMilk2ToPath` at the
revision recorded below; no renderer algorithm or source code from that
CC-BY-NC implementation is copied into MilkRust. See also the upstream MilkDrop3
README's documented saved progress, direction and random values.

The WebGL and native GL/GLES renderers consume the saved pattern name, progress,
direction, and five saved random values. MilkRust contains deterministic spatial
masks for the documented patterns (`Fade`, `zoom`, `side`, `plasma`, `cercle`,
`plasma2/3`, `snail/2/3`, `triangle`, `donuts`, `corner`, `patches`,
`checkerboard`, `bubbles`, `stars`, `cisor`, `wave`, `curtain`, `vertical`,
`horizontal`, `linesvertical/horizontal`, `square`, `stars2`, `clock`, `nuclear`,
`arrow`, `cross`, and `cross2`). These are independent procedural formulas
based on the documented names and saved fields, not a port of MilkDrop3's
transition renderer. The Canvas fallback rejects spatial patterns instead of
silently showing a uniform cross-fade. Active embedded sprite/trailer extensions still fail closed. Bounded `NAME=`
metadata and well-formed sprite blocks explicitly disabled by saved `sprite=0`
are preserved without execution. Unknown flag values, unmatched/nested blocks
and unrelated trailer records remain gated. The wrapper enforces the same
2 MiB source budget even when called independently of the preset importer.

Parsing and rendering these masks do not imply visual equivalence. Exact
per-pattern semantics, saved-file comparisons against reference GPU output,
pattern-specific progress behavior, embedded extension behavior, and generic
renderer/device qualification remain open. Regression cases are source only and
have not run. Full `.milk2` compatibility remains open until those comparisons
pass.

Observed format source: [MDropDX12 564407e](https://github.com/shanevbg/MDropDX12/tree/564407e127c4bef5c5dfbb23fb8f5d1d8afb6120), `src/mDropDX12/engine_presets.cpp` and `engine.cpp`. This citation records format facts, not permission to incorporate its rendering implementation.

## Reference-source boundary

The inspected [MilkDrop3 source snapshot at `6b39088`](https://github.com/milkdrop2077/MilkDrop3/tree/6b39088f789441e18cc1665519ee7ffe439cece5/code/vis_milk2) documents `.milk2` blending in its README, but the checked-in renderer source does not contain a consumer for the saved wrapper/header records described above. It has a randomized preset-transition helper; that helper is not evidence for the frozen, serialized `.milk2` blend field. The available source snapshot therefore cannot serve as a complete implementation reference for the saved patterns. This is a source-availability finding, not a claim that MilkDrop3 itself lacks the feature. A port still needs authoritative per-pattern semantics and reference output for real saved files.

The disabled-sprite flag is a format observation from `engine_presets.cpp`'s
embedded-block parser in the cited source, independently implemented here.
It does not implement active sprite equations, layers or blend modes.
