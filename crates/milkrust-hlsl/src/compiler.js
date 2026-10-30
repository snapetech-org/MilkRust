let compiler;
let initialization;
let initializedUrl;

export async function initializeHlsl(url) {
  if (initialization) {
    if (initializedUrl !== url) throw new Error('HLSL compiler is already bound to another URL');
    return initialization;
  }
  initializedUrl = url;
  initialization = (async () => {
    const { default: createCompiler } = await import(/* @vite-ignore */ url);
    compiler = await createCompiler({ locateFile: (file) => new URL(file, url).href });
  })();
  try { await initialization; } catch (error) {
    initialization = undefined;
    initializedUrl = undefined;
    throw error;
  }
}

export function compileHlsl(source, entry) {
  if (!compiler) throw new Error('HLSL compiler not initialized');
  const encoder = new TextEncoder();
  const input = encoder.encode(source);
  const name = encoder.encode(entry);
  if (input.length > 262144 || name.length > 64) throw new Error('HLSL source limit exceeded');
  const allocations = [];
  const allocate = (size) => {
    const pointer = compiler._malloc(size);
    if (!pointer) throw new Error('HLSL compiler memory limit exceeded');
    allocations.push(pointer);
    return pointer;
  };
  try {
    const src = allocate(input.length + 1), ep = allocate(name.length + 1);
    const out = allocate(1048577), err = allocate(8192);
    // Allocations may grow/rebind the WASM heap; access it only afterwards.
    compiler.HEAPU8.set(input, src); compiler.HEAPU8[src + input.length] = 0;
    compiler.HEAPU8.set(name, ep); compiler.HEAPU8[ep + name.length] = 0;
    const length = compiler._milkrust_hlsl_compile(src, input.length, ep, out, 1048577, err, 8192);
    if (length < 0) throw new Error(compiler.UTF8ToString(err, 8192) || 'HLSL compilation failed');
    if (length > 1048576) throw new Error('HLSL compiler output limit exceeded');
    return new TextDecoder('utf-8', { fatal: true }).decode(compiler.HEAPU8.subarray(out, out + length));
  } finally {
    allocations.reverse().forEach((pointer) => compiler._free(pointer));
  }
}
