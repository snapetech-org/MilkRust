# Third-Party MilkRust Content Policy

MilkRust should support old MilkDrop ecosystem content broadly, but the repo should only redistribute content with clear redistribution permission.

## Content Types

MilkRust content imports should cover:

- `.milk` and `.milk2` presets.
- `.shape` and `.wave` fragments.
- Textures and sprites: `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.jfif`.
- Shader snippets and saved shader injections.
- MilkDrop3-style saved images embedded in presets.
- Playlist, favorites, history, rating, and search/filter metadata.
- Transition, automation, beat-change, and saved-panel settings.
- Preview thumbnails and generated compatibility reports.

## Copy Policy

- **include**: content can be committed into this repo and shipped in builds.
- **optional-download**: tooling may fetch it on user request, but it is not shipped by default.
- **link**: repo/builds may link to the upstream source, but must not copy the content.
- **review**: license/provenance is not yet clear enough to decide.
- **community-unlicensed**: public historical/community content imported into `content/community-unlicensed` for compatibility work, separated from vetted redistributable packs and excluded from default builds.
- **reject**: do not copy, fetch, package, or link as a MilkRust source.

## License Gate

Allowed for vendoring:

- MilkRust-owned content.
- Public domain or CC0.
- Permissive licenses such as MIT, BSD, Apache-2.0, ISC, and Zlib.
- Creative Commons licenses that permit redistribution, with attribution captured in the pack manifest.
- GPL/LGPL/AGPL-compatible content only when the content license is explicitly declared and distribution obligations are documented.

Not allowed for vendoring:

- No explicit license.
- Licenses that forbid redistribution, commercial use, modification, or sublicensing in ways that conflict with MilkRust releases.
- Packs that are only distributed through an installer or binary without source-content license terms.
- Images, sprites, screenshots, fonts, or logos with unclear ownership.

No-license historical MilkDrop presets are common. Those should be link-only by default, even if upstream projects have used them for many years. Upstream claims that no-license presets are "safe to assume" public domain should be recorded as provenance, but should not be treated as an explicit public-domain dedication for vendoring.

For a converted collection, a package-wide license declaration is retained as
source evidence, but does not replace a rights-holder review when the source
does not establish the license chain for each underlying preset or derivative.
Such material may be held as a review candidate, but it must remain excluded
from the public download catalog until an approved, archive-bound rights record
exists.

## Repo Model

- `content/catalog.json` is the source catalog.
- `content/third-party/<pack-id>/` is only for audited and vendorable content.
- `content/community-unlicensed/<pack-id>/` is for aggressive community imports with unclear/no explicit license.
- `content/generated/` is for generated inventories, compatibility matrices, and pack indexes.
- `docs/THIRD_PARTY_CONTENT_AUDIT.generated.md` is regenerated from catalog/local scans.

## Build Model

Default builds should include MilkRust-owned content and vetted redistributable packs only.

Community-unlicensed content should be opt-in for packaging and release builds. It can be present in the repository for compatibility testing and user-accessible libraries, but it must stay clearly marked and removable.

Optional content can be supported through:

- Runtime URL catalogs.
- User-selected local folders.
- Pack manifests that point to upstream download pages.
- Compatibility reports generated from user-provided local copies.

This keeps MilkRust compatible with old MilkDrop packs while separating vetted redistributable content from aggressive community imports that may need removal later.
