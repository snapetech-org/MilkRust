# MilkRust

[![CI](https://github.com/snapetech/MilkRust/actions/workflows/ci.yml/badge.svg)](https://github.com/snapetech/MilkRust/actions/workflows/ci.yml)

MilkRust is the standalone Rust/WASM visualizer engine extracted from slskR.

This repository is being prepared as the home for:

- `milkrust-core`: preset parsing, expression evaluation, runtime frame generation, compatibility reports, geometry, and WebGPU batch builders.
- `milkrust-pack`: portable preset pack manifests, loading, and validation.
- `milkrust-renderer-core`: renderer backend contracts and capability types.
- `milkrust-renderer-headless`: headless renderer stats backend for tests, reports, and batch tooling.
- `milkrust-cli`: command-line validation, inspection, compatibility, and render-stat tooling.
- `milkrust-wasm`: browser-facing WASM bindings and renderers.
- `packages/milkrust-web`: JavaScript client wrapper for applications that consume the WASM package.
- `packages/milkrust-react`: optional React integration built from the web SDK.
- `examples/web-component`: vanilla web component embedding sample for `<milkrust-visualizer>`.
- TypeScript typings for the web SDK are now published in `packages/milkrust-web/src/milkrustEngine.d.ts`.
- `apps/milkrust-player`: standalone browser player prototype.
- `apps/milkrust-studio`: browser authoring/debugging prototype.
- `crates/milkrust-desktop`: native desktop host primitives and headless playback probe.
- `crates/milkrust-desktop` also exposes `DesktopPlayerEngine` for host-side playback integration and control, including a
  pluggable `DesktopAudioProvider` contract.
- `tools`: smoke, compatibility, and performance checks.
- `examples`: small browser clients used for SDK verification.

The naming migration preserves legacy `.milk` and `.milk2` document readers.
It does not establish rendering compatibility: composite shader stages,
shader-aware per-pixel meshes and saved `.milk2` blending remain incomplete.
See [GAPS.md](GAPS.md) for the current engine and integration gates.

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the expanded plan covering the core engine, renderer backends, web SDK, CLI, preset packs, plugins, standalone player, Studio tooling, language SDKs, and host integrations. See [`docs/PLUGIN_API.md`](docs/PLUGIN_API.md), [`docs/PRESET_PACKS.md`](docs/PRESET_PACKS.md), [`docs/THIRD_PARTY_CONTENT_POLICY.md`](docs/THIRD_PARTY_CONTENT_POLICY.md), [`docs/THIRD_PARTY_CONTENT_AUDIT.generated.md`](docs/THIRD_PARTY_CONTENT_AUDIT.generated.md), [`docs/SOURCE_IMPORT_AUDIT.md`](docs/SOURCE_IMPORT_AUDIT.md), [`docs/RENDERER_AND_PLAYER_IMPORT_PLAN.md`](docs/RENDERER_AND_PLAYER_IMPORT_PLAN.md), and [`archive/slskdn-js-milkdrop-port`](archive/slskdn-js-milkdrop-port) for the current pack format, content policy, preserved slskdN JavaScript port, and import checklist.

## License

MilkRust code is licensed as `AGPL-3.0-only`; see [`LICENSE`](LICENSE). The license scope and content carve-outs are documented in [`LICENSE-SCOPE.md`](LICENSE-SCOPE.md).

Preset packs, textures, archived reference material, and generated content reports are not automatically relicensed by the MilkRust code license. Their status is tracked in [`content/catalog.json`](content/catalog.json). In particular, `content/community-unlicensed` is compatibility material with `NOASSERTION` license status and is excluded from default builds unless a distribution explicitly opts in.

## Build

```bash
cargo test --workspace
cargo check -p milkrust-wasm --target wasm32-unknown-unknown
npm run build:wasm
```

`npm run build:wasm` requires the `wasm-bindgen` CLI and writes browser package files to `pkg/`.

## Apps

```bash
npm run build:wasm
npm run dev:player
npm run dev:studio
npm run dev:web-component
```

The app commands serve the local repo at `http://127.0.0.1:4173/`.

`npm run dev:web-component` serves the `examples/web-component/` HTML demo at `http://127.0.0.1:4173/examples/web-component/`.

Community-unlicensed packs are not served by the local app server unless explicitly enabled:

```bash
MILKRUST_INCLUDE_COMMUNITY_CONTENT=1 npm run dev:player
```

## Desktop Host Probe

```bash
cargo run -p milkrust-desktop --bin milkrust-desktop -- --preset examples/sample-pack/presets/warm-scope.milk --frames 120 --fps 60
npm run test:desktop
```

The native probe exercises deterministic frame generation and headless render accounting for desktop integration work.

Also available:

```bash
cargo run -p milkrust-desktop --bin player -- --pack examples/sample-pack --frames 120 --fps 60
cargo run -p milkrust-desktop --bin studio -- --pack examples/sample-pack --json
```

The new `player` and `studio` entries split the desktop runtime surface into playback-focused and
compatibility/inspection-oriented paths while sharing the same preset loading and session runtime.

An optional native windowed prototype is also available (synthetic audio by default):

```bash
cargo run -p milkrust-desktop --features ui --bin player-ui -- --preset examples/sample-pack/presets/warm-scope.milk
```

```bash
cargo run -p milkrust-desktop --features ui --bin player-ui -- --pack examples/sample-pack --fps 60 --preset-duration 20 --no-loop
```

To enable live audio capture for `player-ui`, also enable the `audio` feature:

```bash
cargo run -p milkrust-desktop --features "ui audio" --bin player-ui -- --pack examples/sample-pack --audio-device "Built-in Audio Analog Stereo"
```

Player controls for this shell are:

- `Left` / `Right`: previous / next preset
- `Space`: pause / resume
- `R`: reset the current preset timer

## CLI

```bash
cargo run -p milkrust-cli -- validate preset.milk
cargo run -p milkrust-cli -- inspect preset.milk
cargo run -p milkrust-cli -- compat ./presets
cargo run -p milkrust-cli -- render-stats preset.milk
cargo run -p milkrust-cli -- pack-inspect examples/sample-pack
cargo run -p milkrust-cli -- pack-validate examples/sample-pack
```

`compat` reports whether a preset passes the currently analyzed parser/function/shader subset; its `renderStatus` remains `not-run` and `fidelityStatus` remains `not-assessed`. `render-stats` reports generated geometry counts only. Neither command is evidence of visual compatibility.

## Content Catalog

```bash
npm run content:audit
npm run content:report-community
npm run content:report-community:compat
npm run content:validate
```

Third-party packs live in [`content/catalog.json`](content/catalog.json). Vetted redistributable content goes in `content/third-party`; historical public community imports with unclear/no explicit licenses go in [`content/community-unlicensed`](content/community-unlicensed) for compatibility analysis and are excluded from default builds and downloadable packs. The repo vendors the MIT-licensed [`butterchurn-presets@2.4.7`](content/third-party/butterchurn-presets-2.4.7) converted preset package; projectM community source copies remain in the separately marked community-unlicensed lane.

## Local security scanning

Run `./scripts/security-scan-local.sh` for local Semgrep CE and the Trivy scanners that fit this repository. The installed pre-commit hook runs Semgrep on staged source/config files only; Trivy runs on demand or in CI. This public repository also runs Semgrep CE in GitHub Actions on changes and weekly. See [security-scanning.md](docs/security-scanning.md). Use `./scripts/update-security-tools.sh` to refresh local scanner data and the Trivy database.
