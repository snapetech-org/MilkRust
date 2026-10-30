# Explicitly licensed upstream pack sources

`cayadev-2026` contains 55 complete presets by CaYaDev, from
[SoundVisualizer 2aab1db0458312484d4065013903133b1f49ed0a](https://github.com/CaYatur/SoundVisualizer/tree/2aab1db0458312484d4065013903133b1f49ed0a/presets/cayadev).
The author's folder-specific license dedicates these presets to CC BY 4.0.
The application's MIT license is not used as the preset license. The original
folder license is included in the pack. Every preset has an immutable upstream
URL, author credit, license link, SHA-256 and Git blob identity in `manifest.json`.
Only archive filenames are normalized; the 55 source files are byte-for-byte
unchanged from the pinned upstream blobs.

The private-JIT candidate workflow invokes `build_original_pack_candidates.py
--source-root content/licensed-packs`. It verifies both content digests, preserves
upstream provenance, builds a deterministic archive and creates a pending record
for every preset and required platform. It does not approve rights, render
presets, sign a catalog or enable a download. No archive was generated during
this source-only change. Historical collections with unresolved rights remain
separate and are not covered by this author's license.

Distribution requires retaining these credits, the CC BY 4.0 link and notice of
later modifications. The existing app pack-details/notices view consumes the
same per-asset records. The catalog's exact-archive qualification policy remains
the promotion gate; source import and a clear license do not establish rendering.
