import { readMilkRustPresetResponse } from './presetText.js';
const milkrustPresets = [
  {
    name: 'MilkRust grid smoke',
    source: `
      name=MilkRust grid smoke
      decay=0.91
      wave_r=0.12
      wave_g=0.64
      wave_b=0.88
      wave_a=0.86
      wave_scale=1.2
      zoom=1
      rot=0
      per_frame_1=wave_r=0.35+0.25*bass_att;
      per_frame_2=wave_g=0.45+0.2*mid_att;
      per_frame_3=wave_b=0.55+0.2*treb_att;
      per_frame_4=rot=0.01*sin(time*0.7);
      per_frame_5=zoom=1+0.03*sin(time*0.5);
      per_frame_6=dx=0.015*sin(time*0.6);
      per_frame_7=dy=0.015*cos(time*0.5);
      shape00_enabled=1
      shape00_sides=5
      shape00_rad=0.18
      shape00_x=0.5
      shape00_y=0.5
      shape00_r=0.1
      shape00_g=0.9
      shape00_b=0.45
      shape00_a=0.35
      shape00_r2=0.9
      shape00_g2=0.8
      shape00_b2=0.2
      shape00_a2=0.18
      shape00_border_a=0.9
      shape00_per_frame1=ang=time*0.5;
      wavecode_0_enabled=1
      wavecode_0_samples=96
      wavecode_0_spectrum=1
      wavecode_0_dots=1
      wavecode_0_r=0.7
      wavecode_0_g=0.95
      wavecode_0_b=0.25
      wavecode_0_a=0.75
      wavecode_0_per_point1=x=i;
      wavecode_0_per_point2=y=0.08+sample*0.55;
    `,
  },
  {
    name: 'MilkRust waveform smoke',
    source: `
      name=MilkRust waveform smoke
      decay=0.88
      wave_r=0.85
      wave_g=0.34
      wave_b=0.18
      wave_scale=1.5
      per_frame_1=dx=0.02*sin(time*0.4);
      per_frame_2=dy=0.015*cos(time*0.3);
      per_frame_3=rot=0.02*sin(time*0.2);
      shape00_enabled=1
      shape00_sides=3
      shape00_rad=0.12+0.03*bass_att
      shape00_x=0.35
      shape00_y=0.55
      shape00_r=0.9
      shape00_g=0.2
      shape00_b=0.1
      shape00_a=0.28
      shape00_additive=1
      shape01_enabled=1
      shape01_sides=6
      shape01_rad=0.08+0.02*treb_att
      shape01_x=0.67
      shape01_y=0.45
      shape01_r=0.1
      shape01_g=0.55
      shape01_b=0.95
      shape01_a=0.35
      wavecode_0_enabled=1
      wavecode_0_samples=128
      wavecode_0_r=0.95
      wavecode_0_g=0.85
      wavecode_0_b=0.2
      wavecode_0_a=0.8
      wavecode_0_per_point1=x=i;
      wavecode_0_per_point2=y=0.5+sample*0.35;
    `,
  },
];

const defaultTransitionSeconds = 1.5;
const defaultAutomation = {
  beatSensitivity: 1.35,
  beatsPerPreset: 8,
  minBeatIntervalSeconds: 0.25,
  mode: 'off',
  timedIntervalSeconds: 30,
};

const normalizeStringArray = (values = []) => Array.isArray(values)
  ? values.filter((value) => typeof value === 'string')
  : [];

const normalizePackPath = (value = '') => String(value || '').replace(/^\/+/, '');

const normalizePluginKind = (value = '') => {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'javascript') return 'js';
  if (normalized === 'module') return 'js';
  return normalized || 'data';
};

const absoluteMilkRustUrl = (url) =>
  new URL(url, globalThis.location?.href || 'http://localhost/').toString();

const isUnsafePackPath = (value = '') =>
  normalizePackPath(value).split('/').some((segment) => segment === '..');

const resolvePackAssetUrl = (baseUrl, file) => {
  const normalizedFile = normalizePackPath(file);
  if (!normalizedFile) return '';
  if (/^[a-z][a-z0-9+.-]*:/i.test(normalizedFile)) return normalizedFile;
  if (!baseUrl) return normalizedFile;
  return new URL(normalizedFile, baseUrl).toString();
};

export const normalizeMilkRustPackManifest = (manifest = {}, manifestUrl = '') => {
  const baseUrl = manifestUrl
    ? new URL('.', absoluteMilkRustUrl(manifestUrl)).toString()
    : '';
  const presets = Array.isArray(manifest.presets) ? manifest.presets : [];
  const textures = Array.isArray(manifest.textures) ? manifest.textures : [];
  const fragments = Array.isArray(manifest.fragments) ? manifest.fragments : [];
  const plugins = Array.isArray(manifest.plugins) ? manifest.plugins : [];
  return {
    schemaVersion: Number(manifest.schemaVersion ?? manifest.schema_version ?? 1) || 1,
    id: String(manifest.id || ''),
    name: String(manifest.name || ''),
    version: String(manifest.version || ''),
    author: String(manifest.author || ''),
    description: String(manifest.description || ''),
    license: String(manifest.license || ''),
    requiredMilkRustVersion: String(
      manifest.requiredMilkRustVersion || manifest.required_milkrust_version || '',
    ),
    sourceUrls: normalizeStringArray(manifest.sourceUrls || manifest.source_urls),
    presets: presets.map((preset) => ({
      id: String(preset?.id || ''),
      title: String(preset?.title || ''),
      file: normalizePackPath(preset?.file),
      sourceFormat: String(preset?.sourceFormat || preset?.source_format || preset?.format || ''),
      url: resolvePackAssetUrl(baseUrl, preset?.file),
      tags: normalizeStringArray(preset?.tags),
      thumbnail: normalizePackPath(preset?.thumbnail),
      thumbnailUrl: resolvePackAssetUrl(baseUrl, preset?.thumbnail),
    })),
    textures: textures.map((texture) => ({
      id: String(texture?.id || ''),
      file: normalizePackPath(texture?.file),
      url: resolvePackAssetUrl(baseUrl, texture?.file),
      aliases: normalizeStringArray(texture?.aliases),
    })),
    fragments: fragments.map((fragment) => ({
      id: String(fragment?.id || ''),
      kind: String(fragment?.kind || 'preset'),
      file: normalizePackPath(fragment?.file),
      url: resolvePackAssetUrl(baseUrl, fragment?.file),
      tags: normalizeStringArray(fragment?.tags),
    })),
    plugins: plugins.map((plugin) => ({
      id: String(plugin?.id || ''),
      kind: normalizePluginKind(plugin?.kind),
      entry: normalizePackPath(plugin?.entry),
      url: resolvePackAssetUrl(baseUrl, plugin?.entry),
      source: String(plugin?.entry || ''),
    })),
    playlist: normalizeStringArray(manifest.playlist),
    automationDefaults: manifest.automationDefaults || manifest.automation_defaults || {},
  };
};

export const validateMilkRustPackManifest = (manifest = {}, manifestUrl = '') => {
  const normalized = normalizeMilkRustPackManifest(manifest, manifestUrl);
  const errors = [];
  const warnings = [];
  if (normalized.schemaVersion !== 1) {
    errors.push(`unsupported schemaVersion ${normalized.schemaVersion}`);
  }
  for (const field of ['id', 'name', 'version']) {
    if (!normalized[field]) errors.push(`manifest field ${field} is required`);
  }
  if (!normalized.presets.length) warnings.push('pack contains no presets');
  for (const preset of normalized.presets) {
    if (!preset.id) errors.push('preset id is required');
    if (!preset.file) errors.push(`preset ${preset.id || '<missing>'} file is required`);
    if (isUnsafePackPath(preset.file)) errors.push(`preset ${preset.id || preset.file} path must stay inside the pack`);
  }
  for (const plugin of normalized.plugins) {
    if (!plugin.id) errors.push('plugin id is required');
    if (!plugin.entry) errors.push(`plugin ${plugin.id || '<missing>'} entry is required`);
    if (isUnsafePackPath(plugin.entry)) errors.push(`plugin ${plugin.id || plugin.entry || '<missing>'} path must stay inside the pack`);
    if (!['data', 'js'].includes(plugin.kind)) {
      warnings.push(`plugin ${plugin.id || '<missing>'} uses unsupported kind ${plugin.kind}; treated as data plugin`);
      plugin.kind = 'data';
    }
  }
  return {
    errors,
    manifest: normalized,
    valid: errors.length === 0,
    warnings,
  };
};

const packManifestUrl = (packUrl) => {
  const value = String(packUrl || '');
  if (value.endsWith('.json')) return absoluteMilkRustUrl(value);
  return new URL('manifest.json', absoluteMilkRustUrl(value.endsWith('/') ? value : `${value}/`)).toString();
};

export const loadMilkRustPack = async (packUrl, { fetchImpl = globalThis.fetch } = {}) => {
  const validation = await loadMilkRustPackManifest(packUrl, { fetchImpl });
  const presets = await Promise.all(validation.manifest.presets.map(async (preset) => ({
    ...preset,
    name: preset.title || preset.id || preset.file,
    source: await loadMilkRustPackPresetSource(preset, { fetchImpl }),
  })));
  const plugins = await loadMilkRustPackPlugins(validation, { fetchImpl });
  return {
    ...validation,
    valid: validation.valid && plugins.valid,
    errors: [...(validation.errors || []), ...plugins.errors],
    warnings: [...(validation.warnings || []), ...plugins.warnings],
    plugins,
    manifest: validation.manifest,
    presets,
  };
};

export const loadMilkRustPackManifest = async (packUrl, { fetchImpl = globalThis.fetch } = {}) => {
  if (typeof fetchImpl !== 'function') {
    throw new Error('loadMilkRustPackManifest requires a fetch implementation');
  }
  const manifestUrl = packManifestUrl(packUrl);
  const manifestResponse = await fetchImpl(manifestUrl);
  if (!manifestResponse.ok) {
    throw new Error(`failed to load pack manifest ${manifestUrl}`);
  }
  const validation = validateMilkRustPackManifest(await manifestResponse.json(), manifestUrl);
  if (!validation.valid) {
    throw new Error(`invalid MilkRust pack: ${validation.errors.join('; ')}`);
  }
  validation.manifestUrl = manifestUrl;
  return validation;
};

export const loadMilkRustPackPresetSource = async (
  preset,
  { fetchImpl = globalThis.fetch } = {},
) => {
  if (typeof fetchImpl !== 'function') {
    throw new Error('loadMilkRustPackPresetSource requires a fetch implementation');
  }
  const response = await fetchImpl(preset?.url || preset?.file || '');
  if (!response.ok) {
    throw new Error(`failed to load preset ${preset?.file || preset?.id || '<unknown>'}`);
  }
  return readMilkRustPresetResponse(response);
};

export const loadMilkRustPackPlugins = async (
  manifest,
  { fetchImpl = globalThis.fetch } = {},
) => {
  if (typeof fetchImpl !== 'function') {
    throw new Error('loadMilkRustPackPlugins requires a fetch implementation');
  }
  const manifestUrl = manifest?.manifestUrl || manifest?.manifest?.manifestUrl || manifest?.manifest_url;
  const normalized = normalizeMilkRustPackManifest(manifest?.manifest || manifest, manifestUrl);
  const warnings = [];
  const errors = [];
  const plugins = await Promise.all(normalized.plugins.map(async (plugin) => {
    try {
      if (plugin.kind === 'js') {
        errors.push(`plugin ${plugin.id || '<missing>'}: executable pack plugins are disabled; only data entries may be loaded from a pack`);
        return null;
      }

      if (!plugin.url) {
        warnings.push(`plugin ${plugin.id || '<missing>'} has no url`);
        return null;
      }
      const response = await fetchImpl(plugin.url);
      if (!response.ok) {
        errors.push(`failed to load plugin ${plugin.id || plugin.entry || '<missing>'}`);
        return null;
      }
      const payload = await response.json();
      return {
        ...plugin,
        source: 'data',
        payload,
      };
    } catch (error) {
      errors.push(`${plugin.id || plugin.entry || 'plugin'} load error: ${error?.message || String(error)}`);
      return null;
    }
  }));

  const loadedPlugins = plugins.filter(Boolean);
  return {
    valid: errors.length === 0,
    errors,
    warnings,
    plugins: loadedPlugins,
    manifest: normalized,
  };
};

let rustModulePromise;
let rustModuleInitPromise;
const hlslInitializations = new WeakMap();

const loadMilkRustModule = async (modulePath = globalThis.__milkrustModulePath || '/milkrust_wasm.js') => {
  let rustModule = globalThis.__milkrustModule;
  if (!rustModule) {
    if (!rustModulePromise) {
      rustModulePromise = import(/* @vite-ignore */ modulePath).catch((error) => {
        rustModulePromise = undefined;
        throw error;
      });
    }
    rustModule = await rustModulePromise;
    if (typeof rustModule.default === 'function' && !rustModuleInitPromise) {
      const wasmPath = modulePath.replace(/\.js(?:\?.*)?$/, '_bg.wasm');
      rustModuleInitPromise = Promise.resolve(rustModule.default({ module_or_path: wasmPath })).catch((error) => {
        rustModuleInitPromise = undefined;
        throw error;
      });
    }
    await rustModuleInitPromise;
  }
  if (typeof rustModule.setTextureDecoderWorkerUrl === 'function') {
    const decoderUrl = new URL('milkrust_texture_decoder.js', absoluteMilkRustUrl(modulePath)).href;
    if (globalThis.location?.origin && new URL(decoderUrl).origin === globalThis.location.origin) {
      rustModule.setTextureDecoderWorkerUrl(decoderUrl);
    }
  }
  if (typeof rustModule.initializeHlslCompiler === 'function' && !hlslInitializations.has(rustModule)) {
    const compilerUrl = new URL('milkrust_hlsl.js', absoluteMilkRustUrl(modulePath)).href;
    const ready = Promise.resolve().then(() => rustModule.initializeHlslCompiler(compilerUrl)).catch((error) => {
      hlslInitializations.delete(rustModule);
      throw error;
    });
    hlslInitializations.set(rustModule, ready);
  }
  await hlslInitializations.get(rustModule);
  return rustModule;
};

const createFrameReader = (audioContext, audioNode, enableStereo = false) => {
  const analyser = audioContext.createAnalyser();
  analyser.fftSize = 2048;
  audioNode.connect(analyser);

  const waveform = new Uint8Array(analyser.fftSize);
  const frequency = new Uint8Array(analyser.frequencyBinCount);
  const stereoConnections = [];
  let stereoAnalysers = null;
  let channelData = null;
  if (enableStereo && typeof audioContext.createChannelSplitter === 'function' &&
      typeof audioContext.createGain === 'function') {
    try {
      const channels = [audioContext.createAnalyser(), audioContext.createAnalyser()];
      if (channels.every((channel) => typeof channel.getFloatTimeDomainData === 'function')) {
        const input = audioContext.createGain();
        // Upmix mono and downmix multichannel to the native stereo contract.
        // The splitter itself stays discrete; the gain node performs the mix.
        input.channelCount = 2;
        input.channelCountMode = 'explicit';
        input.channelInterpretation = 'speakers';
        const splitter = audioContext.createChannelSplitter(2);
        channels.forEach((channel) => { channel.fftSize = 4096; });
        const connect = (from, to, output = 0) => {
          from.connect(to, output, 0);
          stereoConnections.push([from, to]);
        };
        connect(audioNode, input);
        connect(input, splitter);
        channels.forEach((channel, index) => connect(splitter, channel, index));
        stereoAnalysers = channels;
        channelData = channels.map((channel) => new Float32Array(channel.fftSize));
      }
    } catch (error) {
      stereoConnections.forEach(([from, to]) => { try { from.disconnect(to); } catch {} });
      try { audioNode.disconnect(analyser); } catch {}
      throw error;
    }
  }

  return {
    disconnect: () => {
      stereoConnections.forEach(([from, to]) => { try { from.disconnect(to); } catch {} });
      try {
        audioNode.disconnect(analyser);
      } catch {
        // The shared audio graph may have been rebuilt or torn down first.
      }
    },
    read: () => {
      analyser.getByteTimeDomainData(waveform);
      analyser.getByteFrequencyData(frequency);
      const spectrum = Array.from(frequency);
      let pcmStereo;
      if (stereoAnalysers) {
        pcmStereo = new Float32Array(1024);
        const ratio = (Number(audioContext.sampleRate) || 44100) / 44100;
        stereoAnalysers.forEach((channel, index) => {
          const data = channelData[index];
          channel.getFloatTimeDomainData(data);
          for (let frame = 0; frame < 512; frame += 1) {
            const position = Math.max(0, data.length - 1 - (511 - frame) * ratio);
            const lower = Math.floor(position);
            const upper = Math.min(lower + 1, data.length - 1);
            pcmStereo[frame * 2 + index] = data[lower] + (data[upper] - data[lower]) * (position - lower);
          }
        });
      }
      return {
        pcmStereo,
        bands: getAudioBands(spectrum),
        samples: Array.from(waveform, (value) => (value - 128) / 128),
        spectrum,
      };
    },
  };
};

const getAudioBands = (spectrum = []) => {
  if (!spectrum.length) return { bass: 0, mid: 0, treble: 0 };
  const normalized = (start, end) => {
    const safeEnd = Math.max(start + 1, Math.min(end, spectrum.length));
    let total = 0;
    for (let index = start; index < safeEnd; index += 1) {
      total += spectrum[index] / 255;
    }
    return total / (safeEnd - start);
  };
  return {
    bass: normalized(0, Math.max(1, Math.floor(spectrum.length / 8))),
    mid: normalized(Math.floor(spectrum.length / 8), Math.floor(spectrum.length / 3)),
    treble: normalized(Math.floor(spectrum.length / 3), spectrum.length),
  };
};

const getSpectrumEnergy = (spectrum = []) => {
  if (!spectrum.length) return 0;
  const limit = Math.max(1, Math.min(24, spectrum.length));
  let total = 0;
  for (let index = 0; index < limit; index += 1) {
    total += Number(spectrum[index]) || 0;
  }
  return total / (limit * 255);
};

export const getMilkRustBeatUpdate = (
  previous = {},
  spectrum = [],
  now = 0,
  automation = defaultAutomation,
) => {
  const energy = getSpectrumEnergy(spectrum);
  const smoothedEnergy = previous.smoothedEnergy === undefined
    ? energy
    : (previous.smoothedEnergy * 0.85) + (energy * 0.15);
  const secondsSinceBeat = now - (previous.lastBeatAt ?? -Infinity);
  const isBeat = energy > Math.max(0.05, smoothedEnergy * automation.beatSensitivity)
    && secondsSinceBeat >= automation.minBeatIntervalSeconds;
  const beatCount = isBeat ? (previous.beatCount || 0) + 1 : (previous.beatCount || 0);
  return {
    beatCount,
    energy,
    isBeat,
    lastBeatAt: isBeat ? now : previous.lastBeatAt,
    smoothedEnergy,
  };
};

export const getMilkRustTransitionProgress = (startedAt, seconds, now) => {
  if (!Number.isFinite(seconds) || seconds <= 0) return 1;
  const linear = Math.max(0, Math.min(1, (now - startedAt) / seconds));
  return linear * linear * (3 - linear * 2);
};

export const getMilkRustTransitionAlphas = (progress, mode = 'crossfade') => {
  const clampedProgress = Math.max(0, Math.min(1, Number(progress) || 0));
  const normalizedMode = String(mode || '').trim().toLowerCase().replace(/[\s_-]+/g, '');
  if (['fade', 'fadeblack', 'fadethroughblack'].includes(normalizedMode)) {
    return {
      incoming: clampedProgress <= 0.5 ? 0 : (clampedProgress - 0.5) * 2,
      outgoing: clampedProgress >= 0.5 ? 0 : 1 - (clampedProgress * 2),
    };
  }
  if (['overlay', 'burnin', 'hold'].includes(normalizedMode)) {
    return {
      incoming: clampedProgress,
      outgoing: 1,
    };
  }
  if (['cut', 'instant', 'none'].includes(normalizedMode)) {
    return {
      incoming: 1,
      outgoing: 0,
    };
  }
  return {
    incoming: clampedProgress,
    outgoing: 1 - clampedProgress,
  };
};

const normalizeAutomation = (automation = {}) => ({
  ...defaultAutomation,
  ...automation,
  mode: ['beat', 'timed'].includes(automation.mode) ? automation.mode : 'off',
});

const parseJson = (value, fallback = null) => {
  if (value === null || value === undefined || value === 'null') return fallback;
  if (typeof value !== 'string') return value;
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
};

const toCsv = (values = []) => values.join(',');

const textureAssetsJson = (textureAssets = {}) => JSON.stringify(textureAssets || {});

const getWebGpuStatus = (rendererBackend) => ({
  available: false,
  backend: rendererBackend === 'webgpu' ? 'rust-webgl2-fallback' : 'rust-webgl2',
  reason: rendererBackend === 'webgpu'
    ? 'MilkRust currently renders through the Rust WebGL2/canvas backend.'
    : '',
});

const pluginHookNames = [
  'onPresetLoad',
  'onPresetLoaded',
  'onPresetChange',
  'onFrameStart',
  'onAudioFrame',
  'onBeat',
  'onAutomationStep',
  'onRenderFrame',
  'onInput',
  'onExport',
];

const createPluginHooks = () => Object.fromEntries(
  pluginHookNames.map((name) => [name, []]),
);

const attachPluginHooks = (registry, target) => {
  for (const name of pluginHookNames) {
    const hook = target?.[name];
    if (typeof hook === 'function') {
      registry[name].push(hook);
    }
  }
};

export const createMilkRustEngine = async ({
  audioContext,
  audioNode,
  canvas,
  modulePath,
  rendererBackend = 'webgl2',
}) => {
  const rustModule = await loadMilkRustModule(modulePath);
  const rustEngine = new rustModule.MilkRustEngine(canvas);
  const frameReader = createFrameReader(audioContext, audioNode, typeof rustEngine.setPcmStereo === 'function');
  const webGpuStatus = getWebGpuStatus(rendererBackend);
  let presetIndex = 0;
  let wasWaitingForTextures = false;
  let wasWaitingForContext = false;
  let presetLibrary = [...milkrustPresets];
  let activePresetTitle = rustEngine.loadPresetText(
    presetLibrary[presetIndex].source,
    presetLibrary[presetIndex].name,
    '{}',
  );
  let automation = normalizeAutomation();
  let beatState = {};
  let lastAutomatedPresetAt = 0;
  let mouseState = {
    mouse_down: 0,
    mouse_dx: 0,
    mouse_dy: 0,
    mouse_x: 0.5,
    mouse_y: 0.5,
  };
  let pluginHooks = createPluginHooks();
  let installedPlugins = [];
  let pluginState = {
    activePresetName: activePresetTitle,
    lastPresetIndex: presetIndex,
    automation,
    pluginCount: 0,
  };

  const runPluginHooks = (name, context = {}) => {
    const hooks = pluginHooks[name] || [];
    let nextContext = { ...context };
    for (const hook of hooks) {
      try {
        const next = hook(nextContext);
        if (next && typeof next === 'object') {
          nextContext = { ...nextContext, ...next };
        }
      } catch (error) {
        console.warn(`MilkRust plugin hook error (${name}):`, error?.message || error);
      }
    }
    return nextContext;
  };

  const updatePluginState = (next = {}) => {
    pluginState = {
      ...pluginState,
      ...next,
    };
    if (next?.automation) {
      pluginState.automation = normalizeAutomation(next.automation);
    }
    return pluginState;
  };

  const installPlugins = (nextPlugins = []) => {
    pluginHooks = createPluginHooks();
    const plugins = Array.isArray(nextPlugins) ? nextPlugins : [];
    installedPlugins = [];
    for (const plugin of plugins) {
      const hookSource = plugin?.api ?? plugin?.payload ?? plugin;
      if (!hookSource || (typeof hookSource !== 'object' && typeof hookSource !== 'function')) {
        continue;
      }
      if (plugin?.id && !installedPlugins.some((entry) => entry.id === plugin.id)) {
        installedPlugins.push({
          id: String(plugin.id),
          kind: String(plugin.kind || 'data'),
          source: String(plugin.source || (plugin.kind === 'js' ? 'module' : 'data')),
          api: hookSource,
          url: plugin.url,
        });
      }
      attachPluginHooks(pluginHooks, hookSource);
      if (typeof hookSource === 'function') {
        pluginHooks.onRenderFrame.push((context) => hookSource(context));
      }
    }
    pluginState.pluginCount = installedPlugins.length;
    return {
      pluginCount: installedPlugins.length,
      pluginHooks: Object.fromEntries(
        pluginHookNames.map((name) => [name, pluginHooks[name].length]),
      ),
      plugins: installedPlugins,
    };
  };

  const pluginDescriptorsToLoad = (pack) => {
    if (Array.isArray(pack?.plugins)) return pack.plugins;
    if (Array.isArray(pack?.plugins?.plugins)) return pack.plugins.plugins;
    return [];
  };

  const runPresetHooks = (preset, source, options = {}) => {
    let context = {
      preset,
      source,
      presetName: preset?.name || '',
      presetIndex,
      textureAssets: options.textureAssets,
      automation,
      timestamp: audioContext.currentTime || 0,
    };
    context = runPluginHooks('onPresetLoad', context);
    context = runPluginHooks('onPresetLoaded', context);
    return context;
  };

  const runFrameHooks = (now, frame) => {
    const frameContext = runPluginHooks('onFrameStart', {
      now,
      presetIndex,
      presetName: activePresetTitle,
      automation,
      frame,
      bands: frame.bands,
      spectrum: frame.spectrum,
      samples: frame.samples,
    });
    runPluginHooks('onAudioFrame', frameContext);
    return frameContext;
  };

  const loadPreset = (index, options = {}) => {
    presetIndex = (index + presetLibrary.length) % presetLibrary.length;
    const preset = presetLibrary[presetIndex];
    if (!preset) return null;
    const hookResult = runPresetHooks(preset, preset.source, options);
    const presetSource = typeof hookResult.source === 'string' ? hookResult.source : preset.source;
    const presetName = typeof hookResult.presetName === 'string' ? hookResult.presetName : preset.name;
    activePresetTitle = rustEngine.loadPresetText(
      presetSource,
      presetName,
      textureAssetsJson(options.textureAssets),
    );
    updatePluginState({
      activePresetName: activePresetTitle,
      lastPresetIndex: presetIndex,
    });
    runPluginHooks('onPresetChange', {
      presetIndex,
      presetName: activePresetTitle,
      preset,
      presetSource,
      textureAssets: options.textureAssets,
    });
    return activePresetTitle;
  };

  const maybeAdvanceAutomatedPreset = (renderFrame, now) => {
    if (automation.mode === 'off') return null;
    if (automation.mode === 'timed') {
      if (now - lastAutomatedPresetAt < automation.timedIntervalSeconds) return null;
      lastAutomatedPresetAt = now;
      return loadPreset(presetIndex + 1);
    }

    const nextBeatState = getMilkRustBeatUpdate(
      beatState,
      renderFrame.spectrum,
      now,
      automation,
    );
    const beatContext = runPluginHooks('onBeat', {
      ...nextBeatState,
      automation,
      now,
      presetIndex,
      presetName: activePresetTitle,
      source: presetLibrary[presetIndex]?.source,
      audio: {
        bands: renderFrame.bands,
        spectrum: renderFrame.spectrum,
        samples: renderFrame.samples,
      },
    });
    beatState = {
      ...nextBeatState,
      ...beatContext,
    };
    if (
      !beatState.isBeat
      || beatState.beatCount < automation.beatsPerPreset
      || now - lastAutomatedPresetAt < defaultTransitionSeconds
    ) {
      return null;
    }
    const automationStep = runPluginHooks('onAutomationStep', {
      automation,
      now,
      presetIndex,
      beatState,
      source: presetLibrary[presetIndex]?.source,
    });
    if (automationStep?.advancePreset === false) return null;
    beatState = {
      ...nextBeatState,
      beatCount: 0,
    };
    lastAutomatedPresetAt = now;
    return loadPreset(presetIndex + 1);
  };

  return {
    name: rendererBackend === 'webgpu'
      ? 'MilkRust WebGL2 fallback'
      : 'MilkRust WebGL2',
    presetName: activePresetTitle,
    getPluginState: () => ({ ...pluginState }),
    dispose: () => {
      frameReader.disconnect();
      rustEngine.free?.();
    },
    exportPresetFragment: (type = 'shape', index = 0) => {
      const result = parseJson(rustEngine.exportPresetFragment(type, index));
      const finalized = runPluginHooks('onExport', {
        operation: 'export-preset-fragment',
        artifact: result,
        type,
        index,
      });
      return finalized.artifact || result;
    },
    exportPresetText: () => {
      const result = parseJson(rustEngine.exportPresetText());
      const finalized = runPluginHooks('onExport', {
        operation: 'export-preset-text',
        artifact: result,
      });
      return finalized.artifact || result;
    },
    getPresetDebugSnapshot: () => parseJson(
      rustEngine.getPresetDebugSnapshotJson(JSON.stringify(webGpuStatus)),
      {},
    ),
    getPresetFragmentSummary: () => parseJson(rustEngine.getPresetFragmentSummaryJson(), {
      shapes: [],
      waves: [],
    }),
    getPresetParameterSummary: () => parseJson(rustEngine.getPresetParameterSummaryJson(), {}),
    inspectPresetText: (source, fileName = '') =>
      parseJson(rustEngine.inspectPresetText(source, fileName), {
        title: fileName || 'Imported preset',
      }),
    loadPresetFragmentText: (source, fileName = '', options = {}) => {
      const type = options.type || (String(fileName).toLowerCase().endsWith('.wave')
        ? 'wave'
        : 'shape');
      const hookContext = runPluginHooks('onPresetLoad', {
        source,
        presetName: fileName,
        textureAssets: options.textureAssets,
      });
      const result = parseJson(rustEngine.loadPresetFragmentText(
        source,
        fileName,
        type,
        textureAssetsJson(hookContext.textureAssets || options.textureAssets),
      ));
      if (result?.title) activePresetTitle = result.title;
      runPluginHooks('onPresetChange', {
        presetName: activePresetTitle,
        source: typeof hookContext.source === 'string' ? hookContext.source : source,
        preset: {
          name: fileName,
        },
      });
      return result;
    },
    loadPresetText: (source, fileName = '', options = {}) => {
      const hookContext = runPluginHooks('onPresetLoad', {
        source,
        presetName: fileName,
        textureAssets: options.textureAssets,
      });
      activePresetTitle = rustEngine.loadPresetText(
        typeof hookContext.source === 'string' ? hookContext.source : source,
        typeof hookContext.presetName === 'string' ? hookContext.presetName : fileName,
        textureAssetsJson(hookContext.textureAssets || options.textureAssets),
      );
      updatePluginState({ activePresetName: activePresetTitle });
      runPluginHooks('onPresetChange', {
        presetName: activePresetTitle,
        source: typeof hookContext.source === 'string' ? hookContext.source : source,
      });
      return activePresetTitle;
    },
    loadPresetPack: (pack, options = {}) => {
      const pluginSummary = pluginDescriptorsToLoad(pack);
      if (pluginSummary.length) {
        installPlugins(pluginSummary);
      }
      const presets = Array.isArray(pack?.presets) ? pack.presets : [];
      if (!presets.length) return null;
      presetLibrary = presets.map((preset) => ({
        name: preset.name || preset.title || preset.id || preset.file || 'Pack preset',
        source: preset.source || '',
      })).filter((preset) => preset.source);
      if (!presetLibrary.length) return null;
      return loadPreset(options.index || 0, options);
    },
    nextPreset: (options = {}) => loadPreset(presetIndex + 1, options),
    randomizePresetParameters: (options = {}) => {
      const result = parseJson(rustEngine.randomizePresetParameters(
        textureAssetsJson(options.textureAssets),
      ));
      if (result?.title) activePresetTitle = result.title;
      updatePluginState({ activePresetName: activePresetTitle });
      return result;
    },
    loadPlugins: (plugins = []) => installPlugins(plugins),
    removePresetFragment: (type = 'shape', index = 0, options = {}) => {
      const result = parseJson(rustEngine.removePresetFragment(
        type === 'wave' ? 'wave' : 'shape',
        index,
        textureAssetsJson(options.textureAssets),
      ));
      if (result?.title) activePresetTitle = result.title;
      updatePluginState({ activePresetName: activePresetTitle });
      return result;
    },
    render: () => {
      if (typeof rustEngine.renderStatusJson === 'function' &&
          parseJson(rustEngine.renderStatusJson())?.state === 'context-lost') {
        wasWaitingForContext = true;
        return { presetName: activePresetTitle, waitingForContext: true };
      }
      const contextRecovered = wasWaitingForContext;
      wasWaitingForContext = false;
      const now = audioContext.currentTime || 0;
      const localClock = new Date();
      rustEngine.setClockState(
        localClock.getFullYear(),
        localClock.getMonth() + 1,
        localClock.getDate(),
        localClock.getDay(),
        localClock.getHours(),
        localClock.getMinutes(),
        localClock.getSeconds(),
        localClock.getMilliseconds(),
      );
      const frame = frameReader.read();
      runFrameHooks(now, frame);
      const automatedPresetName = maybeAdvanceAutomatedPreset(frame, now);
      if (frame.pcmStereo) rustEngine.setPcmStereo(frame.pcmStereo, now);
      rustEngine.render(
        now,
        frame.bands.bass,
        frame.bands.mid,
        frame.bands.treble,
        toCsv(frame.samples),
        toCsv(frame.spectrum.map((value) => value / 255)),
        mouseState.mouse_down,
        mouseState.mouse_x,
        mouseState.mouse_y,
        mouseState.mouse_dx,
        mouseState.mouse_dy,
      );
      mouseState.mouse_dx = 0;
      mouseState.mouse_dy = 0;
      const renderStatus = typeof rustEngine.renderStatusJson === 'function'
        ? parseJson(rustEngine.renderStatusJson()) : null;
      if (renderStatus?.state === 'failed') {
        return { error: renderStatus.error || 'Preset rendering failed', fidelity: 'not-assessed' };
      }
      const waitingForTextures = renderStatus?.state === 'waiting-textures';
      const textureStateChanged = wasWaitingForTextures !== waitingForTextures;
      wasWaitingForTextures = waitingForTextures;
      runPluginHooks('onRenderFrame', {
        now,
        frame,
        presetIndex,
        presetName: activePresetTitle,
      });
      return automatedPresetName || waitingForTextures || textureStateChanged || contextRecovered
        ? { presetName: automatedPresetName || activePresetTitle, waitingForTextures,
          ...(contextRecovered ? { waitingForContext: false } : {}) } : null;
    },
    resize: (width, height) => {
      rustEngine.resize(width, height);
    },
    setMouseState: (nextMouseState = {}) => {
      mouseState = {
        ...mouseState,
        ...nextMouseState,
      };
      runPluginHooks('onInput', {
        state: mouseState,
        input: nextMouseState,
      });
      return mouseState;
    },
    setPresetAutomation: (nextAutomation = {}) => {
      automation = normalizeAutomation(nextAutomation);
      updatePluginState({ automation });
      beatState = {};
      lastAutomatedPresetAt = audioContext.currentTime || 0;
      return automation;
    },
    updatePresetBaseValue: (key, value, options = {}) => {
      const result = parseJson(rustEngine.updatePresetBaseValue(
        key,
        Number(value),
        textureAssetsJson(options.textureAssets),
      ));
      if (result?.title) activePresetTitle = result.title;
      return result;
    },
  };
};
