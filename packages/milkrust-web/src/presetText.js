// Keep the byte-decoding policy aligned with milkrust-core/src/text.rs.
const LIMIT = 2 * 1024 * 1024;
export const decodeMilkRustPresetBytes = (input) => {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (!bytes.length || bytes.length > LIMIT) throw new Error('Preset is empty or exceeds 2 MiB');
  let source;
  if (bytes[0] === 0xff && bytes[1] === 0xfe || bytes[0] === 0xfe && bytes[1] === 0xff) {
    if (bytes.length % 2) throw new Error('Truncated UTF-16 preset');
    source = new TextDecoder(bytes[0] === 0xff ? 'utf-16le' : 'utf-16be', { fatal: true }).decode(bytes);
  } else {
    try { source = new TextDecoder('utf-8', { fatal: true }).decode(bytes); }
    catch {
      if (bytes.some((byte) => [0x81, 0x8d, 0x8f, 0x90, 0x9d].includes(byte))) {
        throw new Error('Undefined Windows-1252 byte in preset');
      }
      source = new TextDecoder('windows-1252', { fatal: true }).decode(bytes);
    }
  }
  if (!source.length || /[\u0000-\u0008\u000e-\u001f\u007f-\u009f]/u.test(source)
      || new TextEncoder().encode(source).length > LIMIT) {
    throw new Error('Decoded preset is empty, binary or exceeds 2 MiB');
  }
  return source;
};

export const readMilkRustPresetResponse = async (response) => {
  const length = Number(response.headers?.get?.('content-length'));
  if (Number.isFinite(length) && length > LIMIT) throw new Error('Preset exceeds 2 MiB');
  if (response.body?.getReader) {
    const reader = response.body.getReader();
    const chunks = []; let size = 0;
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        size += value.byteLength;
        if (size > LIMIT) throw new Error('Preset exceeds 2 MiB');
        chunks.push(value);
      }
    } catch (error) { try { await reader.cancel(error); } catch {} throw error; }
    finally { reader.releaseLock(); }
    const bytes = new Uint8Array(size); let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    return decodeMilkRustPresetBytes(bytes);
  }
  if (response.arrayBuffer) return decodeMilkRustPresetBytes(await response.arrayBuffer());
  // Compatibility for injected fetch adapters that expose already decoded text.
  return decodeMilkRustPresetBytes(new TextEncoder().encode(await response.text()));
};
