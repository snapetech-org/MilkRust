import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { spawn } from 'node:child_process';

const repoRoot = path.resolve(new URL('..', import.meta.url).pathname);
const communityRoot = path.join(repoRoot, 'content/community-unlicensed');
const outputPath = path.join(repoRoot, 'content/generated/community-pack-summary.json');
const markdownOutputPath = path.join(repoRoot, 'content/generated/COMMUNITY_PACK_SUMMARY.md');
const args = process.argv.slice(2);
const write = args.includes('--write');
const compat = args.includes('--compat');
const limitArg = args.find((arg) => arg.startsWith('--limit='));
const limit = limitArg ? Number(limitArg.split('=')[1]) : 0;

const runJson = (command, commandArgs) => new Promise((resolve, reject) => {
  const child = spawn(command, commandArgs, {
    cwd: repoRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', (data) => {
    stdout += data;
  });
  child.stderr.on('data', (data) => {
    stderr += data;
  });
  child.on('exit', (code) => {
    if (code === 0) {
      try {
        resolve(JSON.parse(stdout));
      } catch (error) {
        reject(new Error(`failed to parse JSON from ${command}: ${error.message}`));
      }
    } else {
      reject(new Error(`${command} ${commandArgs.join(' ')} exited with ${code}: ${stderr || stdout}`));
    }
  });
});

const readJson = async (file) => JSON.parse(await fs.readFile(file, 'utf8'));

const listPackDirs = async () => {
  const entries = await fs.readdir(communityRoot, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(communityRoot, entry.name))
    .sort();
};

const extensionCounts = async (root, counts = {}) => {
  const entries = await fs.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      await extensionCounts(fullPath, counts);
    } else if (entry.isFile()) {
      const extension = path.extname(entry.name).toLowerCase() || '<none>';
      counts[extension] = (counts[extension] || 0) + 1;
    }
  }
  return counts;
};

const fingerprintPack = async (root) => {
  const files = [];
  const walk = async (directory, prefix = '') => {
    const entries = await fs.readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
    for (const entry of entries) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      const fullPath = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`community pack contains a symlink: ${fullPath}`);
      if (entry.isDirectory()) await walk(fullPath, relative);
      else if (entry.isFile()) files.push({ fullPath, relative });
    }
  };
  await walk(root);
  const digest = createHash('sha256');
  for (const file of files) {
    const name = Buffer.from(file.relative.replaceAll('\\', '/'), 'utf8');
    const contents = await fs.readFile(file.fullPath);
    digest.update(`${name.length}:`);
    digest.update(name);
    digest.update(`${contents.length}:`);
    digest.update(contents);
  }
  return { sourceFileCount: files.length, corpusSha256: digest.digest('hex') };
};

const runCompatReport = async (sourceDir) => {
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
    fileCount: report.totalCount,
    presetCount: report.presetCount,
    parsedPresetCount: report.presetCount,
    supportedCount: report.supportedCount,
    unsupportedCount: report.unsupportedCount,
    webGpuSupportedCount: report.webGpuSupportedCount,
    webGpuUnsupportedCount: report.webGpuUnsupportedCount,
    unsupportedFunctions: report.unsupportedFunctions,
    unsupportedShaderSections: report.unsupportedShaderSections,
    webGpuUnsupportedShaderSections: report.webGpuUnsupportedShaderSections,
  };
};

const writeCompatSubset = async (packDir, manifest, maxFiles) => {
  if (!manifest.presets.length) return null;
  if (!maxFiles) {
    return {
      mode: 'full',
      ...(await runCompatReport(packDir)),
    };
  }
  const subsetDir = await fs.mkdtemp(path.join('/tmp', 'milkrust-community-compat-'));
  try {
    const selected = manifest.presets.slice(0, maxFiles);
    for (const preset of selected) {
      const source = path.join(packDir, preset.file);
      const target = path.join(subsetDir, `${preset.id}${path.extname(preset.file) || '.milk'}`);
      await fs.copyFile(source, target);
    }
    return {
      mode: 'sample',
      limit: maxFiles,
      ...(await runCompatReport(subsetDir)),
    };
  } finally {
    await fs.rm(subsetDir, { recursive: true, force: true });
  }
};

const main = async () => {
  const packDirs = await listPackDirs();
  const packs = [];
  for (const packDir of packDirs) {
    const manifestPath = path.join(packDir, 'manifest.json');
    const manifest = await readJson(manifestPath);
    const counts = await extensionCounts(packDir);
    const fingerprint = await fingerprintPack(packDir);
    const pack = {
      id: manifest.id,
      name: manifest.name,
      path: path.relative(repoRoot, packDir),
      license: manifest.license,
      sourceUrls: manifest.sourceUrls || [],
      presetCount: manifest.presets?.length || 0,
      textureCount: manifest.textures?.length || 0,
      playlistCount: manifest.playlist?.length || 0,
      ...fingerprint,
      byExtension: Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right))),
    };
    if (compat) {
      pack.compatibility = await writeCompatSubset(packDir, manifest, limit);
    }
    packs.push(pack);
  }

  const summary = {
    reportSchemaVersion: 2,
    generatedAt: new Date().toISOString(),
    packCount: packs.length,
    totalPresets: packs.reduce((total, pack) => total + pack.presetCount, 0),
    totalTextures: packs.reduce((total, pack) => total + pack.textureCount, 0),
    compatibilityMode: compat ? (limit ? 'sample' : 'full') : 'none',
    compatibilityLimit: compat && limit ? limit : 0,
    compatibilityReportSchemaVersion: compat ? 2 : null,
    reportInterpretation: 'Static parser/function/shader-subset results only; rendering and visual fidelity are not assessed by this report.',
    analysisScope: compat ? 'static parser/function/shader-subset analysis only' : 'not-run',
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

function renderMarkdown(summary) {
  const rows = summary.packs.map((pack) => {
    const corpus = `${pack.sourceFileCount} files · `${pack.corpusSha256}``;
    const compatibility = pack.compatibility
      ? `${pack.compatibility.supportedCount}/${pack.compatibility.fileCount || pack.compatibility.presetCount} files pass the current static subset (${pack.compatibility.parsedPresetCount || pack.compatibility.presetCount} parsed presets)`
      : 'not sampled';
    const rendererEvidence = pack.compatibility
      ? `${pack.compatibility.renderedCount} rendered; ${pack.compatibility.fidelityStatus}`
      : 'not assessed';
    const blockers = pack.compatibility
      ? renderList([
        ...pack.compatibility.unsupportedFunctions,
        ...pack.compatibility.unsupportedShaderSections,
        ...pack.compatibility.webGpuUnsupportedShaderSections,
      ])
      : '';
    return `| ${pack.id} | ${corpus} | ${pack.presetCount} | ${pack.textureCount} | ${compatibility} | ${rendererEvidence} | ${blockers || 'none in current static subset'} |`;
  }).join('\n');

  return `# Community Pack Summary

Generated by \`node tools/report-milkrust-community-packs.mjs\`.

Community-unlicensed packs are imported into \`content/community-unlicensed\` for compatibility work and opt-in builds. They are excluded from default builds by catalog policy.

## Totals

- Packs: ${summary.packCount}
- Presets: ${summary.totalPresets}
- Textures: ${summary.totalTextures}
- Compatibility mode: ${summary.compatibilityMode === 'sample' ? `sample (${summary.compatibilityLimit})` : summary.compatibilityMode}
- Analysis scope: ${summary.packs.find((pack) => pack.compatibility)?.compatibility.analysisScope || 'not run'}
- Render/fidelity status: ${summary.packs.find((pack) => pack.compatibility)?.compatibility.renderStatus || 'not run'} / ${summary.packs.find((pack) => pack.compatibility)?.compatibility.fidelityStatus || 'not assessed'}

## Packs

| Pack | Corpus snapshot | Presets | Textures | Static subset analysis | Renderer evidence | Static-subset blockers |
| --- | --- | ---: | ---: | --- | --- | --- |
${rows}
`;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
