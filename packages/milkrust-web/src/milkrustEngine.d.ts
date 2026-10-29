export type MilkRustRendererBackend = 'webgl' | 'webgpu' | 'headless';

export type MilkRustPluginKind = 'data' | 'js';

export interface MilkRustPresetSource {
  id: string;
  title: string;
  file: string;
  name?: string;
  sourceFormat?: string;
  tags?: string[];
  thumbnail?: string;
  source?: string;
  url?: string;
}

export interface MilkRustPackTexture {
  id: string;
  file: string;
  aliases?: string[];
  url?: string;
}

export interface MilkRustPackFragment {
  id: string;
  kind?: string;
  file: string;
  tags?: string[];
  url?: string;
}

export interface MilkRustPackPluginManifest {
  id: string;
  kind: MilkRustPluginKind;
  entry: string;
  source?: string;
  url?: string;
}

export interface MilkRustPackManifest {
  schemaVersion: number;
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  license: string;
  requiredMilkRustVersion: string;
  sourceUrls: string[];
  presets: MilkRustPresetSource[];
  textures: MilkRustPackTexture[];
  fragments: MilkRustPackFragment[];
  plugins: MilkRustPackPluginManifest[];
  playlist: string[];
  automationDefaults?: Record<string, unknown>;
}

export interface MilkRustPackValidation {
  valid: boolean;
  errors: string[];
  warnings: string[];
  manifest: MilkRustPackManifest;
  manifestUrl?: string;
}

export interface MilkRustPackLoadResult extends MilkRustPackValidation {
  presets: MilkRustPresetSource[];
}

export interface MilkRustPluginDescriptor {
  id: string;
  kind: MilkRustPluginKind | string;
  source: string;
  entry: string;
  url?: string;
  payload?: unknown;
  api?: MilkRustPluginHooks | ((context: MilkRustPluginContext) => MilkRustPluginContext);
  module?: unknown;
}

export interface MilkRustPluginLoadResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
  plugins: MilkRustPluginDescriptor[];
  manifest: MilkRustPackManifest;
}

export interface MilkRustPluginContext {
  preset?: { name?: string };
  presetName?: string;
  source?: string;
  presetIndex?: number;
  timestamp?: number;
  now?: number;
  frame?: {
    bands: { bass: number; mid: number; treble: number };
    spectrum: number[];
    samples: number[];
  };
  automation?: MilkRustAutomation;
  [key: string]: unknown;
}

export interface MilkRustPluginState {
  activePresetName: string;
  lastPresetIndex: number;
  pluginCount: number;
  automation?: MilkRustAutomation;
}

export interface MilkRustPluginHooks {
  onPresetLoad?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onPresetLoaded?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onPresetChange?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onFrameStart?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onAudioFrame?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onBeat?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onAutomationStep?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onRenderFrame?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onInput?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
  onExport?: (context: MilkRustPluginContext) => MilkRustPluginContext | void;
}

export interface MilkRustAutomation {
  beatSensitivity?: number;
  beatsPerPreset?: number;
  minBeatIntervalSeconds?: number;
  mode?: 'off' | 'beat' | 'time';
  transitionMode?: 'crossfade' | 'fade_through_black' | 'overlay' | 'cut' | string;
  timedIntervalSeconds?: number;
  transitionSeconds?: number;
  randomizeOnBeat?: boolean;
}

export interface MilkRustEngineConfig {
  audioContext: AudioContext;
  audioNode: AudioNode;
  canvas: HTMLCanvasElement;
  modulePath?: string;
  rendererBackend?: MilkRustRendererBackend;
  transitionSeconds?: number;
  defaultPresetName?: string;
}

export interface MilkRustPresetResult {
  title?: string;
  source?: string;
  values?: Record<string, unknown>;
  fileName?: string;
}

export interface MilkRustFrame {
  bands: { bass: number; mid: number; treble: number };
  samples: number[];
  spectrum: number[];
}

export interface MilkRustRenderState {
  error?: string;
  fidelity?: 'not-assessed';
  waitingForTextures?: boolean;
  waitingForContext?: boolean;
  presetName?: string;
  presetIndex?: number;
}

export interface MilkRustEngine {
  name: string;
  presetName: string;
  getPluginState(): MilkRustPluginState;
  dispose(): void;
  exportPresetFragment(type?: 'shape' | 'wave', index?: number): MilkRustPresetResult;
  exportPresetText(): MilkRustPresetResult;
  getPresetDebugSnapshot(): unknown;
  getPresetFragmentSummary(): unknown;
  getPresetParameterSummary(): unknown;
  inspectPresetText(source: string, fileName?: string): MilkRustPresetResult;
  loadPresetFragmentText(
    source: string,
    fileName: string,
    options?: { type?: 'shape' | 'wave'; textureAssets?: Record<string, string> },
  ): MilkRustPresetResult;
  loadPresetText(source: string, fileName?: string, options?: { textureAssets?: Record<string, string> }): string;
  loadPresetPack(
    pack: { presets?: MilkRustPresetSource[]; plugins?: MilkRustPluginDescriptor[] },
    options?: { index?: number; textureAssets?: Record<string, string>; autoplay?: boolean },
  ): string | null;
  nextPreset(options?: { autoplay?: boolean; index?: number }): string | null;
  randomizePresetParameters(options?: { textureAssets?: Record<string, string> }): MilkRustPresetResult;
  loadPlugins(plugins: MilkRustPluginDescriptor[]): {
    pluginCount: number;
    pluginHooks: Record<string, number>;
    plugins: MilkRustPluginDescriptor[];
  };
  removePresetFragment(
    type: 'shape' | 'wave',
    index?: number,
    options?: { textureAssets?: Record<string, string> },
  ): MilkRustPresetResult;
  render(): MilkRustRenderState | null;
  resize(width: number, height: number): void;
  setMouseState(nextMouseState: Partial<{
    mouse_down: number;
    mouse_x: number;
    mouse_y: number;
    mouse_dx: number;
    mouse_dy: number;
  }>): Record<string, number>;
  setPresetAutomation(nextAutomation: MilkRustAutomation): MilkRustAutomation;
  updatePresetBaseValue(key: string, value: number, options?: { textureAssets?: Record<string, string> }): MilkRustPresetResult;
}

export function normalizeStringArray(values?: unknown): string[];
export function normalizeMilkRustPackManifest(
  manifest?: unknown,
  manifestUrl?: string,
): MilkRustPackManifest;
export function validateMilkRustPackManifest(
  manifest?: unknown,
  manifestUrl?: string,
): MilkRustPackValidation;
export function loadMilkRustPackManifest(
  packUrl: string,
  options?: { fetchImpl?: typeof fetch },
): Promise<MilkRustPackValidation>;
export function loadMilkRustPackPresetSource(
  preset: Pick<MilkRustPresetSource, 'url' | 'file'>,
  options?: { fetchImpl?: typeof fetch },
): Promise<string>;
export function loadMilkRustPackPlugins(
  manifest: { manifest?: MilkRustPackManifest; manifestUrl?: string } | MilkRustPackManifest,
  options?: { fetchImpl?: typeof fetch },
): Promise<MilkRustPluginLoadResult>;
export function loadMilkRustPack(
  packUrl: string,
  options?: { fetchImpl?: typeof fetch },
): Promise<MilkRustPackLoadResult & MilkRustPluginLoadResult>;

export function getMilkRustTransitionProgress(
  frameInterval: number,
  frameIndex: number,
  totalFrames: number,
): number;
export function getMilkRustTransitionAlphas(
  transitionProgress: number,
  transitionMode?: MilkRustAutomation['transitionMode'],
): { incoming: number; outgoing: number };

export function getMilkRustBeatUpdate(
  baseline: Record<string, unknown> | null,
  spectrum: number[],
  now: number,
  automation?: MilkRustAutomation,
): {
  isBeat: boolean;
  beatCount: number;
  beatSensitivity?: number;
};

export function createMilkRustEngine(config: MilkRustEngineConfig): Promise<MilkRustEngine>;
