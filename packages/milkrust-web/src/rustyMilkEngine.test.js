import assert from 'node:assert/strict';
import { beforeEach, describe, it, mock } from 'node:test';

import {
  createMilkRustEngine,
  getMilkRustBeatUpdate,
  getMilkRustTransitionAlphas,
  getMilkRustTransitionProgress,
  loadMilkRustPack,
  loadMilkRustPackManifest,
  loadMilkRustPackPresetSource,
  loadMilkRustPackPlugins,
  normalizeMilkRustPackManifest,
  validateMilkRustPackManifest,
} from './milkrustEngine.js';

const createAnalyser = () => ({
  fftSize: 0,
  frequencyBinCount: 4,
  getByteFrequencyData: mock.fn((data) => {
    data.set([0, 128, 255, 64]);
  }),
  getByteTimeDomainData: mock.fn((data) => {
    data.set([0, 128, 255, 128]);
  }),
});

const createRustEngineMock = () => ({
  exportPresetFragment: mock.fn((type) =>
    JSON.stringify({ fileName: `active.${type}`, source: `[${type}]\nenabled=1\n` })),
  exportPresetText: mock.fn(() =>
    JSON.stringify({ fileName: 'active.milk', source: 'name=Active\nzoom=1\n' })),
  free: mock.fn(),
  getPresetDebugSnapshotJson: mock.fn(() =>
    JSON.stringify({ renderer: 'Rust WebGL2 renderer active', title: 'Active' })),
  getPresetFragmentSummaryJson: mock.fn(() =>
    JSON.stringify({ shapes: [{ index: 0, label: 'Shape 1' }], waves: [] })),
  getPresetParameterSummaryJson: mock.fn(() => JSON.stringify({ decay: 0.91, zoom: 1 })),
  inspectPresetText: mock.fn((_source, fileName) =>
    JSON.stringify({ title: fileName.replace(/\.milk2?$/, '') || 'Imported preset' })),
  loadPresetFragmentText: mock.fn((_source, fileName) =>
    JSON.stringify({
      source: `name=Active\n; merged ${fileName}`,
      title: `Active + ${fileName}`,
    })),
  loadPresetText: mock.fn((_source, fileName) =>
    fileName?.replace(/\.milk2?$/, '') || 'Imported preset'),
  randomizePresetParameters: mock.fn(() =>
    JSON.stringify({ source: 'name=Random', title: 'Random', values: { zoom: 1.2 } })),
  removePresetFragment: mock.fn((type) =>
    JSON.stringify({ source: `name=Active\n; removed ${type}`, title: 'Active edited' })),
  render: mock.fn(),
  resize: mock.fn(),
  updatePresetBaseValue: mock.fn((key, value) =>
    JSON.stringify({
      source: `name=Active\n${key}=${value}`,
      title: 'Active edited',
      values: { [key]: value },
    })),
});

describe('createMilkRustEngine', () => {
  let rustEngine;

  beforeEach(() => {
    rustEngine = createRustEngineMock();
    class MilkRustEngineMock {
      constructor() {
        return rustEngine;
      }
    }
    globalThis.__milkrustModule = {
      MilkRustEngine: MilkRustEngineMock,
    };
  });

  it('eases transition progress between renderer sets', () => {
    assert.equal(getMilkRustTransitionProgress(10, 2, 10), 0);
    assert.equal(getMilkRustTransitionProgress(10, 2, 11), 0.5);
    assert.equal(getMilkRustTransitionProgress(10, 2, 12), 1);
    assert.equal(getMilkRustTransitionProgress(10, 0, 10), 1);
  });

  it('maps transition modes to incoming and outgoing alphas', () => {
    assert.deepEqual(getMilkRustTransitionAlphas(0.25, 'crossfade'), {
      incoming: 0.25,
      outgoing: 0.75,
    });
    assert.deepEqual(getMilkRustTransitionAlphas(0.25, 'fade_through_black'), {
      incoming: 0,
      outgoing: 0.5,
    });
    assert.deepEqual(getMilkRustTransitionAlphas(0.5, 'overlay'), {
      incoming: 0.5,
      outgoing: 1,
    });
    assert.deepEqual(getMilkRustTransitionAlphas(0.5, 'cut'), {
      incoming: 1,
      outgoing: 0,
    });
  });

  it('loads pack data plugins through fetch', async () => {
    const calls = [];
    const responses = new Map([
      ['http://localhost/packs/demo/manifest.json', {
        ok: true,
        json: async () => ({
          id: 'demo',
          name: 'Demo',
          version: '0.1.0',
          plugins: [{ id: 'playlist', kind: 'data', entry: 'plugins/playlist.json' }],
          presets: [],
        }),
      }],
      ['http://localhost/packs/demo/plugins/playlist.json', {
        ok: true,
        json: async () => ({
          kind: 'playlist',
          presetIds: ['first', 'second'],
          shuffle: false,
        }),
      }],
    ]);
    const fetchImpl = async (url) => {
      calls.push(url);
      return responses.get(url);
    };

    const manifest = await loadMilkRustPackManifest('http://localhost/packs/demo/', { fetchImpl });
    const pluginState = await loadMilkRustPackPlugins(manifest, { fetchImpl });
    assert.equal(pluginState.valid, true);
    assert.equal(pluginState.plugins.length, 1);
    assert.equal(pluginState.plugins[0].kind, 'data');
    assert.equal(pluginState.plugins[0].payload.kind, 'playlist');
    assert.deepEqual(calls, [
      'http://localhost/packs/demo/manifest.json',
      'http://localhost/packs/demo/plugins/playlist.json',
    ]);
  });

  it('rejects executable pack plugins without fetching or evaluating their module', async () => {
    globalThis.__milkrustTestJsPluginCalls = 0;
    const jsPluginUrl = `data:text/javascript,${encodeURIComponent('globalThis.__milkrustTestJsPluginCalls += 1; export default {};')}`;
    const manifest = validateMilkRustPackManifest({
      id: 'demo', name: 'Demo', version: '0.1.0', presets: [],
      plugins: [{ id: 'js-hook', kind: 'javascript', entry: jsPluginUrl }],
    });
    const fetchImpl = mock.fn(async () => { throw new Error('Executable asset must not be fetched'); });
    const result = await loadMilkRustPackPlugins(manifest, { fetchImpl });
    assert.equal(result.valid, false);
    assert.equal(result.plugins.length, 0);
    assert.match(result.errors[0], /executable pack plugins are disabled/);
    assert.equal(fetchImpl.mock.calls.length, 0);
    assert.equal(globalThis.__milkrustTestJsPluginCalls, 0);
    delete globalThis.__milkrustTestJsPluginCalls;
  });

  it('propagates a blocked executable entry into the aggregate pack result', async () => {
    const requests = [];
    const result = await loadMilkRustPack('http://localhost/packs/demo/', {
      fetchImpl: async (url) => {
        requests.push(url);
        assert.match(url, /manifest\.json$/);
        return { ok: true, json: async () => ({ id: 'demo', name: 'Demo', version: '0.1.0',
          presets: [], plugins: [{ id: 'blocked', kind: 'js', entry: 'plugins/run.js' }] }) };
      },
    });
    assert.equal(result.valid, false);
    assert.match(result.errors[0], /blocked/);
    assert.equal(requests.length, 1);
  });

  it('marks failed plugin loads without crashing the pack load', async () => {
    const badJsPluginUrl = `data:text/javascript,${encodeURIComponent(`
      export const onFrameStart = () => {
      // missing closing brace
    `)}`;
    const manifest = validateMilkRustPackManifest({
      id: 'demo',
      name: 'Demo',
      version: '0.1.0',
      plugins: [{ id: 'bad-js', kind: 'javascript', entry: badJsPluginUrl }],
      presets: [],
    });

    const pluginState = await loadMilkRustPackPlugins(manifest, {
      fetchImpl: async () => ({ ok: false }),
    });

    assert.equal(pluginState.valid, false);
    assert.equal(pluginState.plugins.length, 0);
    assert.equal(pluginState.errors.length, 1);
    assert.match(pluginState.errors[0], /bad-js/);
  });

  it('detects beat pulses from low-frequency spectrum energy', () => {
    const baseline = getMilkRustBeatUpdate(
      {},
      [16, 18, 17, 19],
      1,
      { beatSensitivity: 1.35, minBeatIntervalSeconds: 0.25 },
    );
    const pulse = getMilkRustBeatUpdate(
      baseline,
      [240, 230, 220, 210],
      1.4,
      { beatSensitivity: 1.35, minBeatIntervalSeconds: 0.25 },
    );

    assert.equal(baseline.isBeat, false);
    assert.equal(pulse.isBeat, true);
    assert.equal(pulse.beatCount, 1);
  });

  it('normalizes and validates pack manifests', () => {
    const normalized = normalizeMilkRustPackManifest({
      schemaVersion: 1,
      id: 'web-pack',
      name: 'Web Pack',
      version: '0.1.0',
      presets: [
        {
          id: 'one',
          title: 'One',
          file: 'presets/one.milk',
          tags: ['fixture'],
        },
      ],
    }, 'http://127.0.0.1:4173/examples/sample-pack/manifest.json');
    const validation = validateMilkRustPackManifest(normalized);
    const relative = normalizeMilkRustPackManifest({
      id: 'relative-pack',
      name: 'Relative Pack',
      version: '0.1.0',
      presets: [{ id: 'dots', file: 'presets/v1..ok.milk' }],
    }, 'packs/relative/manifest.json');

    assert.equal(normalized.presets[0].url, 'http://127.0.0.1:4173/examples/sample-pack/presets/one.milk');
    assert.deepEqual(normalized.presets[0].tags, ['fixture']);
    assert.equal(validation.valid, true);
    assert.equal(relative.presets[0].url, 'http://localhost/packs/relative/presets/v1..ok.milk');
    assert.equal(validateMilkRustPackManifest(relative).valid, true);
    assert.equal(validateMilkRustPackManifest({
      id: 'bad',
      name: 'Bad',
      version: '0.1.0',
      presets: [{ id: 'escape', file: '../escape.milk' }],
    }).valid, false);
  });

  it('loads pack manifests and preset sources through fetch', async () => {
    const calls = [];
    const responses = new Map([
      ['http://localhost/packs/demo/manifest.json', {
        ok: true,
        json: async () => ({
          id: 'demo',
          name: 'Demo',
          version: '0.1.0',
          presets: [{ id: 'first', title: 'First', file: 'presets/first.milk' }],
        }),
      }],
      ['http://localhost/packs/demo/presets/first.milk', {
        ok: true,
        text: async () => 'name=First\nzoom=1\n',
      }],
    ]);
    const fetchImpl = async (url) => {
      calls.push(url);
      return responses.get(url);
    };
    const manifest = await loadMilkRustPackManifest('http://localhost/packs/demo/', {
      fetchImpl,
    });
    const source = await loadMilkRustPackPresetSource(manifest.manifest.presets[0], {
      fetchImpl,
    });
    const pack = await loadMilkRustPack('http://localhost/packs/demo/', { fetchImpl });

    assert.equal(manifest.valid, true);
    assert.equal(manifest.manifest.presets[0].name, undefined);
    assert.match(source, /zoom=1/);
    assert.equal(pack.valid, true);
    assert.equal(pack.presets[0].name, 'First');
    assert.match(pack.presets[0].source, /zoom=1/);
    assert.deepEqual(calls.slice(0, 2), [
      'http://localhost/packs/demo/manifest.json',
      'http://localhost/packs/demo/presets/first.milk',
    ]);
  });

  it('feeds waveform, spectrum, and mouse state into the Rust renderer', async () => {
    const analyser = createAnalyser();
    const audioNode = {
      connect: mock.fn(),
      disconnect: mock.fn(),
    };
    const engine = await createMilkRustEngine({
      audioContext: {
        createAnalyser: () => analyser,
        currentTime: 12,
      },
      audioNode,
      canvas: {},
    });

    engine.setMouseState({
      mouse_down: 1,
      mouse_dx: 0.2,
      mouse_dy: -0.1,
      mouse_x: 0.75,
      mouse_y: 0.25,
    });
    engine.render();
    engine.resize(320, 180);

    assert.equal(engine.name, 'MilkRust WebGL2');
    assert.deepEqual(audioNode.connect.mock.calls[0].arguments, [analyser]);
    const renderArgs = rustEngine.render.mock.calls[0].arguments;
    assert.equal(renderArgs[0], 12);
    assert.equal(renderArgs[7], 0.75);
    assert.equal(renderArgs[8], 0.25);
    assert.equal(renderArgs[9], 0.2);
    assert.equal(renderArgs[10], -0.1);
    assert.match(renderArgs[4], /-1/);
    assert.deepEqual(rustEngine.resize.mock.calls[0].arguments, [320, 180]);
  });

  it('exposes renderer failures and clears the texture-loading state after upload', async () => {
    let status = { state: 'failed', error: 'Composite shader failed' };
    rustEngine.renderStatusJson = () => JSON.stringify(status);
    const engine = await createMilkRustEngine({
      audioContext: { createAnalyser, currentTime: 1 },
      audioNode: { connect: mock.fn(), disconnect: mock.fn() }, canvas: {},
    });
    const onRenderFrame = mock.fn();
    engine.loadPlugins([{ id: 'application-hook', api: { onRenderFrame } }]);
    assert.deepEqual(engine.render(), { error: 'Composite shader failed', fidelity: 'not-assessed' });
    assert.equal(onRenderFrame.mock.calls.length, 0);
    status = { state: 'waiting-textures' };
    assert.equal(engine.render().waitingForTextures, true);
    status = { state: 'preview' };
    assert.equal(engine.render().waitingForTextures, false);
    assert.equal(engine.render(), null);
    engine.dispose();
  });

  it('fires plugin hooks for engine lifecycle events', async () => {
    const analyser = createAnalyser();
    const audioNode = {
      connect: mock.fn(),
      disconnect: mock.fn(),
    };
    const plugin = {
      id: 'test-plugin',
      kind: 'data',
      onPresetLoad: mock.fn(),
      onPresetChange: mock.fn(),
      onFrameStart: mock.fn(),
      onRenderFrame: mock.fn(),
      onExport: mock.fn(),
    };
    const engine = await createMilkRustEngine({
      audioContext: {
        createAnalyser: () => analyser,
        currentTime: 4,
      },
      audioNode,
      canvas: {},
    });

    engine.loadPlugins([plugin]);
    engine.loadPresetText('name=Imported', 'imported.milk');
    engine.render();
    engine.exportPresetText();

    assert.equal(plugin.onPresetLoad.mock.calls.length, 1);
    assert.equal(plugin.onPresetChange.mock.calls.length, 1);
    assert.equal(plugin.onFrameStart.mock.calls.length, 1);
    assert.equal(plugin.onRenderFrame.mock.calls.length, 1);
    assert.equal(plugin.onExport.mock.calls.length, 1);
  });

  it('loads, edits, exports, and disposes through the Rust WASM boundary', async () => {
    const analyser = createAnalyser();
    const audioNode = {
      connect: mock.fn(),
      disconnect: mock.fn(),
    };
    const engine = await createMilkRustEngine({
      audioContext: {
        createAnalyser: () => analyser,
        currentTime: 0,
      },
      audioNode,
      canvas: {},
      rendererBackend: 'webgpu',
    });

    assert.equal(engine.name, 'MilkRust WebGL2 fallback');
    assert.equal(engine.loadPresetPack({
      presets: [
        {
          id: 'packed',
          title: 'Packed Preset',
          file: 'packed.milk',
          source: 'name=Packed Preset\nzoom=1\n',
        },
      ],
    }), 'Packed Preset');
    assert.equal(engine.loadPresetText('name=Imported', 'imported.milk'), 'imported');
    assert.deepEqual(engine.inspectPresetText('name=Imported', 'imported.milk'), {
      title: 'imported',
    });
    assert.deepEqual(engine.loadPresetFragmentText('enabled=1', 'shape.shape'), {
      source: 'name=Active\n; merged shape.shape',
      title: 'Active + shape.shape',
    });
    assert.deepEqual(engine.updatePresetBaseValue('zoom', 1.2), {
      source: 'name=Active\nzoom=1.2',
      title: 'Active edited',
      values: { zoom: 1.2 },
    });
    assert.deepEqual(engine.randomizePresetParameters(), {
      source: 'name=Random',
      title: 'Random',
      values: { zoom: 1.2 },
    });
    assert.deepEqual(engine.removePresetFragment('shape', 0), {
      source: 'name=Active\n; removed shape',
      title: 'Active edited',
    });
    assert.deepEqual(engine.exportPresetText(), {
      fileName: 'active.milk',
      source: 'name=Active\nzoom=1\n',
    });
    assert.deepEqual(engine.exportPresetFragment('shape', 0), {
      fileName: 'active.shape',
      source: '[shape]\nenabled=1\n',
    });
    assert.deepEqual(engine.getPresetParameterSummary(), { decay: 0.91, zoom: 1 });
    assert.deepEqual(engine.getPresetFragmentSummary(), {
      shapes: [{ index: 0, label: 'Shape 1' }],
      waves: [],
    });
    assert.deepEqual(engine.getPresetDebugSnapshot(), {
      renderer: 'Rust WebGL2 renderer active',
      title: 'Active',
    });

    engine.dispose();

    assert.deepEqual(audioNode.disconnect.mock.calls[0].arguments, [analyser]);
    assert.equal(rustEngine.free.mock.callCount(), 1);
  });
});
