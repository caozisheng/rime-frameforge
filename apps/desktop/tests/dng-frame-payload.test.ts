import { describe, expect, it } from 'vitest';

import { decodeDngFramePayload } from '../src/runtime/dng-frame-payload.js';

function payload(descriptor: object, raw: readonly number[]): ArrayBuffer {
  const json = new TextEncoder().encode(JSON.stringify(descriptor));
  const rawOffset = 4 + json.length + (json.length & 1);
  const buffer = new ArrayBuffer(rawOffset + raw.length * 2);
  new DataView(buffer).setUint32(0, json.length, true);
  new Uint8Array(buffer, 4, json.length).set(json);
  raw.forEach((sample, index) => new DataView(buffer).setUint16(rawOffset + index * 2, sample, true));
  return buffer;
}

describe('DNG frame payload', () => {
  it('returns descriptor and aligned RAW view without copying the payload buffer', () => {
    const buffer = payload({ frameIndex: 3, fileName: 'frame3.dng' }, [64, 1024]);

    const decoded = decodeDngFramePayload(buffer);

    expect(decoded.descriptor).toMatchObject({ frameIndex: 3, fileName: 'frame3.dng' });
    expect(decoded.rawByteOffset % 2).toBe(0);
    expect(decoded.payload).toBe(buffer);
    expect([...new Uint16Array(decoded.payload, decoded.rawByteOffset)]).toEqual([64, 1024]);
  });
});
