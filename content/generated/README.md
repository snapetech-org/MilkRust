# Generated Content Reports

This directory contains generated MilkRust content reports.

- `community-pack-summary.json`: machine-readable summary of community-unlicensed packs.
- `COMMUNITY_PACK_SUMMARY.md`: human-readable summary of the same data.
- `desktop-plugin-reports/desktop-plugin-discovery-summary.json`: machine-readable pack plugin discovery summary for desktop runners.
- `desktop-plugin-reports/*-*.json`: per-mode discovery snapshots for pack scans (`player-*`, `probe-*`, `studio-*`).
- `desktop-plugin-reports/desktop-plugin-discovery-summary.md`: optional human-readable markdown summary (`--write-markdown`).

Regenerate with:

```bash
npm run content:report-community
npm run content:report-community:compat
npm run content:report-community:compat:full
npm run desktop:plugin-report
npm run desktop:plugin-report:catalog
```

The checked-in community summary may be a historical snapshot until the full
compatibility report is regenerated. The current report generator retains a
compact result for each manifest preset, filesystem-discovered preset, and
manifest path that is missing from the scan; it also preserves directory,
read, input-limit, encoding and source-inventory diagnostics. These static
results do not establish renderer or visual fidelity.
