# MilkRust Preset Packs

MilkRust packs use a folder-based source layout for presets, textures, reusable fragments, thumbnails, data-only plugins, playlists, automation defaults, and license notices. A deterministic ZIP-based `.rmpack` distribution archive is defined for packs whose redistribution status is explicitly cleared.

## Layout

```text
manifest.json
presets/
textures/
fragments/
thumbnails/
plugins/
licenses/
integrity.json        # generated inside .rmpack archives
```

Only `manifest.json` is mandatory for an editable folder pack. Every published archive must additionally declare a known `distributionStatus`, license identifier, non-empty content notice, source URLs and shipped license/attribution notices. Its `assets` array must account for every content file other than `manifest.json` and the notice files. Each asset record carries its path, kind, license, author or rights holder, HTTPS source URL and matching notice file. Asset paths in the manifest must be relative and stay inside the pack directory.

## Archive packaging

`tools/package-rmpack.py` builds a stable archive from a source folder. The pack must explicitly set `distributionStatus` to `redistributable`; the builder refuses unreviewed or `NOASSERTION` community content. It validates per-asset attribution and notice coverage before writing the archive. File order, timestamps, compression and mode bits are normalized. The archive includes `integrity.json` with the relative path, byte length and SHA-256 digest of each source file. A separate catalog records the archive SHA-256 and download size.

The checked-in `content/downloads/catalog.json` contains the CC0 MilkRust sample pack and link-only entries for sources without redistribution permission. The sample pack proves the archive path; it does not establish broad MilkDrop rendering compatibility. The PlezyNG app must verify the catalog-pinned archive digest before extraction, enforce byte/file limits, validate paths and dependencies, and activate the install atomically. User imports must remain distinct from built-in packs.

MilkRust's compatibility output is static parser/function/shader/render-feature analysis. A preset that passes that subset has not been compiled on a GPU, rendered, or visually compared. Known stage gaps are returned as blockers when found; for example, a preset containing both warp and composite shader bodies requires separate render stages that are not yet implemented.

Multi-state `.milk2` files are blocked in the current subset until saved MilkDrop 3 blend patterns and transition state are implemented. The present generic two-frame crossfade is not reported as equivalent behavior.

Hash verification protects the archive against accidental or in-transit changes. A runtime-fetched catalog must also be authenticated by a pinned signature before authorizing new URLs or hashes. PlezyNG's initial catalog is embedded in app code with the expected archive digest rather than trusting a mutable catalog endpoint.

## Manifest

```json
{
  "schemaVersion": 1,
  "id": "milkrust-sample-pack",
  "name": "MilkRust Sample Pack",
  "version": "0.1.0",
  "author": "MilkRust",
  "description": "Small local fixture pack.",
  "license": "CC0-1.0",
  "distributionStatus": "redistributable",
  "licenseFiles": ["licenses/CC0-1.0.txt"],
  "contentNotice": "MilkRust-authored sample fixture.",
  "requiredMilkRustVersion": "0.1.0",
  "sourceUrls": ["https://github.com/snapetech/MilkRust"],
  "assets": [
    {
      "path": "presets/warm-scope.milk",
      "kind": "preset",
      "license": "CC0-1.0",
      "author": "MilkRust",
      "sourceUrl": "https://github.com/snapetech/MilkRust/blob/<commit>/examples/sample-pack/presets/warm-scope.milk",
      "licenseFile": "licenses/CC0-1.0.txt"
    }
  ],
  "presets": [
    {
      "id": "warm-scope",
      "title": "Warm Scope",
      "file": "presets/warm-scope.milk",
      "sourceFormat": "milk",
      "tags": ["scope"],
      "thumbnail": "thumbnails/warm-scope.png"
    }
  ],
  "textures": [
    { "id": "noise", "file": "textures/noise.png", "aliases": ["noise_lq"] }
  ],
  "fragments": [
    { "id": "center-polygon", "kind": "shape", "file": "fragments/center-polygon.json" }
  ],
  "plugins": [
    { "id": "default-playlist", "kind": "data", "entry": "plugins/default-playlist.json" }
  ],
  "playlist": ["warm-scope"],
  "automationDefaults": {
    "transitionSeconds": 8
  }
}
```

## Current Tooling

```bash
cargo run -p milkrust-cli -- pack-inspect examples/sample-pack
cargo run -p milkrust-cli -- pack-validate examples/sample-pack
npm run content:report-community:compat:full
```

`pack-inspect` reports manifest metadata. `pack-validate` reads referenced presets, runs compatibility analysis, checks path safety, and warns about missing optional assets such as thumbnails.

`sourceFormat` defaults from the file extension. Supported values currently include `milk`, `milk2`, and `butterchurn-json`. Native MilkRust rendering support is for `milk` and `milk2`; `butterchurn-json` is catalog/import material until a converter or compatibility adapter is added.

The web SDK exposes both eager and lazy pack loading:

```js
import {
  loadMilkRustPack,
  loadMilkRustPackManifest,
  loadMilkRustPackPresetSource,
} from '@milkrust/web';
```

- `loadMilkRustPack(url)` loads the manifest and every preset source. This is appropriate for small packs.
- `loadMilkRustPackManifest(url)` loads and validates only `manifest.json`. This is the default for large community libraries.
- `loadMilkRustPackPresetSource(preset)` fetches one preset source from a normalized manifest entry.

MilkRust Player uses the lazy path for bundled community packs so packs with thousands of presets can populate the browser list without fetching every `.milk` file at startup.

The SDK now also supports plugin entries:

- `loadMilkRustPackPlugins(manifest)` loads each plugin entry from `plugins/`.
- `engine.loadPlugins(plugins)` registers lifecycle hooks and data descriptors.
- pack plugin JSON descriptors can drive playlist order through a `kind: "playlist"` payload and `presetIds`.

See [`docs/PLUGIN_API.md`](PLUGIN_API.md) for the current hook surface.

## Build Path

- `milkrust-pack` is the shared Rust crate for pack manifests and validation.
- The CLI is the first consumer.
- The web SDK loads the same manifest shape for browser clients.
- The standalone player accepts local served pack folders and builds a selectable library from generated pack indexes.
- Studio should export this structure from authored presets, fragments, textures, and thumbnails.
- Release workflows may publish `.rmpack` archives only after provenance, per-asset rights and compatibility reports are reviewed.
