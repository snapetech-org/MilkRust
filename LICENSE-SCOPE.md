# MilkRust License Scope

MilkRust code is licensed under the GNU Affero General Public License v3.0 only (`AGPL-3.0-only`). The full license text is in [`LICENSE`](LICENSE).

This applies to MilkRust-owned source code, tooling, app prototypes, SDK wrappers, tests, examples, documentation, and package metadata unless a file or directory says otherwise.

## Published Packages

- Rust crates in `crates/`: `AGPL-3.0-only`.
- `crates/milkrust-desktop`: native desktop host primitives and headless probe are also `AGPL-3.0-only`.
- JavaScript package `@milkrust/web`: `AGPL-3.0-only`.
- Browser app prototypes in `apps/`: `AGPL-3.0-only`.
- MilkRust build and audit tools in `tools/`: `AGPL-3.0-only`.

The AGPL choice is intentional while MilkRust contains code imported from previous MilkRust/slsk projects and is distributed as a network-capable browser/WASM SDK. It keeps source obligations clear for hosted or modified deployments.

## Content Is Not Relicensed

The repository also contains presets, textures, generated reports, archived reference material, and third-party package copies. The MilkRust project license does not relicense those materials.

Content license status is tracked in [`content/catalog.json`](content/catalog.json) and summarized in [`docs/THIRD_PARTY_CONTENT_AUDIT.generated.md`](docs/THIRD_PARTY_CONTENT_AUDIT.generated.md).

Current content classes:

- `examples/sample-pack`: MilkRust-owned fixture content, catalogued as `CC0-1.0`.
- `content/third-party`: third-party content with explicit redistribution terms, such as MIT-licensed Butterchurn presets.
- `content/community-unlicensed`: public historical/community content imported for compatibility work with `NOASSERTION` license status. This content is not covered by the MilkRust AGPL license, is excluded from default builds by catalog policy, and must remain removable.
- `content/generated`: generated indexes and reports. Generated reports may describe third-party content but do not change its license.
- `archive`: preserved source/reference material. Each archived source keeps its original provenance and should not be treated as newly licensed content unless a local notice says so.

## Distribution Rules

Default source and package releases may include MilkRust code, MilkRust-owned examples, generated reports, and vetted third-party content whose catalog `copyPolicy` is `include`.

Release builds must not silently bundle `content/community-unlicensed`. Distributions that opt into that content should preserve the per-pack `MILKRUST-COMMUNITY-NOTICE.md` files, catalog metadata, source links, and takedown/removal language.

The local app server follows the same default posture: community-unlicensed content is not served unless `MILKRUST_INCLUDE_COMMUNITY_CONTENT=1` is set or the server is constructed with `includeCommunityContent: true`.

Downstream pack authors should set explicit licenses in their pack `manifest.json`. If a pack has mixed licensing, each asset should carry its own notice under the pack's `licenses/` directory.

## Typed HLSL compiler

`crates/milkrust-hlsl/vendor/hlslparser/` contains Thekla HLSLParser under its
MIT license, pinned and locally modified as recorded in `UPSTREAM.json`. Its
license remains in that directory and is copied into browser/app notice bundles.
The browser compiler also distributes Emscripten and libc++ runtime notices.
MilkRust integration code remains AGPL-3.0-only.

## MilkDrop 3 saved blend and sprite behavior

`crates/milkrust-core/src/milk2_mask.rs` contains an MIT-licensed clean-room
port of behavioral formulas from M0Rf30/lactea. Its published saved-sprite
behavior specification also informed MilkRust's independent parsing and runtime
implementation. The copyright and full license terms are preserved in
[`crates/milkrust-core/THIRD_PARTY_NOTICES.md`](crates/milkrust-core/THIRD_PARTY_NOTICES.md).
Other MilkRust-owned `.milk2` parsing, sprite runtime and GPU integration
remains `AGPL-3.0-only`.
