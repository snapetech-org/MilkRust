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
other trailer content remain gated. Progress is a frozen blend position; it
does not advance as a transition timer. Each preset retains its own
equation/register state and feedback targets.

`milk2.rs` is an independent bounded parser of these format facts. The wrapper
layout was observed in MDropDX12's `ParseMilk2File` and `SaveMilk2ToPath` at the
revision recorded below; no renderer algorithm or source code from that
CC-BY-NC implementation is copied into MilkRust. See also the upstream MilkDrop3
README's documented saved progress, direction and random values.

The experimental WebGL and native GL routes support a uniform **Fade** envelope
with only known saved metadata. Other named spatial patterns and embedded sprites
retain explicit renderer requirements. Parsing and round-trip preservation do
not imply visual equivalence. Generic renderer qualification remains conservative.
Regression cases are source only and have not run. Full `.milk2` compatibility
remains open until all saved spatial patterns, extension behavior and reference
GPU sequences are implemented and qualified.

Observed format source: [MDropDX12 564407e](https://github.com/shanevbg/MDropDX12/tree/564407e127c4bef5c5dfbb23fb8f5d1d8afb6120), `src/mDropDX12/engine_presets.cpp` and `engine.cpp`. This citation records format facts, not permission to incorporate its rendering implementation.
