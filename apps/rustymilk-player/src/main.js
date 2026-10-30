import {
  createMilkRustEngine,
  loadMilkRustPackManifest,
  loadMilkRustPackPlugins,
  loadMilkRustPackPresetSource,
} from '/packages/milkrust-web/src/milkrustEngine.js';

const canvas = document.querySelector('#visualizer');
const status = document.querySelector('#status');
const debug = document.querySelector('#debug');
const packList = document.querySelector('#pack-list');
const presetList = document.querySelector('#preset-list');
const presetSearch = document.querySelector('#preset-search');
const automation = document.querySelector('#automation');
const interval = document.querySelector('#interval');
const beats = document.querySelector('#beats');
const playlistSelect = document.querySelector('#playlist-list');
const playlistNameInput = document.querySelector('#playlist-name');
const playlistImportInput = document.querySelector('#playlist-import');
const toggleFavoriteButton = document.querySelector('#toggle-favorite');
const viewAllButton = document.querySelector('#view-all');
const prevHistoryButton = document.querySelector('#prev-history');
const nextHistoryButton = document.querySelector('#next-history');
const savePlaylistButton = document.querySelector('#save-playlist');
const updatePlaylistButton = document.querySelector('#update-playlist');
const renamePlaylistButton = document.querySelector('#rename-playlist');
const clearPlaylistButton = document.querySelector('#clear-playlist');
const deletePlaylistButton = document.querySelector('#delete-playlist');
const exportPlaylistsButton = document.querySelector('#export-playlists');

const builtInPresets = [
  {
    id: 'builtin-grid-smoke',
    name: 'MilkRust Grid Smoke',
    source: 'name=MilkRust Grid Smoke\ndecay=0.91\nwave_r=0.12\nwave_g=0.64\nwave_b=0.88\nwave_a=0.86\nwave_scale=1.2\nzoom=1\nrot=0\nper_frame_1=rot=0.02*sin(time);\nshape00_enabled=1\nshape00_sides=5\nshape00_rad=0.22\nshape00_x=0.5\nshape00_y=0.5\nshape00_r=0.1\nshape00_g=0.9\nshape00_b=0.45\nshape00_a=0.42\nshape00_r2=0.9\nshape00_g2=0.8\nshape00_b2=0.2\nshape00_a2=0.18\nshape00_border_a=0.85\nwavecode_0_enabled=1\nwavecode_0_samples=96\nwavecode_0_spectrum=1\nwavecode_0_r=0.7\nwavecode_0_g=0.95\nwavecode_0_b=0.25\nwavecode_0_a=0.82\nwavecode_0_per_point1=x=i;\nwavecode_0_per_point2=y=0.5+sample*0.35;',
  },
  {
    id: 'builtin-amber-tunnel',
    name: 'MilkRust Amber Tunnel',
    source: 'name=MilkRust Amber Tunnel\ndecay=0.86\nwave_r=0.92\nwave_g=0.52\nwave_b=0.18\nwave_a=0.82\nwave_scale=1.55\nzoom=1.05\nrot=-0.018\nper_frame_1=dx=0.02*sin(time*0.4);\nper_frame_2=dy=0.015*cos(time*0.3);\nshape00_enabled=1\nshape00_sides=3\nshape00_rad=0.15\nshape00_x=0.35\nshape00_y=0.55\nshape00_r=0.9\nshape00_g=0.2\nshape00_b=0.1\nshape00_a=0.32\nshape01_enabled=1\nshape01_sides=6\nshape01_rad=0.11\nshape01_x=0.67\nshape01_y=0.45\nshape01_r=0.1\nshape01_g=0.55\nshape01_b=0.95\nshape01_a=0.36\nwavecode_0_enabled=1\nwavecode_0_samples=128\nwavecode_0_r=0.95\nwavecode_0_g=0.85\nwavecode_0_b=0.2\nwavecode_0_a=0.8\nwavecode_0_per_point1=x=i;\nwavecode_0_per_point2=y=0.5+sample*0.35;',
  },
];

let audioContext;
let audioNode;
let oscillator;
let engine;
let animationFrame = 0;
let activeIndex = 0;
let presets = [...builtInPresets];
let textureAssets = {};
let activeLoadToken = 0;
let favoriteViewOnly = false;
let historyStack = [];
let historyIndex = -1;
let favorites = new Set();
let playlists = [];
let activePlaylistId = '';
let activePackId = 'builtin';
const FAVORITE_STORAGE_KEY = 'milkrust-player-favorites-v1';
const PLAYLISTS_STORAGE_KEY = 'milkrust-player-playlists-v1';
const ACTIVE_PLAYLIST_STORAGE_KEY = 'milkrust-player-active-playlist-v1';
const PLAYLIST_EXPORT_VERSION = 1;
const PLAYLIST_EXPORT_FILE_PREFIX = 'milkrust-playlists';

try {
  const storedFavorites = JSON.parse(localStorage.getItem(FAVORITE_STORAGE_KEY) || '[]');
  if (Array.isArray(storedFavorites)) {
    storedFavorites.forEach((entry) => {
      if (typeof entry === 'string' && entry.length > 0) {
        favorites.add(entry);
      }
    });
  }
} catch {
  favorites = new Set();
}

try {
  const storedPlaylists = JSON.parse(localStorage.getItem(PLAYLISTS_STORAGE_KEY) || '[]');
  if (Array.isArray(storedPlaylists)) {
    playlists = storedPlaylists
      .filter((entry) => entry && typeof entry === 'object')
      .map((entry) => ({
        id: String(entry.id || '').trim(),
        name: String(entry.name || '').trim(),
        packId: String(entry.packId || '').trim() || 'builtin',
        presetIds: Array.isArray(entry.presetIds)
          ? [...new Set(entry.presetIds.map((id) => String(id)).filter(Boolean))]
          : [],
      }))
      .filter((entry) => entry.id.length > 0 && entry.name.length > 0);
  }
} catch {
  playlists = [];
}
activePlaylistId = String(localStorage.getItem(ACTIVE_PLAYLIST_STORAGE_KEY) || '');

const builtinPack = {
  id: 'builtin',
  name: 'Built-in',
  presets: builtInPresets,
};

const resolvePluginPlaylist = (plugins = [], presets = []) => {
  const presetById = new Map(
    presets.map((preset) => [String(preset.id || preset.title || preset.name || ''), preset]),
  );
  const playlistPlugin = plugins.find((plugin) => {
    const payloadKind = String(plugin?.payload?.kind || plugin?.kind || '').toLowerCase();
    return plugin?.payload && payloadKind.includes('playlist');
  });
  if (!playlistPlugin?.payload?.presetIds?.length) return null;
  const pluginPresetIds = playlistPlugin.payload.presetIds;
  const ordered = pluginPresetIds
    .map((id) => presetById.get(String(id)))
    .filter(Boolean);
  if (!ordered.length) return null;
  return {
    presets: ordered,
  };
};

const persistFavorites = () => {
  localStorage.setItem(FAVORITE_STORAGE_KEY, JSON.stringify(Array.from(favorites)));
};

const persistPlaylists = () => {
  const unique = [];
  const seen = new Set();
  playlists.forEach((playlist) => {
    const key = `${playlist.packId || 'builtin'}::${playlist.id}`;
    if (seen.has(key)) return;
    seen.add(key);
    unique.push({
      id: playlist.id,
      name: playlist.name,
      packId: playlist.packId || 'builtin',
      presetIds: Array.isArray(playlist.presetIds) ? playlist.presetIds : [],
    });
  });
  playlists = unique;
  localStorage.setItem(PLAYLISTS_STORAGE_KEY, JSON.stringify(unique));
  if (activePlaylistId) {
    localStorage.setItem(ACTIVE_PLAYLIST_STORAGE_KEY, activePlaylistId);
  } else {
    localStorage.removeItem(ACTIVE_PLAYLIST_STORAGE_KEY);
  }
};

const cloneAndNormalizeImportInput = (value, fallbackPackId = activePackId) => {
  const raw = String(value || '').trim();
  return raw.length > 0 ? raw : fallbackPackId;
};

const sanitizePlaylistId = (value) => String(value || '').trim();

const normalizeImportedPlaylist = (entry = {}) => {
  const packId = cloneAndNormalizeImportInput(entry.packId, activePackId);
  const id = sanitizePlaylistId(entry.id) || crypto.randomUUID();
  const name = sanitizePlaylistId(entry.name || entry.title);
  const presetIds = Array.isArray(entry.presetIds)
    ? Array.from(new Set(
      entry.presetIds.map((value) => String(value || '').trim()).filter(Boolean),
    ))
    : [];
  if (!packId) return null;
  return {
    id,
    name: name || `Imported Playlist ${id.slice(0, 8)}`,
    packId,
    presetIds,
  };
};

const ensureUniquePlaylistId = (playlist = {}, collection = playlists) => {
  if (!playlist || !playlist.id || !playlist.packId) return playlist;
  let id = playlist.id;
  let collision = collection.some((entry) => entry.id === id && entry.packId === playlist.packId);
  while (collision) {
    id = crypto.randomUUID();
    collision = collection.some((entry) => entry.id === id && entry.packId === playlist.packId);
  }
  return { ...playlist, id };
};

const ensureUniquePlaylistName = (playlist = {}, collection = playlists) => {
  if (!playlist || !playlist.name || !playlist.packId) return playlist;
  let name = playlist.name;
  let suffix = 2;
  let normalized = name.toLowerCase();
  while (collection.some((entry) => (
    entry.packId === playlist.packId
    && entry.id !== playlist.id
    && entry.name.toLowerCase() === normalized
  ))) {
    name = `${playlist.name} (${suffix})`;
    normalized = name.toLowerCase();
    suffix += 1;
  }
  return { ...playlist, name };
};

const collectCurrentPackPlaylists = (packId = activePackId) => playlists
  .filter((playlist) => playlist.packId === packId);

const parsePlaylistImportDocument = (text) => {
  try {
    const parsed = JSON.parse(text);
    if (!parsed) return [];
    if (Array.isArray(parsed)) {
      return parsed.map((entry) => normalizeImportedPlaylist(entry)).filter(Boolean);
    }
    if (Array.isArray(parsed.playlists)) {
      return parsed.playlists.map((entry) => normalizeImportedPlaylist(entry)).filter(Boolean);
    }
    if (parsed.playlist) {
      return [normalizeImportedPlaylist(parsed.playlist)].filter(Boolean);
    }
    return [];
  } catch {
    return [];
  }
};

const buildPlaylistExportPayload = (playlistsToExport = collectCurrentPackPlaylists()) => {
  const normalized = Array.isArray(playlistsToExport)
    ? playlistsToExport.map((playlist) => ({
      id: playlist.id,
      name: playlist.name,
      packId: playlist.packId,
      presetIds: playlist.presetIds,
    }))
    : [];
  return {
    schemaVersion: PLAYLIST_EXPORT_VERSION,
    kind: 'milkrust-playlist-export',
    generatedAt: new Date().toISOString(),
    packId: activePackId,
    playlists: normalized,
  };
};

const startPlaylistDownload = (playlistData) => {
  const payload = JSON.stringify(buildPlaylistExportPayload(playlistData), null, 2);
  const blob = new Blob([payload], {
    type: 'application/json;charset=utf-8',
  });
  const fileName = `${PLAYLIST_EXPORT_FILE_PREFIX}-${activePackId}-${new Date()
    .toISOString()
    .replace(/[-:.]/g, '')
    .replace('T', '-')}.json`;
  const downloadUrl = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = downloadUrl;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(downloadUrl);
};

const importPlaylistsFromText = async (text = '') => {
  const imported = parsePlaylistImportDocument(text);
  if (!imported.length) {
    return 0;
  }
  const before = playlists;
  const targetActive = activePlaylistId || '';
  const importedResolved = [];
  playlists = imported.reduce((accum, entry) => {
    const normalized = ensureUniquePlaylistName(ensureUniquePlaylistId(entry, accum));
    importedResolved.push(normalized);
    accum.push(normalized);
    return accum;
  }, [...before]);
  if (!targetActive && importedResolved.length) {
    activePlaylistId = importedResolved[0].id;
  }
  persistPlaylists();
  refreshPlaylistControls();
  return imported.length;
};

const getActivePlaylist = () => playlists.find((playlist) => playlist.id === activePlaylistId && playlist.packId === activePackId);

const getVisiblePlaylistIdSet = () => {
  const activePlaylist = getActivePlaylist();
  if (!activePlaylist || !Array.isArray(activePlaylist.presetIds)) return null;
  return new Set(activePlaylist.presetIds);
};

const presetKey = (preset = {}, fallbackIndex = 0) => {
  const rawId = preset.id || preset.title || preset.name || preset.file;
  if (rawId) return String(rawId);
  return `${fallbackIndex}:${preset.packId || preset.pack || 'local'}`;
};

const normalizePresetIds = (sourcePresets = [], packId = 'pack') => sourcePresets.map((preset, index) => ({
  ...preset,
  id: String(preset.id || preset.title || preset.name || preset.file || `${packId}:${index}`),
  packId,
}));

const visiblePresetEntries = () => {
  const playlistPresetIds = getVisiblePlaylistIdSet();
  const query = String(presetSearch?.value || '').trim().toLowerCase();
  return presets
    .map((preset, index) => ({ preset, index }))
    .filter(({ preset, index }) => {
      if (playlistPresetIds && playlistPresetIds.size > 0 && !playlistPresetIds.has(presetKey(preset, index))) return false;
      if (favoriteViewOnly && !favorites.has(presetKey(preset, index))) return false;
      return query.length === 0 || presetLabel(preset).toLowerCase().includes(query);
    });
};

const updateFavoriteControls = () => {
  const preset = presets[activeIndex];
  const key = preset ? presetKey(preset, activeIndex) : '';
  if (key) {
    toggleFavoriteButton.textContent = favorites.has(key) ? '♥ Unfavorite' : '♡ Favorite';
  }
  viewAllButton.textContent = favoriteViewOnly ? 'View: Favorites' : 'View: All';
  const hasAnyFavorite = presets.some((preset, index) => favorites.has(presetKey(preset, index)));
  viewAllButton.disabled = !hasAnyFavorite;
  prevHistoryButton.disabled = historyIndex <= 0;
  nextHistoryButton.disabled = historyIndex < 0 || historyIndex >= (historyStack.length - 1);
  const hasActivePlaylist = Boolean(getActivePlaylist());
  updatePlaylistControls({
    hasActivePlaylist,
  });
};

const ensureActivePlaylistValid = () => {
  if (activePlaylistId) {
    const activePlaylist = getActivePlaylist();
    if (!activePlaylist) {
      activePlaylistId = '';
    }
  }
  if (!activePlaylistId || !getActivePlaylist()) {
    localStorage.removeItem(ACTIVE_PLAYLIST_STORAGE_KEY);
  }
};

const refreshPlaylistList = () => {
  const currentPackPlaylists = playlists
    .filter((playlist) => playlist.packId === activePackId)
    .sort((a, b) => a.name.localeCompare(b.name));
  const options = currentPackPlaylists.map((playlist) => {
    const option = document.createElement('option');
    option.value = playlist.id;
    option.textContent = `${playlist.name} (${playlist.presetIds.length})`;
    return option;
  });
  if (options.length === 0) {
    const option = document.createElement('option');
    option.value = '';
    option.textContent = 'No playlists';
    options.push(option);
  }
  playlistSelect.replaceChildren(...options);
  const activePlaylist = getActivePlaylist();
  playlistSelect.value = activePlaylist ? activePlaylist.id : '';
};

const updatePlaylistControls = ({
  hasActivePlaylist = false,
} = {}) => {
  const canModifyPlaylist = hasActivePlaylist;
  playlistNameInput.disabled = false;
  if (!canModifyPlaylist) {
    playlistSelect.value = '';
  }
  playlistNameInput.value = canModifyPlaylist ? (playlistNameInput.value || getActivePlaylist()?.name || '') : playlistNameInput.value;
  const presetName = playlistNameInput.value.trim().length > 0;
  savePlaylistButton.disabled = !presetName;
  const hasCurrentPack = playlists.some((playlist) => playlist.packId === activePackId);
  playlistSelect.disabled = !hasCurrentPack;
  updatePlaylistButton.disabled = !canModifyPlaylist;
  renamePlaylistButton.disabled = !canModifyPlaylist || !presetName;
  clearPlaylistButton.disabled = !canModifyPlaylist;
  deletePlaylistButton.disabled = !canModifyPlaylist;
  const currentPackHasPlaylists = getCurrentPackPlaylists().length > 0;
  exportPlaylistsButton.disabled = !currentPackHasPlaylists;
};

const refreshPlaylistControls = () => {
  const activePlaylist = getActivePlaylist();
  if (!activePlaylist) {
    activePlaylistId = '';
    localStorage.removeItem(ACTIVE_PLAYLIST_STORAGE_KEY);
  }
  refreshPlaylistList();
  updatePlaylistControls({
    hasActivePlaylist: Boolean(activePlaylist),
  });
  refreshPresetList();
};

const setActivePack = (packId) => {
  activePackId = packId || 'builtin';
  ensureActivePlaylistValid();
};

const resetHistory = () => {
  historyStack = [];
  historyIndex = -1;
  updateFavoriteControls();
};

const recordHistory = (index) => {
  if (historyIndex >= 0 && historyStack[historyIndex] === index) {
    updateFavoriteControls();
    return;
  }
  historyStack = historyStack.slice(0, historyIndex + 1);
  historyStack.push(index);
  if (historyStack.length > 256) {
    historyStack = historyStack.slice(historyStack.length - 256);
  }
  historyIndex = historyStack.length - 1;
  updateFavoriteControls();
};

let packLibrary = [
  builtinPack,
  {
    id: 'milkrust-sample-pack',
    name: 'MilkRust Sample Pack',
    path: '/examples/sample-pack/',
  },
];

const setStatus = (value) => {
  status.textContent = value;
};

const presetLabel = (preset) => preset.name || preset.title || preset.id || preset.file || 'Preset';

const refreshPackList = () => {
  packList.replaceChildren(...packLibrary.map((pack) => {
    const option = document.createElement('option');
    option.value = pack.id;
    option.textContent = pack.name;
    return option;
  }));
};

const refreshPresetList = () => {
  const entries = visiblePresetEntries();
  presetList.replaceChildren(...entries.map(({ preset, index }) => {
    const option = document.createElement('option');
    option.value = String(index);
    option.textContent = presetLabel(preset);
    return option;
  }));
  if (!entries.length) {
    const option = document.createElement('option');
    option.value = '';
    option.textContent = 'No presets to show';
    presetList.replaceChildren(option);
    presetList.disabled = true;
  } else {
    presetList.disabled = false;
    const activeEntry = entries.find((entry) => entry.index === activeIndex);
    if (activeEntry) {
      presetList.value = String(activeIndex);
    } else {
      activeIndex = entries[0].index;
      presetList.value = String(activeIndex);
    }
  }
  presetList.value = String(activeIndex);
  updateFavoriteControls();
};

const resize = () => {
  const width = Math.max(1, Math.floor(canvas.clientWidth * window.devicePixelRatio));
  const height = Math.max(1, Math.floor(canvas.clientHeight * window.devicePixelRatio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
    engine?.resize(width, height);
  }
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

const loadActivePreset = async (recordVisit = true) => {
  if (!engine) return;
  const loadToken = ++activeLoadToken;
  const preset = presets[activeIndex];
  if (!preset) return;
  if (!preset.source && preset.url) {
    setStatus(`Loading ${presetLabel(preset)}`);
    preset.source = await loadMilkRustPackPresetSource(preset);
  }
  if (loadToken !== activeLoadToken) return;
  const title = engine.loadPresetText(preset.source, presetLabel(preset), { textureAssets });
  setStatus(title);
  debug.textContent = JSON.stringify(engine.getPresetDebugSnapshot(), null, 2);
  refreshPresetList();
  if (recordVisit) {
    recordHistory(activeIndex);
  }
};

const render = () => {
  resize();
  const update = engine?.render();
  if (window.__milkrustCollectStats) {
    window.__milkrustPlayerStats = readCanvasStats();
  }
  if (update?.error) setStatus(`Renderer stopped: ${update.error}`);
  else if (update?.waitingForTextures) setStatus('Loading preset textures…');
  else if (update?.presetName) setStatus(update.presetName);
  animationFrame = requestAnimationFrame(render);
};

const stopEngine = () => {
  cancelAnimationFrame(animationFrame);
  engine?.dispose();
  oscillator?.stop?.();
  engine = null;
  oscillator = null;
};

const startWithNode = async (context, node) => {
  stopEngine();
  audioContext = context;
  audioNode = node;
  engine = await createMilkRustEngine({
    audioContext,
    audioNode,
    canvas,
    modulePath: '/pkg/milkrust_wasm.js',
  });
  engine.setPresetAutomation({
    beatsPerPreset: Number(beats.value) || 8,
    mode: automation.value,
    timedIntervalSeconds: Number(interval.value) || 30,
  });
  await loadActivePreset();
  render();
  window.__milkrustPlayerReady = true;
};

const visiblePresetKeys = () => visiblePresetEntries().map(({ preset, index }) => ({
  key: presetKey(preset, index),
  index,
}));

const stepVisiblePreset = (delta) => {
  const entries = visiblePresetEntries();
  if (!entries.length) return;
  let position = entries.findIndex((entry) => entry.index === activeIndex);
  if (position === -1) position = 0;
  position = (position + delta) % entries.length;
  if (position < 0) {
    position += entries.length;
  }
  activeIndex = entries[position].index;
  loadActivePreset();
};

const stepHistory = (delta) => {
  const nextIndex = historyIndex + delta;
  if (nextIndex < 0 || nextIndex >= historyStack.length) return;
  historyIndex = nextIndex;
  activeIndex = historyStack[nextIndex];
  loadActivePreset(false);
};

const setActivePlaylist = (playlistId) => {
  activePlaylistId = playlistId || '';
  ensureActivePlaylistValid();
  if (activePlaylistId) {
    localStorage.setItem(ACTIVE_PLAYLIST_STORAGE_KEY, activePlaylistId);
  } else {
    localStorage.removeItem(ACTIVE_PLAYLIST_STORAGE_KEY);
  }
  refreshPlaylistControls();
};

const loadPack = async (packId) => {
  const pack = packLibrary.find((entry) => entry.id === packId) || builtinPack;
  setStatus(`Loading ${pack.name}`);
  setActivePack(pack.id);
  if (pack.presets) {
    presets = normalizePresetIds(pack.presets, pack.id);
  } else {
    const loaded = await loadMilkRustPackManifest(pack.path || pack.url);
    const pluginState = await loadMilkRustPackPlugins(loaded);
    const pluginPackPresets = loaded.manifest.presets.map((preset) => ({
      ...preset,
      name: preset.title || preset.id || preset.file,
      packId: loaded.manifest.id,
    }));
    const pluginPlaylist = resolvePluginPlaylist(pluginState.plugins || [], pluginPackPresets);
    presets = normalizePresetIds(pluginPlaylist?.presets || pluginPackPresets, loaded.manifest.id);
    if (pluginState.plugins?.length && engine?.loadPlugins) {
      engine.loadPlugins(pluginState.plugins);
    }
    pack.presets = [...presets];
  }
  resetHistory();
  activeIndex = 0;
  refreshPlaylistControls();
  recordHistory(activeIndex);
  await loadActivePreset();
  if (!engine) {
    setStatus(`${pack.name}: ${presets.length} presets`);
  }
};

const loadCommunityPackChoices = async () => {
  try {
    const response = await fetch('/content/generated/community-pack-summary.json');
    if (!response.ok) return;
    const summary = await response.json();
    const packs = Array.isArray(summary.packs) ? summary.packs : [];
    packLibrary = [
      ...packLibrary,
      ...packs
        .filter((pack) => pack.presetCount > 0)
        .map((pack) => ({
          id: pack.id,
          name: pack.name || pack.id,
          path: `/${pack.path}/`,
        })),
    ];
    refreshPackList();
  } catch {
    // The generated community index is optional in lean builds.
  }
};

const getCurrentPackPlaylists = () => playlists.filter((playlist) => playlist.packId === activePackId);

const buildPlaylistPayloadFromVisible = () => visiblePresetKeys().map(({ key }) => key);

const savePlaylist = (name, { overwrite = false } = {}) => {
  const normalizedName = String(name || '').trim();
  if (!normalizedName) return null;
  const normalizedKeyName = normalizedName.toLowerCase();
  const existing = getCurrentPackPlaylists().find((playlist) => playlist.name.toLowerCase() === normalizedKeyName);
  const payload = {
    name: normalizedName,
    packId: activePackId,
    presetIds: buildPlaylistPayloadFromVisible(),
  };
  if (existing) {
    existing.presetIds = payload.presetIds;
    activePlaylistId = existing.id;
    persistPlaylists();
    return existing;
  }
  const playlist = {
    id: overwrite && activePlaylistId ? activePlaylistId : crypto.randomUUID(),
    ...payload,
  };
  playlists = [...playlists, playlist];
  activePlaylistId = playlist.id;
  persistPlaylists();
  return playlist;
};

const updateActivePlaylist = () => {
  const playlist = getActivePlaylist();
  if (!playlist) return;
  playlist.presetIds = buildPlaylistPayloadFromVisible();
  persistPlaylists();
};

const renameActivePlaylist = (name) => {
  const playlist = getActivePlaylist();
  if (!playlist) return;
  const normalized = String(name || '').trim();
  if (!normalized) return;
  const conflict = getCurrentPackPlaylists().find((entry) => (
    entry.id !== playlist.id && entry.name.toLowerCase() === normalized.toLowerCase()
  ));
  if (conflict) return;
  playlist.name = normalized;
  persistPlaylists();
};

const clearActivePlaylist = () => {
  const playlist = getActivePlaylist();
  if (!playlist) return;
  playlist.presetIds = [];
  persistPlaylists();
};

const deleteActivePlaylist = () => {
  if (!activePlaylistId) return;
  playlists = playlists.filter((playlist) => playlist.id !== activePlaylistId || playlist.packId !== activePackId);
  activePlaylistId = '';
  persistPlaylists();
  refreshPlaylistControls();
};

const handlePlaylistExport = () => {
  const currentPackPlaylists = getCurrentPackPlaylists();
  if (!currentPackPlaylists.length) return;
  startPlaylistDownload(currentPackPlaylists);
};

const handlePlaylistImport = async (event) => {
  const files = Array.from(event.target.files || []);
  let importedCount = 0;
  for (const file of files) {
    const text = await file.text();
    importedCount += await importPlaylistsFromText(text);
  }
  if (importedCount > 0) {
    setStatus(`Imported ${importedCount} playlist${importedCount === 1 ? '' : 's'}`);
  } else {
    setStatus('No playlists imported from selected file');
  }
  event.target.value = '';
};

document.querySelector('#start-demo').addEventListener('click', async () => {
  const context = new AudioContext();
  const gain = context.createGain();
  gain.gain.value = 0.0001;
  oscillator = context.createOscillator();
  oscillator.frequency.value = 96;
  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.start();
  await startWithNode(context, oscillator);
});

document.querySelector('#start-mic').addEventListener('click', async () => {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const context = new AudioContext();
  await startWithNode(context, context.createMediaStreamSource(stream));
});

exportPlaylistsButton?.addEventListener('click', () => {
  handlePlaylistExport();
});

playlistImportInput?.addEventListener('change', (event) => {
  handlePlaylistImport(event);
});

playlistSelect.addEventListener('change', () => {
  setActivePlaylist(playlistSelect.value);
  if (!playlistSelect.value) {
    return;
  }
  const entries = visiblePresetEntries();
  if (entries.length) {
    activeIndex = entries[0].index;
    loadActivePreset();
  }
});

savePlaylistButton.addEventListener('click', () => {
  const playlist = savePlaylist(playlistNameInput.value, { overwrite: false });
  if (!playlist) return;
  refreshPlaylistControls();
  playlistSelect.value = playlist.id;
});

updatePlaylistButton.addEventListener('click', () => {
  if (!getActivePlaylist()) return;
  updateActivePlaylist();
  refreshPlaylistControls();
});

renamePlaylistButton.addEventListener('click', () => {
  const playlist = getActivePlaylist();
  if (!playlist) return;
  const nextName = playlistNameInput.value;
  if (!nextName.trim()) return;
  if (getCurrentPackPlaylists().some((entry) => (
    entry.id !== playlist.id && entry.name.toLowerCase() === nextName.trim().toLowerCase()
  ))) {
    return;
  }
  renameActivePlaylist(nextName);
  refreshPlaylistControls();
});

clearPlaylistButton.addEventListener('click', () => {
  clearActivePlaylist();
  refreshPlaylistControls();
});

deletePlaylistButton.addEventListener('click', () => {
  const playlist = getActivePlaylist();
  if (!playlist) return;
  deleteActivePlaylist();
  refreshPlaylistControls();
});

presetList.addEventListener('change', () => {
  const next = Number(presetList.value);
  if (!Number.isFinite(next) || !Number.isInteger(next)) return;
  activeIndex = next;
  loadActivePreset();
});

packList.addEventListener('change', () => {
  loadPack(packList.value);
});

document.querySelector('#previous').addEventListener('click', () => {
  stepVisiblePreset(-1);
});

document.querySelector('#next').addEventListener('click', () => {
  stepVisiblePreset(1);
});

document.querySelector('#random').addEventListener('click', () => {
  const visibleKeys = visiblePresetEntries();
  if (!visibleKeys.length) return;
  const nextVisible = visibleKeys[Math.floor(Math.random() * visibleKeys.length)];
  activeIndex = nextVisible.index;
  loadActivePreset();
});

document.querySelector('#prev-history').addEventListener('click', () => {
  stepHistory(-1);
});

document.querySelector('#next-history').addEventListener('click', () => {
  stepHistory(1);
});

document.querySelector('#view-all').addEventListener('click', () => {
  favoriteViewOnly = !favoriteViewOnly;
  refreshPresetList();
});

document.querySelector('#toggle-favorite').addEventListener('click', () => {
  const preset = presets[activeIndex];
  if (!preset) return;
  const key = presetKey(preset, activeIndex);
  if (favorites.has(key)) {
    favorites.delete(key);
  } else {
    favorites.add(key);
  }
  persistFavorites();
  refreshPresetList();
  updateFavoriteControls();
});

presetSearch?.addEventListener('input', () => {
  refreshPresetList();
});
playlistNameInput?.addEventListener('input', () => {
  refreshPlaylistControls();
});

automation.addEventListener('change', () => {
  engine?.setPresetAutomation({
    beatsPerPreset: Number(beats.value) || 8,
    mode: automation.value,
    timedIntervalSeconds: Number(interval.value) || 30,
  });
});

document.querySelector('#preset-files').addEventListener('change', async (event) => {
  const files = Array.from(event.target.files || []);
  const added = [];
  for (const file of files) {
    const source = await file.text();
    added.push({
      id: crypto.randomUUID(),
      name: file.name,
      packId: 'builtin',
      source,
    });
  }
  presets = [...presets, ...added];
  activeIndex = Math.max(0, presets.length - files.length);
  packList.value = 'builtin';
  setActivePack('builtin');
  refreshPresetList();
  loadActivePreset();
});

document.querySelector('#texture-files').addEventListener('change', async (event) => {
  const files = Array.from(event.target.files || []);
  const entries = await Promise.all(files.map((file) => new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve([file.name, String(reader.result || '')]);
    reader.readAsDataURL(file);
  })));
  textureAssets = Object.fromEntries(entries);
  loadActivePreset();
});

canvas.addEventListener('pointermove', (event) => {
  const rect = canvas.getBoundingClientRect();
  engine?.setMouseState({
    mouse_x: (event.clientX - rect.left) / rect.width,
    mouse_y: (event.clientY - rect.top) / rect.height,
    mouse_dx: event.movementX / Math.max(1, rect.width),
    mouse_dy: event.movementY / Math.max(1, rect.height),
  });
});

canvas.addEventListener('pointerdown', () => engine?.setMouseState({ mouse_down: 1 }));
canvas.addEventListener('pointerup', () => engine?.setMouseState({ mouse_down: 0 }));
window.addEventListener('resize', resize);
refreshPackList();
refreshPlaylistControls();
refreshPresetList();
recordHistory(activeIndex);
updateFavoriteControls();
loadCommunityPackChoices();
