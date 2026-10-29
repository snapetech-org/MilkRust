import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { decodeMilkRustPresetBytes, readMilkRustPresetResponse } from './presetText.js';

describe('legacy preset text policy', () => {
  it('decodes names without replacement characters', () => {
    assert.equal(decodeMilkRustPresetBytes(Uint8Array.of(65, 0xe9, 0x96)), 'Aé–');
    assert.equal(decodeMilkRustPresetBytes(Uint8Array.of(0xff, 0xfe, 65, 0, 0x3c, 0xd8, 0x0c, 0xdf)), 'A🌌');
    assert.equal(decodeMilkRustPresetBytes(Uint8Array.of(0xfe, 0xff, 0, 65)), 'A');
    assert.equal(decodeMilkRustPresetBytes(Uint8Array.of(0xef, 0xbb, 0xbf, 65)), 'A');
  });
  it('rejects binary and malformed encodings', () => {
    for (const bytes of [[0x81], [65, 0, 66], [0xff, 0xfe, 0, 0xd8], [0xff, 0xfe, 65]]) {
      assert.throws(() => decodeMilkRustPresetBytes(Uint8Array.from(bytes)));
    }
  });
  it('cancels a stream before buffering beyond the encoded budget', async () => {
    let cancelled = false;
    const response = { body: new ReadableStream({
      pull(controller) { controller.enqueue(new Uint8Array(1024 * 1024)); },
      cancel() { cancelled = true; },
    }) };
    await assert.rejects(readMilkRustPresetResponse(response), /exceeds/);
    assert.equal(cancelled, true);
  });
});
