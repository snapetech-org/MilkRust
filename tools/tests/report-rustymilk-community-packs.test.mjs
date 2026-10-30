import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

import {
  accountForManifestFiles,
  fingerprintPack,
  normalizeCompatibilityReport,
  renderMarkdown,
} from '../report-rustymilk-community-packs.mjs';

test('pack inventory hashes bounded files and counts every discovered extension', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'milkrust-pack-inventory-'));
  try {
    await mkdir(path.join(root, 'presets'));
    await writeFile(path.join(root, 'presets', 'one.milk'), 'name=One');
    await writeFile(path.join(root, 'NOTICE.md'), 'notice');
    const inventory = await fingerprintPack(root);
    assert.equal(inventory.sourceFileCount, 2);
    assert.equal(inventory.fileInventoryComplete, true);
    assert.equal(inventory.inventoryErrorCount, 0);
    assert.equal(inventory.byExtension['.milk'], 1);
    assert.equal(inventory.byExtension['.md'], 1);
    assert.match(inventory.corpusSha256, /^[0-9a-f]{64}$/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('compatibility normalization retains every discovered file and diagnostic', () => {
  const report = {
    totalCount: 3,
    fileAccountingComplete: false,
    discoveryErrorCount: 1,
    discoveryErrors: [{ path: '/workspace/packs/locked', error: 'permission denied' }],
    fileReadErrorCount: 1,
    fileReadErrors: [{ fileName: '/workspace/packs/unreadable.milk', error: 'permission denied' }],
    inputRejectionCount: 1,
    inputRejections: [{
      fileName: '/workspace/packs/oversized.milk',
      status: 'input-limit-rejected',
      error: 'preset exceeds byte limit',
    }],
    encodingWarningCount: 1,
    encodingWarnings: [{
      fileName: '/workspace/packs/legacy.milk',
      encoding: 'windows-1252',
      status: 'decoded-losslessly',
    }],
    entries: [
      {
        id: 'visible', fileName: '/workspace/packs/visible.milk', format: 'milk',
        sourceReadStatus: 'readable-utf8', parseStatus: 'parsed',
        featureSubsetStatus: 'supported-by-current-static-subset', presetCount: 1,
        supported: true, webGpuSupported: true, presetReports: [],
      },
      {
        id: 'unreadable', fileName: '/workspace/packs/unreadable.milk', format: 'milk',
        sourceReadStatus: 'read-failed', parseStatus: 'not-run',
        featureSubsetStatus: 'not-evaluated', presetCount: 0,
        supported: false, webGpuSupported: false, presetReports: [],
      },
      {
        id: 'oversized', fileName: '/workspace/packs/oversized.milk', format: 'milk',
        sourceReadStatus: 'input-limit-rejected', parseStatus: 'rejected-input-limits',
        featureSubsetStatus: 'rejected-input-limits', presetCount: 0,
        supported: false, webGpuSupported: false, presetReports: [],
      },
    ],
  };
  const result = normalizeCompatibilityReport(report, '/workspace/packs');
  assert.equal(result.fileCount, 3);
  assert.equal(result.fileResults.length, 3);
  assert.equal(result.fileResults[0].file, 'visible.milk');
  assert.equal(result.fileResults[1].sourceReadStatus, 'read-failed');
  assert.equal(result.fileResults[2].parseStatus, 'rejected-input-limits');
  assert.equal(result.fileAccountingComplete, false);
  assert.deepEqual(result.fileReadErrors, [
    { error: 'permission denied', file: 'unreadable.milk' },
  ]);
  assert.equal(result.inputRejections[0].file, 'oversized.milk');
  assert.equal(result.encodingWarnings[0].file, 'legacy.milk');
  assert.deepEqual(result.discoveryErrors, [
    { error: 'permission denied', directory: 'locked' },
  ]);
});

test('sample report entries map back to original manifest paths and IDs', () => {
  const report = {
    totalCount: 1, fileAccountingComplete: true, discoveryErrorCount: 0, discoveryErrors: [],
    fileReadErrorCount: 0, fileReadErrors: [], inputRejectionCount: 0, inputRejections: [],
    encodingWarningCount: 0, encodingWarnings: [],
    entries: [{
      id: 'sample-000000', fileName: '/tmp/sample-000000.milk', format: 'milk',
      sourceReadStatus: 'readable-utf8', parseStatus: 'parsed',
      featureSubsetStatus: 'supported-by-current-static-subset', presetCount: 1,
      supported: true, webGpuSupported: true, presetReports: [],
    }],
  };
  const result = normalizeCompatibilityReport(report, '/tmp', new Map([
    ['sample-000000.milk', { id: 'original-id', file: 'presets/original.milk' }],
  ]));
  assert.deepEqual(
    { id: result.fileResults[0].id, file: result.fileResults[0].file },
    { id: 'original-id', file: 'presets/original.milk' },
  );
});

test('manifest merge keeps missing and unlisted files in the denominator', () => {
  const compatibility = {
    fileCount: 2,
    fileAccountingComplete: true,
    fileResults: [
      { id: 'present', file: 'presets/present.milk', sourceReadStatus: 'readable-utf8', supported: true, webGpuSupported: true },
      { id: 'extra', file: 'presets/extra.milk', sourceReadStatus: 'readable-utf8', supported: true, webGpuSupported: true },
    ],
    unsupportedCount: 0,
    webGpuUnsupportedCount: 0,
  };
  const merged = accountForManifestFiles(compatibility, [
    { id: 'manifest-present', file: 'presets/present.milk' },
    { id: 'manifest-missing', file: 'presets/missing.milk' },
  ]);
  assert.equal(merged.fileResults.length, 3);
  assert.equal(merged.fileResults[0].id, 'manifest-present');
  assert.equal(merged.fileResults[1].manifestStatus, 'missing');
  assert.equal(merged.fileResults[1].sourceReadStatus, 'manifest-file-not-discovered');
  assert.equal(merged.fileResults[2].manifestStatus, 'not-listed');
  assert.equal(merged.missingManifestFileCount, 1);
  assert.equal(merged.unlistedDiscoveredFileCount, 1);
  assert.equal(merged.unsupportedCount, 1);
  assert.equal(merged.manifestAccountingComplete, false);
});

test('compatibility report refuses a denominator that omits discovered files', () => {
  assert.throws(
    () => normalizeCompatibilityReport({
      totalCount: 2,
      entries: [],
      fileReadErrorCount: 0, fileReadErrors: [],
    }, '/workspace/packs'),
    /do not account for all 2 discovered files/,
  );
});

test('markdown lists scan, read, input and manifest issues by stable path', () => {
  const markdown = renderMarkdown({
    packCount: 1, totalPresets: 2, totalSourceFiles: 4, totalTextures: 0,
    sourceInventoryComplete: true, sourceInventoryErrorCount: 0,
    compatibilityMode: 'full', compatibilityLimit: 0,
    packs: [{
      id: 'sample', sourceFileCount: 4, corpusSha256: 'abc',
      presetCount: 2, textureCount: 0,
      compatibility: {
        supportedCount: 1, fileCount: 2, presetCount: 2, parsedPresetCount: 1,
        manifestPresetFileCount: 2, missingManifestFileCount: 1,
        fileReadErrorCount: 1, fileReadErrors: [{ file: 'broken/file.milk', error: 'permission denied' }],
        inputRejectionCount: 1, inputRejections: [{ file: 'large.milk', status: 'input-limit-rejected', error: 'too large' }],
        discoveryErrorCount: 1, discoveryErrors: [{ directory: 'locked', error: 'permission denied' }],
        encodingWarningCount: 1, encodingWarnings: [{ file: 'legacy.milk', encoding: 'windows-1252' }],
        fileResults: [{ file: 'missing.milk', manifestStatus: 'missing' }],
        unsupportedFunctions: [], unsupportedRendererFeatures: [], unsupportedShaderSections: [],
        webGpuUnsupportedRendererFeatures: [], webGpuUnsupportedShaderSections: [],
        renderedCount: 0, fidelityStatus: 'not-assessed',
      },
    }],
  });
  assert.match(markdown, /sample\/broken\/file\.milk/);
  assert.match(markdown, /permission denied/);
  assert.match(markdown, /windows-1252/);
  assert.match(markdown, /large\.milk/);
  assert.match(markdown, /locked/);
  assert.match(markdown, /manifest file was not discovered/);
});
