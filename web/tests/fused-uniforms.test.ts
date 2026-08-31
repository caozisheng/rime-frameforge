import { describe, expect, it } from 'vitest';

import { normalGraphQuantization } from '../src/generated/normal_quantization.generated.js';
import { FUSED_UNIFORM_BYTES, packFusedUniforms } from '../src/gpu/fused-uniforms.js';
import type { RawFrameDescriptor } from '../src/contracts.js';

const descriptor: RawFrameDescriptor = {
  width: 32,
  height: 24,
  rowStrideSamples: 32,
  storageBits: 16,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 4095,
};

describe('fused uniform ABI', () => {
  it('packs graph and six quantization blocks into the fixed layout', () => {
    const bytes = packFusedUniforms(descriptor, 7, { vng_threshold: 1.5, ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }, normalGraphQuantization);
    const view = new DataView(bytes);

    expect(bytes.byteLength).toBe(FUSED_UNIFORM_BYTES);
    expect([view.getUint32(0, true), view.getUint32(4, true), view.getFloat32(8, true), view.getFloat32(12, true)]).toEqual([32, 24, 64, 4095]);
    expect([view.getUint32(16, true), view.getUint32(20, true), view.getUint32(24, true), view.getUint32(28, true)]).toEqual([0, 1, 1, 2]);
    expect(view.getUint32(44, true)).toBe(7);
    expect(view.getFloat32(48, true)).toBe(2 ** 14);
    expect([view.getUint32(432, true), view.getUint32(436, true), view.getUint32(440, true), view.getUint32(444, true), view.getUint32(448, true), view.getUint32(452, true)]).toEqual([1, 1, 1, 1, 1, 1]);
  });

  it('clears inline quantization flags when the graph is disabled', () => {
    const bytes = packFusedUniforms(descriptor, 0, { vng_threshold: 1.5, ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }, { ...normalGraphQuantization, enabled: false });
    const view = new DataView(bytes);

    expect([view.getUint32(432, true), view.getUint32(436, true), view.getUint32(440, true), view.getUint32(444, true), view.getUint32(448, true), view.getUint32(452, true)]).toEqual([0, 0, 0, 0, 0, 0]);
  });
});
