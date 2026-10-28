import init, { MilkRustEngine } from '/pkg/milkrust_wasm.js';

const canvas = document.querySelector('#preview');
const source = document.querySelector('#source');
const report = document.querySelector('#report');
const parameter = document.querySelector('#parameter');
const parameterValue = document.querySelector('#parameter-value');
const exportPackButton = document.querySelector('#export-pack');
const packImportInput = document.querySelector('#pack-file');

let engine;
let animationFrame = 0;
const PACK_EXPORT_VERSION = 1;
const STUDIO_EXPORT_PREFIX = 'milkrust-studio-preset';

const sanitizePackToken = (value = '') => String(value || '')
  .trim()
  .toLowerCase()
  .replace(/[^a-z0-9]+/g, '-')
  .replace(/(^-|-$)/g, '');
const safeFileName = (value = '', fallback = 'preset') => {
  const safe = sanitizePackToken(value) || sanitizePackToken(fallback);
  return `${safe || 'preset'}.json`;
};

const safePresetTitle = (value) => {
  const trimmed = String(value || '').trim();
  if (trimmed) return trimmed;
  try {
    const inspected = JSON.parse(engine?.inspectPresetText(value || defaultSource, 'studio.milk'));
    return String(inspected?.title || '').trim() || 'MilkRust Studio Preset';
  } catch {
    return 'MilkRust Studio Preset';
  }
};

const buildPackFromCurrentSource = () => {
  const presetSource = String(source.value || defaultSource);
  const fileName = 'studio-export.milk';
  const presetId = `preset-${Date.now().toString(36)}-${Math.floor(Math.random() * 10000).toString(36)}`;
  const title = safePresetTitle(presetSource);
  return {
    schemaVersion: PACK_EXPORT_VERSION,
    id: `studio-${Math.random().toString(36).slice(2, 10)}`,
    name: title,
    version: '1.0.0',
    author: 'Unspecified (local editor export)',
    description: 'Exported from MilkRust Studio',
    license: 'NOASSERTION',
    distributionStatus: 'local-only',
    contentNotice: 'Local editor export. Authors and redistribution rights have not been verified; preserve original source notices before sharing.',
    requiredMilkRustVersion: '0.1.0',
    sourceUrls: [],
    presets: [{
      id: presetId,
      title,
      file: fileName,
      sourceFormat: 'milk',
    }],
    textures: [],
    fragments: [],
    plugins: [],
    playlist: [],
    automationDefaults: {
      beatSensitivity: 1.35,
      beatsPerPreset: 8,
      minBeatIntervalSeconds: 0.25,
      transitionSeconds: 1.5,
      mode: 'off',
      timedIntervalSeconds: 30,
    },
    embeddedSources: {
      [fileName]: presetSource,
    },
  };
};

const parsePresetSourcesFromPack = (payload = {}) => {
  const presets = Array.isArray(payload?.presets) ? payload.presets : [];
  const embedded = payload?.embeddedSources || {};
  const parsed = presets
    .map((preset, index) => {
      const file = String(preset?.file || '').trim();
      const sourceText = typeof file === 'string' && file.length > 0 ? embedded?.[file] : '';
      if (!sourceText) return null;
      return {
        index,
        id: String(preset?.id || `preset-${index}`),
        title: String(preset?.title || preset?.name || `Preset ${index + 1}`),
        file,
        source: sourceText,
      };
    })
    .filter(Boolean);
  if (parsed.length) return parsed;
  if (typeof payload?.source === 'string' && payload?.source.length > 0) {
    return [{
      index: 0,
      id: 'imported',
      title: String(payload?.title || payload?.name || 'Imported Preset'),
      file: 'imported.milk',
      source: payload.source,
    }];
  }
  if (Array.isArray(payload?.presetSources)) {
    return payload.presetSources
      .map((preset, index) => {
        if (typeof preset?.source !== 'string') return null;
        return {
          index,
          id: String(preset?.id || `imported-${index}`),
          title: String(preset?.title || preset?.name || `Preset ${index + 1}`),
          file: String(preset?.file || `imported-${index}.milk`),
          source: String(preset.source),
        };
      })
      .filter(Boolean);
  }
  if (Array.isArray(payload)) {
    return payload
      .map((preset, index) => {
        if (typeof preset?.source !== 'string') return null;
        return {
          index,
          id: String(preset?.id || `imported-${index}`),
          title: String(preset?.title || preset?.name || `Preset ${index + 1}`),
          file: String(preset?.file || `imported-${index}.milk`),
          source: String(preset.source),
        };
      })
      .filter(Boolean);
  }
  return [];
};

const loadStudioPack = (text = '') => {
  const presets = parsePresetSourcesFromPack(parseImportPayload(text));
  if (!presets.length) {
    show('Unable to load pack: no embedded preset sources found');
    return;
  }
  source.value = presets[0].source;
  loadPreview();
  show({
    imported: {
      presetCount: presets.length,
      selected: presets[0].title,
      file: presets[0].file,
    },
  });
};

const defaultSource = `name=MilkRust Studio Draft
decay=0.9
wave_r=0.2
wave_g=0.7
wave_b=0.95
wave_a=0.85
wave_scale=1.25
zoom=1
rot=0
per_frame_1=rot=0.02*sin(time*0.5);
shape00_enabled=1
shape00_sides=5
shape00_rad=0.18
shape00_a=0.32
wavecode_0_enabled=1
wavecode_0_samples=96
wavecode_0_per_point1=x=i;
wavecode_0_per_point2=y=0.5+sample*0.35;
`;

const resize = () => {
  const width = Math.max(1, Math.floor(canvas.clientWidth * window.devicePixelRatio));
  const height = Math.max(1, Math.floor(canvas.clientHeight * window.devicePixelRatio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
    engine?.resize(width, height);
  }
};

const show = (value) => {
  report.textContent = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
};

const readCanvasStats = () => {
  const gl = canvas.getContext('webgl2');
  if (!gl) return null;
  const pixels = new Uint8Array(canvas.width * canvas.height * 4);
  gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
  let litPixels = 0;
  let channelTotal = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    const total = pixels[index] + pixels[index + 1] + pixels[index + 2];
    if (total > 12) litPixels += 1;
    channelTotal += total;
  }
  return { channelTotal, litPixels, pixelCount: canvas.width * canvas.height };
};

const inspect = () => {
  try {
    const inspected = JSON.parse(engine.inspectPresetText(source.value, 'studio.milk'));
    const parameters = JSON.parse(engine.getPresetParameterSummaryJson());
    const fragments = JSON.parse(engine.getPresetFragmentSummaryJson());
    const debug = JSON.parse(engine.getPresetDebugSnapshotJson('{}'));
    show({ inspected, parameters, fragments, debug });
  } catch (error) {
    show(error.message || String(error));
  }
};

const loadPreview = () => {
  try {
    const title = engine.loadPresetText(source.value, 'studio.milk', '{}');
    show(JSON.parse(engine.getPresetDebugSnapshotJson('{}')));
    return title;
  } catch (error) {
    show(error.message || String(error));
    return '';
  }
};

const parseImportPayload = (text = '') => {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
};

const exportText = (payload, fallbackName) => {
  if (!payload?.source) return;
  const objectUrl = URL.createObjectURL(new Blob([payload.source], { type: 'text/plain' }));
  const link = document.createElement('a');
  link.href = objectUrl;
  link.download = payload.fileName || fallbackName;
  link.click();
  URL.revokeObjectURL(objectUrl);
};

const startDownload = (payload, filename = STUDIO_EXPORT_PREFIX) => {
  const objectUrl = URL.createObjectURL(
    new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json;charset=utf-8' }),
  );
  const link = document.createElement('a');
  link.href = objectUrl;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(objectUrl);
};

const render = (time = 0) => {
  resize();
  const bass = Math.sin(time * 0.004) * 0.5 + 0.5;
  const mid = Math.sin(time * 0.0027 + 1.2) * 0.5 + 0.5;
  const treble = Math.sin(time * 0.006 + 2.4) * 0.5 + 0.5;
  const waveform = Array.from({ length: 128 }, (_, index) =>
    Math.sin(index * 0.21 + time * 0.006) * 0.7);
  const spectrum = Array.from({ length: 128 }, (_, index) =>
    Math.max(0, Math.sin(index * 0.08 + time * 0.004)));
  engine.render(
    time / 1000,
    bass,
    mid,
    treble,
    waveform.join(','),
    spectrum.join(','),
    0,
    0.5,
    0.5,
    0,
    0,
  );
  if (typeof engine.renderStatusJson === 'function') {
    const status = JSON.parse(engine.renderStatusJson());
    if (status.state === 'failed') report.textContent = `Renderer stopped: ${status.error}`;
  }
  if (window.__milkrustCollectStats) {
    window.__milkrustStudioStats = readCanvasStats();
  }
  animationFrame = requestAnimationFrame(render);
};

await init({ module_or_path: '/pkg/milkrust_wasm_bg.wasm' });
engine = new MilkRustEngine(canvas);
source.value = defaultSource;
resize();
loadPreview();
render();

document.querySelector('#preset-file').addEventListener('change', async (event) => {
  const [file] = event.target.files || [];
  if (!file) return;
  source.value = await file.text();
  loadPreview();
});

packImportInput?.addEventListener('change', async (event) => {
  const [file] = event.target.files || [];
  if (!file) return;
  const payload = await file.text();
  loadStudioPack(payload);
  event.target.value = '';
});

exportPackButton?.addEventListener('click', () => {
  const payload = buildPackFromCurrentSource();
  startDownload(payload, safeFileName(payload.name, STUDIO_EXPORT_PREFIX));
});

document.querySelector('#inspect').addEventListener('click', inspect);
document.querySelector('#load-preview').addEventListener('click', loadPreview);

document.querySelector('#randomize').addEventListener('click', () => {
  const result = JSON.parse(engine.randomizePresetParameters('{}'));
  if (result?.source) {
    source.value = result.source;
    loadPreview();
  }
});

document.querySelector('#apply-parameter').addEventListener('click', () => {
  const result = JSON.parse(engine.updatePresetBaseValue(
    parameter.value,
    Number(parameterValue.value),
    '{}',
  ));
  if (result?.source) {
    source.value = result.source;
    loadPreview();
  }
});

document.querySelector('#fragment-file').addEventListener('change', async (event) => {
  const [file] = event.target.files || [];
  if (!file) return;
  const result = JSON.parse(engine.loadPresetFragmentText(
    await file.text(),
    file.name,
    file.name.toLowerCase().endsWith('.wave') ? 'wave' : 'shape',
    '{}',
  ));
  if (result?.source) {
    source.value = result.source;
    loadPreview();
  }
});

document.querySelector('#export-shape').addEventListener('click', () => {
  exportText(JSON.parse(engine.exportPresetFragment('shape', 0)), 'shape.shape');
});

document.querySelector('#export-wave').addEventListener('click', () => {
  exportText(JSON.parse(engine.exportPresetFragment('wave', 0)), 'wave.wave');
});

window.addEventListener('resize', resize);
window.addEventListener('beforeunload', () => {
  cancelAnimationFrame(animationFrame);
  engine?.free?.();
});
