# Plugin API (JavaScript SDK)

`@milkrust/web` exposes a lightweight plugin surface in the browser SDK so hosts can add behavior without forking the engine.

## Plugin model

`loadMilkRustPackPlugins(manifest, { fetchImpl })` loads plugin entries from a normalized
pack manifest. It returns:

- `valid`: whether every plugin loaded successfully
- `errors`: hard errors
- `warnings`: non-blocking issues
- `plugins`: loaded plugin descriptors

Each plugin descriptor can come from:

- **data plugin** (`kind: "data"`): JSON payload loaded from `plugin.url`
- **js plugin** (`kind: "js"`): rejected by pack loading; downloaded preset packs cannot authorize code execution

`kind` is normalized to accept `js`, `javascript`, and `module`.

Data plugins can carry config, deck wiring metadata, presets, and heuristics.
An executable entry makes the plugin and aggregate pack results `valid: false`
with an `errors` entry. It is skipped without fetching/importing its code; other
data entries may still load.

## Plugin hooks

Install plugins through `engine.loadPlugins(plugins)` where `plugins` is the loaded array.

Supported hooks:

- `onPresetLoad(context)`
- `onPresetLoaded(context)`
- `onPresetChange(context)`
- `onFrameStart(context)`
- `onAudioFrame(context)`
- `onBeat(context)`
- `onAutomationStep(context)`
- `onRenderFrame(context)`
- `onInput(context)`
- `onExport(context)`

Hooks receive a mutable context object and may return a plain object to merge additional context.

## Minimal data plugin example

```json
{
  "kind": "playlist",
  "presetIds": ["warm-scope", "blue-shape"],
  "shuffle": false
}
```

```js
const packed = await loadMilkRustPack('examples/sample-pack/');
const plugins = await loadMilkRustPackPlugins(packed);
engine.loadPlugins(plugins.plugins);
```

## Minimal JS plugin example

```js
export const onFrameStart = ({ automation, presetName }) => {
  if (automation?.mode === 'beat') {
    // plugin logic ...
  }
  return { presetName };
};
```

```js
export default { onFrameStart };
```

## Current constraints

- Hooks are best-effort and run with error isolation (exceptions are logged and ignored).
- Dynamic js plugins are loaded from user-accessible URLs and are not sandboxed.
- Plugin execution does not alter the Rust runtime contract; it can only mutate
  SDK-facing context and local orchestration decisions in the web layer.

Pack loading only accepts declarative data entries. `loadMilkRustPackPlugins` reports an error for executable entries and never imports their URL. An application may still supply its own trusted, already-loaded hook objects to `engine.loadPlugins`; this is separate from loading community content.
