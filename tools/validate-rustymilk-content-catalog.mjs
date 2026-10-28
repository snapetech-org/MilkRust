import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';

const repoRoot = path.resolve(new URL('..', import.meta.url).pathname);
const catalogPath = path.join(repoRoot, 'content/catalog.json');
const downloadsCatalogPath = path.join(repoRoot, 'content/downloads/catalog.json');
const allowedCopyPolicies = new Set([
  'include',
  'optional-download',
  'link',
  'review',
  'reject',
  'community-unlicensed',
]);
const vendorableLicenses = new Set([
  '0BSD',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'CC-BY-3.0',
  'CC-BY-4.0',
  'CC-BY-SA-3.0',
  'CC-BY-SA-4.0',
  'CC0-1.0',
  'GPL-2.0-only',
  'GPL-2.0-or-later',
  'GPL-3.0-only',
  'GPL-3.0-or-later',
  'ISC',
  'LGPL-2.1-only',
  'LGPL-2.1-or-later',
  'LGPL-3.0-only',
  'LGPL-3.0-or-later',
  'MIT',
  'Unlicense',
  'Zlib',
]);

const catalog = JSON.parse(await fs.readFile(catalogPath, 'utf8'));
const downloadsCatalog = JSON.parse(await fs.readFile(downloadsCatalogPath, 'utf8'));
const errors = [];
const ids = new Set();

if (catalog.schemaVersion !== 1) {
  errors.push('catalog.schemaVersion must be 1');
}
if (!Array.isArray(catalog.entries)) {
  errors.push('catalog.entries must be an array');
}

for (const [index, entry] of (catalog.entries || []).entries()) {
  const label = entry.id || `entries[${index}]`;
  if (!entry.id) errors.push(`${label}: id is required`);
  if (ids.has(entry.id)) errors.push(`${label}: duplicate id`);
  ids.add(entry.id);
  if (!entry.name) errors.push(`${label}: name is required`);
  if (!entry.kind) errors.push(`${label}: kind is required`);
  if (!entry.status) errors.push(`${label}: status is required`);
  if (!allowedCopyPolicies.has(entry.copyPolicy)) {
    errors.push(`${label}: copyPolicy must be one of ${[...allowedCopyPolicies].join(', ')}`);
  }
  if (!entry.license) errors.push(`${label}: license is required`);
  if (!entry.source?.path && !entry.source?.url) {
    errors.push(`${label}: source.path or source.url is required`);
  }
  if (entry.copyPolicy === 'include' && !vendorableLicenses.has(entry.license)) {
    errors.push(`${label}: included content must use an explicitly vendorable license`);
  }
  if ((entry.copyPolicy === 'include' || entry.copyPolicy === 'community-unlicensed') && entry.source?.path) {
    const sourcePath = path.resolve(repoRoot, entry.source.path);
    try {
      await fs.access(sourcePath);
    } catch {
      errors.push(`${label}: local source path does not exist: ${entry.source.path}`);
    }
  }
  if (
    entry.license === 'NOASSERTION'
    && !['link', 'review', 'community-unlicensed'].includes(entry.copyPolicy)
  ) {
    errors.push(`${label}: NOASSERTION content must be link-only, review, or community-unlicensed`);
  }
  if (entry.copyPolicy === 'community-unlicensed' && entry.defaultBuild !== false) {
    errors.push(`${label}: community-unlicensed content must set defaultBuild to false`);
  }
}

if (downloadsCatalog.schemaVersion !== 1) errors.push('downloads catalog.schemaVersion must be 1');
if (!Array.isArray(downloadsCatalog.packs)) errors.push('downloads catalog.packs must be an array');
if (!Array.isArray(downloadsCatalog.linkOnlySources)) {
  errors.push('downloads catalog.linkOnlySources must be an array');
}
const downloadableIds = new Set();
for (const [index, entry] of (downloadsCatalog.packs || []).entries()) {
  const label = entry.id || `downloads.packs[${index}]`;
  if (!entry.id || downloadableIds.has(entry.id)) errors.push(`${label}: id is missing or duplicated`);
  downloadableIds.add(entry.id);
  if (entry.distributionStatus !== 'redistributable') {
    errors.push(`${label}: download catalog entries must have redistributable status`);
  }
  if (!vendorableLicenses.has(entry.license)) errors.push(`${label}: license is not in the vendorable allowlist`);
  if (!/^([a-f0-9]{64})$/i.test(entry.sha256 || '')) errors.push(`${label}: sha256 must be a 64-digit hex digest`);
  if (!Array.isArray(entry.licenseFiles) || entry.licenseFiles.length === 0) {
    errors.push(`${label}: licenseFiles must list a shipped notice`);
  }
  if (typeof entry.archive !== 'string' || path.basename(entry.archive) !== entry.archive) {
    errors.push(`${label}: archive must be a filename under content/downloads`);
    continue;
  }
  const archivePath = path.join(repoRoot, 'content/downloads', entry.archive);
  try {
    const bytes = await fs.readFile(archivePath);
    const digest = createHash('sha256').update(bytes).digest('hex');
    if (bytes.length !== entry.archiveBytes) errors.push(`${label}: archiveBytes does not match the archive`);
    if (digest !== entry.sha256) errors.push(`${label}: archive SHA-256 does not match the archive`);
  } catch {
    errors.push(`${label}: archive does not exist: ${entry.archive}`);
  }
  let archiveUrl;
  try {
    archiveUrl = new URL(entry.archiveUrl);
  } catch {
    errors.push(`${label}: archiveUrl must be a URL`);
  }
  if (archiveUrl && (archiveUrl.protocol !== 'https:' || archiveUrl.hostname !== 'raw.githubusercontent.com')) {
    errors.push(`${label}: archiveUrl must use the pinned raw.githubusercontent.com host`);
  }
}
for (const [index, source] of (downloadsCatalog.linkOnlySources || []).entries()) {
  const label = source.id || `downloads.linkOnlySources[${index}]`;
  if (source.distributionStatus !== 'link-only' || source.license !== 'NOASSERTION') {
    errors.push(`${label}: unresolved community sources must remain NOASSERTION and link-only`);
  }
  try {
    if (new URL(source.sourceUrl).protocol !== 'https:') errors.push(`${label}: sourceUrl must use HTTPS`);
  } catch {
    errors.push(`${label}: sourceUrl must be a URL`);
  }
}

if (errors.length) {
  console.error(errors.join('\n'));
  process.exit(1);
}

console.log(`MilkRust content catalog valid: ${catalog.entries.length} entries`);
