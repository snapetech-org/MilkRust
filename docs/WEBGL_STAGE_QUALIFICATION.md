# WebGL stage implementation and pending qualification

The browser source uses distinct warp and composite programs. Warp samples the
previous feedback target, geometry draws into the new feedback target, and
composite samples that target into the canvas. Composite output never replaces
feedback history. Per-pixel UVs reach translated warp shaders through a separate
mesh vertex program. Stage caches, samplers and feedback targets belong to each
preset renderer. This is source implementation, not a measured fidelity result.

`MilkRustFrame.shader_stages` is the stage-aware contract. Older `shader_source`
and `shader_texture_samplers` fields remain for existing consumers; they do not
represent a complete pipeline. The general compatibility report remains
conservative across renderer targets. The browser calls the explicit
`validate_milkrust_webgl_import` gate, which allows this stage implementation but
retains unsupported equation/shader and saved `.milk2` blend failures.

Translation, shader compilation/linking, missing image, image decode/upload,
texture-unit, framebuffer, draw and context-loss failures stop the current
preview. `renderStatusJson` reports the reason; reloading resets the failure and
resources. Images still loading delay drawing. The Canvas fallback cannot claim
shader or warp-mesh support. Browser image decoding happens before the upload
dimension check, so this is not a complete decoder memory sandbox. Per-expression work/depth limits are now implemented in source; aggregate frame
work, propagation of all equation errors, compiler time limits and hung-driver
recovery remain open.

Bounds in this source: 2 MiB preset text, 128 KiB per translated shader,
32 MiB embedded-image map, 64 images, 24 MiB per encoded image, 4096 pixels per
side/8 MP per image or render target, and 32 MP of named GPU textures per preset.
These are limits to qualify, not demonstrated performance budgets. Named image
callbacks are retained and detached on disposal; programs, buffers, targets and
textures are deleted on reset/disposal instead of accumulating across presets.

## Required private JIT / GPU checks (not run)

- Build the Rust workspace and WASM target, regenerate WASM bindings, run the
  existing SDK tests and the new stage/gate regression source. SDK regression
  source now requires rejected executable entries to make the pack/plugin result
  invalid without fetching or importing code, and checks render error/loading status.
- Render `crates/milkrust-core/tests/fixtures/warp-composite-order.milk` with fixed
  audio/time/settings. With geometry disabled, feedback should accumulate red
  (0.25, then 0.375, approaching 0.5) and the final image should carry it in green.
  Check a sequence, not a single frame; feeding composite output into history
  produces other channels. Compare with the pinned reference and account for
  quantization. These expected values are derived from the fixture equations,
  not a captured result.
- Exercise warp-only, composite-only, both stages, per-pixel mesh plus warp,
  independent named textures per stage/preset, more than four sampler names,
  missing/invalid images, compiler errors, context loss, and repeated resize/load/
  dispose. An always-black or fixed-image renderer must fail the fixture.
- Verify alpha and blend behavior separately from saved `.milk2` pattern/state
  semantics, which are still unavailable. Qualify blur/noise, coordinates, EEL,
  feedback/color semantics and the full corpus before promoting any fidelity cell.

No tests, builds, CI, render captures or benchmarks ran in this session. Follow
`AGENTS.md`: compatible private ephemeral JIT runners only, with the actual GPU
and device matrix. The native renderer/ABI and other backend implementations
remain separate work.
