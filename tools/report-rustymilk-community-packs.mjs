import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const communityRoot = path.join(repoRoot, 'content/community-unlicensed');
const outputPath = path.join(repoRoot, 'content/generated/community-pack-summary.json');
const markdownOutputPath = path.join(repoRoot, 'content/generated/COMMUNITY_PACK_SUMMARY.md');
const args = process.argv.slice(2);
const write = args.includes('--write');
const compat = args.includes('--compat');
const limitArg = args.find((arg) => arg.startsWith('--limit='));
const limit = limitArg ? Number(limitArg.split('=')[1]) : 0;

const MAX_COMPAT_REPORT_STDOUT = 128 * 1024 * 1024;
const MAX_COMPAT_REPORT_STDERR = 8 * 1024 * 1024;

const runJson = (command, commandArgs) => new Promise((resolve, reject) => {
  const child = spawn(command, commandArgs, {
    cwd: repoRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stdout = '';
  let stderr = '';
  let stdoutBytes = 0;
  let stderrBytes = 0;
  let outputLimitExceeded = false;
  child.stdout.on('data', (data) => {
    stdoutBytes += data.length;
    if (stdoutBytes > MAX_COMPAT_REPORT_STDOUT) {
      outputLimitExceeded = true;
      child.kill('SIGKILL');
      return;
    }
    stdout += data;
  });
  child.stderr.on('data', (data) => {
    stderrBytes += data.length;
    if (stderrBytes > MAX_COMPAT_REPORT_STDERR) {
      outputLimitExceeded = true;
      child.kill('SIGKILL');
      return;
    }
    stderr += data;
  });
  child.on('error', reject);
  child.on('close', (code, signal) => {
    if (outputLimitExceeded) {
      reject(new Error(`${command} exceeded the bounded compatibility report output limit`));
    } else if (code === 0) {
      try {
        resolve(JSON.parse(stdout));
      } catch (error) {
        reject(new Error(`failed to parse JSON from ${command}: ${error.message}`));
      }
    } else {
      reject(new Error(`${command} ${commandArgs.join(' ')} exited with ${code ?? signal}: ${stderr || stdout}`));
    }
  });
});

const readJson = async (file) => JSON.parse(await fs.readFile(file, 'utf8'));

const listPackDirs = async () => {
  try {
    const entries = await fs.readdir(communityRoot, { withFileTypes: true });
    const packDirs = [];
    const errors = [];
    for (const entry of entries) {
      if (entry.isSymbolicLink()) {
        errors.push({ path: entry.name, error: 'symbolic links are not followed' });
      } else if (entry.isDirectory()) {
        packDirs.push(path.join(communityRoot, entry.name));
      }
    }
    packDirs.sort();
    return { packDirs, errors };
  } catch (error) {
    return { packDirs: [], errors: [{ path: '.', error: error.message }] };
  }
};

const MAX_PACK_SCAN_ENTRIES = 250_000;
const MAX_PACK_SCAN_ERRORS = 256;
const MAX_PACK_FILE_BYTES = 64 * 1024 * 1024;
const MAX_PACK_TOTAL_BYTES = 2 * 1024 * 1024 * 1024;

export const fingerprintPack = async (root) => {
  const files = [];
  const inventoryErrors = [];
  const byExtension = {};
  let visitedEntries = 0;
  let scanTruncated = false;
  let errorsTruncated = false;
  const recordError = (file, error) => {
    if (inventoryErrors.length + 1 < MAX_PACK_SCAN_ERRORS) {
      inventoryErrors.push({ file, error: String(error) });
    } else if (!errorsTruncated) {
      inventoryErrors.push({ file, error: 'additional inventory errors omitted after the diagnostic limit' });
      errorsTruncated = true;
    }
  };
  const walk = async (directory, prefix = '') => {
    let entries;
    try {
      entries = await fs.readdir(directory, { withFileTypes: true });
    } catch (error) {
      recordError(prefix || '.', error.message);
      return;
    }
    entries.sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
    for (const entry of entries) {
      if (visitedEntries >= MAX_PACK_SCAN_ENTRIES) {
        if (!scanTruncated) recordError(prefix || '.', `pack scan stopped at the ${MAX_PACK_SCAN_ENTRIES} entry limit`);
        scanTruncated = true;
        break;
      }
      visitedEntries += 1;
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      const fullPath = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        recordError(relative, 'symbolic links are not followed');
      } else if (entry.isDirectory()) {
        await walk(fullPath, relative);
      } else if (entry.isFile()) {
        files.push({ fullPath, relative });
        const extension = path.extname(entry.name).toLowerCase() || '<none>';
        byExtension[extension] = (byExtension[extension] || 0) + 1;
      } else {
        recordError(relative, 'unsupported filesystem entry type');
      }
      if (scanTruncated) break;
    }
  };
  await walk(root);

  const digest = createHash('sha256');
  let totalBytes = 0;
  for (const file of files) {
    const name = Buffer.from(file.relative.replaceAll('\\', '/'), 'utf8');
    let handle;
    try {
      handle = await fs.open(file.fullPath, 'r');
      const metadata = await handle.stat();
      if (!metadata.isFile()) throw new Error('source path is not a regular file');
      if (metadata.size > MAX_PACK_FILE_BYTES) throw new Error(`file exceeds the ${MAX_PACK_FILE_BYTES} byte inventory limit`);
      if (totalBytes + metadata.size > MAX_PACK_TOTAL_BYTES) {
        throw new Error(`pack exceeds the ${MAX_PACK_TOTAL_BYTES} byte inventory limit`);
      }
      totalBytes += metadata.size;
      digest.update(`${name.length}:`);
      digest.update(name);
      digest.update(`${metadata.size}:`);
      const buffer = Buffer.allocUnsafe(64 * 1024);
      let offset = 0;
      while (offset < metadata.size) {
        const length = Math.min(buffer.length, metadata.size - offset);
        const { bytesRead } = await handle.read(buffer, 0, length, offset);
        if (bytesRead === 0) throw new Error('source file changed or ended during inventory');
        digest.update(buffer.subarray(0, bytesRead));
        offset += bytesRead;
      }
      const extra = Buffer.allocUnsafe(1);
      const { bytesRead: extraBytes } = await handle.read(extra, 0, 1, metadata.size);
      if (extraBytes !== 0) throw new Error('source file grew during inventory');
    } catch (error) {
      recordError(file.relative, error.message);
    } finally {
      if (handle) await handle.close().catch(() => {});
    }
  }
  const sortedExtensions = Object.fromEntries(Object.entries(byExtension).sort(([left], [right]) => left.localeCompare(right)));
  return {
    sourceFileCount: files.length,
    corpusSha256: inventoryErrors.length === 0 ? digest.digest('hex') : null,
    fileInventoryComplete: inventoryErrors.length === 0,
    inventoryErrorCount: inventoryErrors.length,
    inventoryErrors,
    byExtension: sortedExtensions,
  };
};

const normalizeManifestPath = (value) => path.posix.normalize(
  String(value ?? '').replaceAll('\\', '/').replace(/^(?:\.\/)+/, ''),
);

export const normalizeCompatibilityReport = (report, sourceDir, generatedSources = new Map()) => {
  if (!Array.isArray(report.entries) || report.entries.length !== report.totalCount) {
    throw new Error(`compat report entries (${report.entries?.length ?? 'missing'}) do not account for all ${report.totalCount} discovered files`);
  }
  const root = path.resolve(sourceDir);
  const sourceFile = (fileName) => {
    const raw = typeof fileName === 'string' ? fileName : '';
    const generated = generatedSources.get(path.basename(raw));
    if (generated) return { ...generated, file: normalizeManifestPath(generated.file) };
    const absolute = path.isAbsolute(raw) ? path.resolve(raw) : path.resolve(repoRoot, raw);
    const relative = path.relative(root, absolute);
    const withinRoot = relative !== '..'
      && !relative.startsWith(`..${path.sep}`)
      && !path.isAbsolute(relative);
    return {
      id: null,
      file: normalizeManifestPath(withinRoot ? relative : path.basename(absolute)),
    };
  };
  const files = report.entries.map((entry) => {
    const mapped = sourceFile(entry.fileName);
    return {
      id: mapped.id || entry.id,
      file: mapped.file,
      format: entry.format,
      sourceReadStatus: entry.sourceReadStatus,
      parseStatus: entry.parseStatus,
      featureSubsetStatus: entry.featureSubsetStatus,
      presetCount: entry.presetCount,
      supported: entry.supported,
      webGpuSupported: entry.webGpuSupported,
      unsupportedFunctions: entry.unsupportedFunctions || [],
      shaderDiagnostics: entry.shaderDiagnostics || [],
      rendererFeatures: entry.rendererFeatures || [],
      shaderSections: entry.shaderSections || [],
      webGpuRendererFeatures: entry.webGpuRendererFeatures || [],
      webGpuShaderSections: entry.webGpuShaderSections || [],
      presets: (entry.presetReports || []).map((preset) => ({
        index: preset.index,
        title: preset.title,
        error: preset.error,
        parseErrors: preset.parseErrors || [],
        supportedByCurrentSubset: preset.supportedByCurrentSubset,
        webGpuShaderSubsetSupported: preset.webGpuShaderSubsetSupported,
        rendererFeatures: preset.rendererFeatures || [],
        shaderSections: preset.shaderSections || [],
        webGpuRendererFeatures: preset.webGpuRendererFeatures || [],
        webGpuShaderSections: preset.webGpuShaderSections || [],
      })),
    };
  });
  const normalizeIssues = (items = [], field = 'fileName') => items.map((issue) => {
    const mapped = sourceFile(issue[field]);
    const { [field]: ignored, ...detail } = issue;
    return { ...detail, ...(mapped.id ? { id: mapped.id } : {}), file: mapped.file };
  });
  const fileReadErrors = normalizeIssues(report.fileReadErrors);
  const inputRejections = normalizeIssues(report.inputRejections);
  const encodingWarnings = normalizeIssues(report.encodingWarnings);
  const discoveryErrors = (report.discoveryErrors || []).map((issue) => {
    const mapped = sourceFile(issue.path);
    const { path: ignored, ...detail } = issue;
    return { ...detail, directory: mapped.file };
  });
  for (const [items, count, label] of [
    [fileReadErrors, report.fileReadErrorCount || 0, 'file-read'],
    [inputRejections, report.inputRejectionCount || 0, 'input-rejection'],
    [encodingWarnings, report.encodingWarningCount || 0, 'encoding'],
    [discoveryErrors, report.discoveryErrorCount || 0, 'discovery'],
  ]) {
    if (items.length !== count) throw new Error(`compat report ${label} details do not match its count`);
  }
  return {
    fileCount: report.totalCount,
    fileAccountingComplete: report.fileAccountingComplete === true && discoveryErrors.length === 0,
    discoveryErrorCount: report.discoveryErrorCount || 0,
    discoveryErrors,
    fileReadErrorCount: report.fileReadErrorCount || 0,
    fileReadErrors,
    inputRejectionCount: report.inputRejectionCount || 0,
    inputRejections,
    encodingWarningCount: report.encodingWarningCount || 0,
    encodingWarnings,
    fileResults: files,
  };
};

export const accountForManifestFiles = (compatibility, presets, manifestPresetFileCount = presets.length) => {
  const remaining = new Map();
  for (const result of compatibility.fileResults) {
    const key = normalizeManifestPath(result.file);
    const matches = remaining.get(key) || [];
    matches.push(result);
    remaining.set(key, matches);
  }
  const accounted = [];
  const seenManifestPaths = new Set();
  let missingManifestFileCount = 0;
  for (const preset of presets) {
    const file = normalizeManifestPath(preset.file);
    const duplicate = seenManifestPaths.has(file);
    seenManifestPaths.add(file);
    const matches = remaining.get(file) || [];
    const result = matches.shift();
    if (matches.length) remaining.set(file, matches);
    else remaining.delete(file);
    if (result && !duplicate) {
      accounted.push({ ...result, id: preset.id, file, manifestStatus: 'listed' });
      continue;
    }
    missingManifestFileCount += 1;
    const status = preset.compatibilitySourceError
      ? 'manifest-source-error'
      : duplicate
        ? 'duplicate-manifest-path'
        : compatibility.fileAccountingComplete
          ? 'manifest-file-not-discovered'
          : 'not-discovered-scan-incomplete';
    accounted.push({
      id: preset.id,
      file,
      format: path.posix.extname(file).replace(/^\./, '').toLowerCase() || 'milk',
      sourceReadStatus: status,
      parseStatus: 'not-run',
      featureSubsetStatus: 'not-evaluated',
      presetCount: 0,
      supported: false,
      webGpuSupported: false,
      unsupportedFunctions: [],
      shaderDiagnostics: [],
      rendererFeatures: [],
      shaderSections: [],
      webGpuRendererFeatures: [],
      webGpuShaderSections: [],
      presets: [],
      ...(preset.compatibilitySourceError ? { error: preset.compatibilitySourceError } : {}),
      manifestStatus: preset.compatibilitySourceError ? 'unavailable' : duplicate ? 'duplicate' : 'missing',
    });
  }
  const unlisted = [...remaining.values()].flat().map((result) => ({
    ...result,
    manifestStatus: 'not-listed',
  }));
  return {
    ...compatibility,
    fileResults: [...accounted, ...unlisted],
    manifestPresetFileCount,
    analyzedManifestFileCount: presets.length,
    missingManifestFileCount,
    unlistedDiscoveredFileCount: unlisted.length,
    manifestAccountingComplete: compatibility.fileAccountingComplete
      && missingManifestFileCount === 0
      && unlisted.length === 0
      && presets.length === manifestPresetFileCount,
    unsupportedCount: (compatibility.unsupportedCount || 0) + missingManifestFileCount,
    webGpuUnsupportedCount: (compatibility.webGpuUnsupportedCount || 0) + missingManifestFileCount,
  };
};

const runCompatReport = async (sourceDir, generatedSources = new Map()) => {
  const report = await runJson('cargo', [
    'run',
    '-q',
    '-p',
    'milkrust-cli',
    '--',
    'compat',
    sourceDir,
  ]);
  return {
    reportSchemaVersion: report.reportSchemaVersion,
    analysisScope: report.analysisScope,
    renderStatus: report.renderStatus,
    fidelityStatus: report.fidelityStatus,
    renderedCount: report.renderedCount,
    visuallyReviewedCount: report.visuallyReviewedCount,
    presetCount: report.presetCount,
    parsedPresetCount: report.presetCount,
    supportedCount: report.supportedCount,
    unsupportedCount: report.unsupportedCount,
    webGpuSupportedCount: report.webGpuSupportedCount,
    webGpuUnsupportedCount: report.webGpuUnsupportedCount,
    unsupportedFunctions: report.unsupportedFunctions,
    unsupportedRendererFeatures: report.unsupportedRendererFeatures,
    unsupportedShaderSections: report.unsupportedShaderSections,
    webGpuUnsupportedRendererFeatures: report.webGpuUnsupportedRendererFeatures,
    webGpuUnsupportedShaderSections: report.webGpuUnsupportedShaderSections,
    ...normalizeCompatibilityReport(report, sourceDir, generatedSources),
  };
};

const validateManifestPaths = (packDir, presets) => presets.map((preset, index) => {
  const entry = preset && typeof preset === 'object' && !Array.isArray(preset) ? { ...preset } : {};
  if (typeof entry.file !== 'string' || !entry.file) {
    return {
      ...entry,
      id: entry.id || `invalid-entry-${index}`,
      file: `<invalid-manifest-entry-${index}>`,
      compatibilitySourceError: 'manifest preset has no valid file path',
    };
  }
  entry.id = entry.id || path.posix.basename(entry.file, path.posix.extname(entry.file));
  const source = path.resolve(packDir, entry.file);
  const relative = path.relative(path.resolve(packDir), source);
  if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    return { ...entry, compatibilitySourceError: 'manifest preset path escapes pack root' };
  }
  return entry;
});

const writeCompatSubset = async (packDir, manifest, maxFiles) => {
  if (!manifest.presets.length) return null;
  if (!maxFiles) {
    const presets = validateManifestPaths(packDir, manifest.presets);
    const compatibility = await runCompatReport(packDir);
    return {
      mode: 'full',
      ...accountForManifestFiles(compatibility, presets),
    };
  }
  const subsetDir = await fs.mkdtemp(path.join('/tmp', 'milkrust-community-compat-'));
  try {
    const selected = validateManifestPaths(packDir, manifest.presets.slice(0, maxFiles));
    const generatedSources = new Map();
    for (const [index, preset] of selected.entries()) {
      if (preset.compatibilitySourceError) continue;
      const source = path.resolve(packDir, preset.file);
      const generatedName = `sample-${String(index).padStart(6, '0')}${path.extname(preset.file) || '.milk'}`;
      try {
        const metadata = await fs.lstat(source);
        if (metadata.isSymbolicLink() || !metadata.isFile()) {
          throw new Error('manifest preset path is not a regular file');
        }
        await fs.copyFile(source, path.join(subsetDir, generatedName));
        generatedSources.set(generatedName, { id: preset.id, file: preset.file });
      } catch (error) {
        preset.compatibilitySourceError = `sample source could not be copied: ${error.message}`;
      }
    }
    const compatibility = await runCompatReport(subsetDir, generatedSources);
    return {
      mode: 'sample',
      limit: maxFiles,
      ...accountForManifestFiles(compatibility, selected, manifest.presets.length),
    };
  } finally {
    await fs.rm(subsetDir, { recursive: true, force: true });
  }
};

const main = async () => {
  const { packDirs, errors: communityRootErrors } = await listPackDirs();
  const packs = [];
  for (const packDir of packDirs) {
    const manifestPath = path.join(packDir, 'manifest.json');
    const fingerprint = await fingerprintPack(packDir);
    let manifest;
    let manifestError = null;
    let manifestStatus = 'readable';
    try {
      manifest = await readJson(manifestPath);
      if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest)) {
        throw new Error('manifest root must be a JSON object');
      }
      if (manifest.presets == null) throw new Error('manifest omitted the required preset inventory');
      for (const field of ['presets', 'textures', 'playlist']) {
        if (manifest[field] == null) manifest[field] = [];
        else if (!Array.isArray(manifest[field])) throw new Error(`manifest ${field} must be an array`);
      }
    } catch (error) {
      manifestError = error.message;
      manifestStatus = error instanceof SyntaxError ? 'invalid-json' : error.code ? 'read-failed' : 'invalid';
      const fallbackName = path.basename(packDir);
      manifest = {
        id: fallbackName,
        name: fallbackName,
        license: 'unknown',
        sourceUrls: [],
        presets: [],
        textures: [],
        playlist: [],
      };
    }
    const pack = {
      id: manifest.id || path.basename(packDir),
      name: manifest.name || manifest.id || path.basename(packDir),
      path: path.relative(repoRoot, packDir),
      license: manifest.license || 'unknown',
      sourceUrls: manifest.sourceUrls || [],
      presetCount: manifest.presets?.length || 0,
      textureCount: manifest.textures?.length || 0,
      playlistCount: manifest.playlist?.length || 0,
      manifestStatus,
      ...(manifestError ? { manifestError } : {}),
      ...fingerprint,
    };
    if (compat && manifestStatus === 'readable') {
      pack.compatibility = await writeCompatSubset(packDir, manifest, limit);
    } else if (compat) {
      pack.compatibilityStatus = `not-run-manifest-${manifestStatus}`;
    }
    packs.push(pack);
  }

  const summary = {
    reportSchemaVersion: 3,
    generatedAt: new Date().toISOString(),
    packCount: packs.length,
    totalPresets: packs.reduce((total, pack) => total + pack.presetCount, 0),
    totalPresetsComplete: packs.every((pack) => pack.manifestStatus === 'readable'),
    manifestErrorCount: packs.filter((pack) => pack.manifestStatus !== 'readable').length,
    totalTextures: packs.reduce((total, pack) => total + pack.textureCount, 0),
    totalSourceFiles: packs.reduce((total, pack) => total + pack.sourceFileCount, 0),
    sourceInventoryComplete: communityRootErrors.length === 0 && packs.every((pack) => pack.fileInventoryComplete),
    sourceInventoryErrorCount: communityRootErrors.length + packs.reduce((total, pack) => total + pack.inventoryErrorCount, 0),
    communityRootErrors,
    compatibilityMode: compat ? (limit ? 'sample' : 'full') : 'none',
    compatibilityLimit: compat && limit ? limit : 0,
    compatibilityReportSchemaVersion: compat ? 3 : null,
    fileAccounting: compat ? 'every discovered preset file and manifest-listed missing file has a compact result; scan, read, input-limit and decoding diagnostics are retained' : 'not-run',
    reportInterpretation: 'Static parser/function/shader/render-feature results only; file accounting is explicit, but rendering and visual fidelity are not assessed by this report.',
    analysisScope: compat ? 'static parser/function/shader-subset/render-feature analysis only' : 'not-run',
    renderStatus: 'not-run',
    fidelityStatus: 'not-assessed',
    renderedCount: 0,
    visuallyReviewedCount: 0,
    packs,
  };

  if (write) {
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, `${JSON.stringify(summary, null, 2)}\n`);
    await fs.writeFile(markdownOutputPath, renderMarkdown(summary));
  }
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
};

const renderList = (values = []) => values.length
  ? values.map((value) => `\`${value}\``).join(', ')
  : '';

export function renderMarkdown(summary) {
  const escapeMarkdown = (value) => String(value ?? '').replaceAll('\\', '\\\\').replaceAll('`', '\\`').replaceAll('\n', ' ');
  const rows = summary.packs.map((pack) => {
    const corpus = pack.corpusSha256
      ? `${pack.sourceFileCount} files · \`${pack.corpusSha256}\``
      : `${pack.sourceFileCount} files · fingerprint unavailable`;
    const compatibility = pack.compatibility
      ? `${pack.compatibility.supportedCount}/${pack.compatibility.fileResults?.length || pack.compatibility.manifestPresetFileCount || pack.compatibility.presetCount} files pass the current static subset (${pack.compatibility.parsedPresetCount || pack.compatibility.presetCount} parsed presets; ${pack.compatibility.missingManifestFileCount || 0} manifest files not discovered; ${pack.compatibility.fileReadErrorCount || 0} read errors; ${pack.compatibility.inputRejectionCount || 0} input-limit/type rejections; ${pack.compatibility.discoveryErrorCount || 0} scan errors; ${pack.compatibility.encodingWarningCount || 0} non-UTF-8 decoded losslessly)`
      : 'not sampled';
    const rendererEvidence = pack.compatibility
      ? `${pack.compatibility.renderedCount} rendered; ${pack.compatibility.fidelityStatus}`
      : 'not assessed';
    const blockers = pack.compatibility
      ? renderList([
        ...pack.compatibility.unsupportedFunctions,
        ...pack.compatibility.unsupportedRendererFeatures,
        ...pack.compatibility.unsupportedShaderSections,
        ...pack.compatibility.webGpuUnsupportedRendererFeatures,
        ...pack.compatibility.webGpuUnsupportedShaderSections,
      ])
      : '';
    return `| ${pack.id} | ${corpus} | ${pack.presetCount} | ${pack.textureCount} | ${compatibility} | ${rendererEvidence} | ${blockers || 'none in current static subset'} |`;
  }).join('\n');

  const communityRootIssues = (summary.communityRootErrors || []).map((item) =>
    `- \`content/community-unlicensed/${escapeMarkdown(item.path)}\`: pack scan failed (${escapeMarkdown(item.error)})`);
  const accounting = [
    ...communityRootIssues,
    ...summary.packs.flatMap((pack) => {
    const inventory = (pack.inventoryErrors || []).map((item) =>
      `- \`${escapeMarkdown(pack.id)}/${escapeMarkdown(item.file)}\`: inventory failed (${escapeMarkdown(item.error)})`);
    if (pack.manifestError) inventory.push(`- \`${escapeMarkdown(pack.id)}/manifest.json\`: manifest ${escapeMarkdown(pack.manifestStatus)} (${escapeMarkdown(pack.manifestError)})`);
    if (!pack.compatibility) return inventory;
    const compatibility = pack.compatibility;
    const prefix = (file) => `${escapeMarkdown(pack.id)}/${escapeMarkdown(file)}`;
    const errors = (compatibility.fileReadErrors || []).map((item) =>
      `- \`${prefix(item.file)}\`: read failed (${escapeMarkdown(item.error)})`);
    const inputRejects = (compatibility.inputRejections || []).map((item) =>
      `- \`${prefix(item.file)}\`: ${escapeMarkdown(item.status)} (${escapeMarkdown(item.error)})`);
    const warnings = (compatibility.encodingWarnings || []).map((item) =>
      `- \`${prefix(item.file)}\`: decoded as ${escapeMarkdown(item.encoding)} without loss`);
    const discovery = (compatibility.discoveryErrors || []).map((item) =>
      `- \`${escapeMarkdown(pack.id)}/${escapeMarkdown(item.directory)}\`: scan failed (${escapeMarkdown(item.error)})`);
    const missing = (compatibility.fileResults || [])
      .filter((item) => ['missing', 'duplicate', 'unavailable'].includes(item.manifestStatus))
      .map((item) => `- \`${prefix(item.file)}\`: ${item.manifestStatus === 'duplicate' ? 'duplicate manifest path' : item.error || 'manifest file was not discovered'}`);
    const unlisted = (compatibility.fileResults || [])
      .filter((item) => item.manifestStatus === 'not-listed')
      .map((item) => `- \`${prefix(item.file)}\`: discovered preset file is absent from the pack manifest`);
    return [...inventory, ...errors, ...inputRejects, ...warnings, ...discovery, ...missing, ...unlisted];
    }),
  ];

  return `# Community Pack Summary

Generated by \`node tools/report-rustymilk-community-packs.mjs\`.

Community-unlicensed packs are imported into \`content/community-unlicensed\` for compatibility work and opt-in builds. They are excluded from default builds by catalog policy.

## Totals

- Packs: ${summary.packCount}
- Presets: ${summary.totalPresets} (${summary.totalPresetsComplete ? 'complete' : `incomplete; ${summary.manifestErrorCount} manifest read errors`})
- Source files: ${summary.totalSourceFiles}
- Source inventory: ${summary.sourceInventoryComplete ? 'complete' : `incomplete (${summary.sourceInventoryErrorCount} errors)`}
- Textures: ${summary.totalTextures}
- Compatibility mode: ${summary.compatibilityMode === 'sample' ? `sample (${summary.compatibilityLimit})` : summary.compatibilityMode}
- Analysis scope: ${summary.packs.find((pack) => pack.compatibility)?.compatibility.analysisScope || 'not run'}
- Render/fidelity status: ${summary.packs.find((pack) => pack.compatibility)?.compatibility.renderStatus || 'not run'} / ${summary.packs.find((pack) => pack.compatibility)?.compatibility.fidelityStatus || 'not assessed'}

## Packs

| Pack | Corpus snapshot | Presets | Textures | Static subset analysis | Renderer evidence | Static-subset blockers |
| --- | --- | ---: | ---: | --- | --- | --- |
${rows}

## File accounting exceptions

${accounting.length ? accounting.join('\n') : 'No unreadable files or decoding diagnostics were reported.'}
`;
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath && import.meta.url === invokedPath) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
