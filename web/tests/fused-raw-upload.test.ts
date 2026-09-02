import { beforeEach, describe, expect, it } from 'vitest';

import type { RawFrameDescriptor } from '../src/contracts.js';
import { normalGraphQuantization } from '../src/generated/normal_quantization.generated.js';
import type { GpuContext } from '../src/gpu/device.js';
import { NormalGpuExecutor } from '../src/gpu/executor.js';

const descriptor: RawFrameDescriptor = {
  width: 2,
  height: 2,
  rowStrideSamples: 2,
  storageBits: 16,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 4095,
};

function fusedGpu() {
  const writes: Array<{ bytesPerRow: number; rowsPerImage: number }> = [];
  const texture = { createView: () => ({}), destroy: () => undefined } as unknown as GPUTexture;
  const device = {
    limits: { maxTextureDimension2D: 8192 },
    queue: {
      writeBuffer: () => undefined,
      writeTexture: (_destination: unknown, _data: unknown, layout: { bytesPerRow: number; rowsPerImage: number }) => writes.push(layout),
    },
    createTexture: () => texture,
    createBuffer: () => ({ destroy: () => undefined }),
    createShaderModule: () => ({}),
    createComputePipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
  } as unknown as GPUDevice;
  const context = {} as GPUCanvasContext;
  return { gpu: { canvas: { width: 2, height: 2 } as OffscreenCanvas, device, context, canvasFormat: 'bgra8unorm' } satisfies GpuContext, writes };
}

beforeEach(() => {
  Object.assign(globalThis, {
    GPUTextureUsage: { COPY_SRC: 1, COPY_DST: 2, TEXTURE_BINDING: 4, STORAGE_BINDING: 8 },
    GPUBufferUsage: { UNIFORM: 64, COPY_DST: 8, MAP_READ: 1 },
    GPUMapMode: { READ: 1 },
  });
});

describe('fused executor RAW upload layout', () => {
  it('uses padded source row stride when uploading RAW', () => {
    const fake = fusedGpu();
    const padded = { ...descriptor, rowStrideSamples: 4 };
    new NormalGpuExecutor(fake.gpu, new Uint16Array(8).buffer, 0, 1, padded, normalGraphQuantization);

    expect(fake.writes[0]).toEqual({ bytesPerRow: 8, rowsPerImage: 2 });
  });
});
