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

function safePackPath(value) {
  if (typeof value !== 'string') return null;
  const normalized = value.replaceAll('\\', '/');
  const parts = normalized.split('/');
  if (
    !normalized
    || normalized.startsWith('/')
    || normalized.includes(':')
    || normalized.includes('\0')
    || parts.some((part) => !part || part === '.' || part === '..')
  ) return null;
  return parts.join('/');
}

function isHttpsUrl(value) {
  try {
    return new URL(value).protocol === 'https:';
  } catch {
    return false;
  }
}

async function verifyDownloadSource(entry, label) {
  const manifestRelative = safePackPath(entry.sourceManifest);
  if (!manifestRelative) {
    errors.push(`${label}: sourceManifest must be a safe repository-relative path`);
    return;
  }
  const manifestPath = path.resolve(repoRoot, manifestRelative);
  if (!manifestPath.startsWith(`${repoRoot}${path.sep}`)) {
    errors.push(`${label}: sourceManifest escapes the repository`);
    return;
  }
  let manifest;
  try {
    manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
  } catch {
    errors.push(`${label}: source manifest is missing or unreadable`);
    return;
  }
  if (
    manifest.id !== entry.id
    || manifest.version !== entry.version
    || manifest.license !== entry.license
    || manifest.distributionStatus !== 'redistributable'
  ) errors.push(`${label}: source manifest metadata does not match the download catalog`);
  if (typeof manifest.contentNotice !== 'string' || !manifest.contentNotice.trim()) {
    errors.push(`${label}: source manifest contentNotice is required`);
  }
  if (!Array.isArray(manifest.sourceUrls) || manifest.sourceUrls.length === 0 || manifest.sourceUrls.some((url) => !isHttpsUrl(url))) {
    errors.push(`${label}: source manifest must contain HTTPS sourceUrls`);
  }
  if (!Array.isArray(manifest.licenseFiles) || manifest.licenseFiles.length === 0) {
    errors.push(`${label}: source manifest must include licenseFiles`);
    return;
  }

  const packRoot = path.dirname(manifestPath);
  const licensePaths = new Set();
  for (const rawPath of manifest.licenseFiles) {
    const relative = safePackPath(rawPath);
    if (!relative || licensePaths.has(relative)) {
      errors.push(`${label}: licenseFiles contains an unsafe or duplicate path`);
      continue;
    }
    licensePaths.add(relative);
    try {
      const notice = await fs.readFile(path.join(packRoot, ...relative.split('/')));
      if (!notice.length) errors.push(`${label}: license notice is empty: ${relative}`);
    } catch {
      errors.push(`${label}: license notice is missing: ${relative}`);
    }
  }

  if (!Array.isArray(manifest.assets) || manifest.assets.length === 0) {
    errors.push(`${label}: source manifest must include per-asset attribution`);
    return;
  }
  const assetPaths = new Set();
  for (const [index, asset] of manifest.assets.entries()) {
    const assetLabel = `${label}: assets[${index}]`;
    if (!asset || typeof asset !== 'object') {
      errors.push(`${assetLabel} must be an object`);
      continue;
    }
    const relative = safePackPath(asset.path);
    const notice = safePackPath(asset.licenseFile);
    if (!relative || assetPaths.has(relative)) errors.push(`${assetLabel} has an unsafe or duplicate path`);
    else assetPaths.add(relative);
    if (!relative || relative === 'manifest.json' || licensePaths.has(relative)) {
      errors.push(`${assetLabel} must identify a content file`);
      continue;
    }
    if (typeof asset.kind !== 'string' || !asset.kind.trim()) errors.push(`${assetLabel} kind is required`);
    if (!vendorableLicenses.has(asset.license)) errors.push(`${assetLabel} license is not explicitly redistributable`);
    if (typeof asset.author !== 'string' || !asset.author.trim()) errors.push(`${assetLabel} author is required`);
    if (!isHttpsUrl(asset.sourceUrl)) errors.push(`${assetLabel} sourceUrl must use HTTPS`);
    if (!notice || !licensePaths.has(notice)) errors.push(`${assetLabel} must reference a shipped license notice`);
    try {
      const info = await fs.lstat(path.join(packRoot, ...relative.split('/')));
      if (!info.isFile() || info.isSymbolicLink()) errors.push(`${assetLabel} is not a regular file`);
    } catch {
      errors.push(`${assetLabel} file is missing`);
    }
  }

  async function walk(directory, prefix = '') {
    const files = [];
    for (const item of await fs.readdir(directory, { withFileTypes: true })) {
      const relative = prefix ? `${prefix}/${item.name}` : item.name;
      if (item.isSymbolicLink()) {
        errors.push(`${label}: source pack contains a symlink: ${relative}`);
      } else if (item.isDirectory()) {
        files.push(...await walk(path.join(directory, item.name), relative));
      } else if (item.isFile()) {
        files.push(relative);
      }
    }
    return files;
  }
  let sourceFiles;
  try {
    sourceFiles = await walk(packRoot);
  } catch {
    errors.push(`${label}: source pack directory is unreadable`);
    return;
  }
  const expectedAssets = new Set(sourceFiles.filter((file) => file !== 'manifest.json' && !licensePaths.has(file)));
  if (assetPaths.size !== expectedAssets.size || [...expectedAssets].some((file) => !assetPaths.has(file))) {
    errors.push(`${label}: per-asset attribution does not cover every content file`);
  }
}

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
  if (typeof entry.contentNotice !== 'string' || !entry.contentNotice.trim()) {
    errors.push(`${label}: contentNotice is required`);
  }
  if (!Array.isArray(entry.sourceUrls) || entry.sourceUrls.length === 0 || entry.sourceUrls.some((url) => !isHttpsUrl(url))) {
    errors.push(`${label}: sourceUrls must contain HTTPS URLs`);
  }
  await verifyDownloadSource(entry, label);
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
  if (archiveUrl && !/^\/[^/]+\/[^/]+\/[a-f0-9]{40}\/content\/downloads\/[^/]+\.rmpack$/i.test(archiveUrl.pathname)) {
    errors.push(`${label}: archiveUrl must pin a commit and point to a .rmpack under content/downloads`);
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
