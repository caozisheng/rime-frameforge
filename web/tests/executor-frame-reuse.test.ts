import { beforeEach, describe, expect, it } from 'vitest';

import type { RawFrameDescriptor } from '../src/contracts.js';
import { NormalGpuExecutor } from '../src/gpu/executor.js';
import type { GpuContext } from '../src/gpu/device.js';
import { normalGraphQuantization } from '../src/generated/normal_quantization.generated.js';

const descriptor: RawFrameDescriptor = {
  width: 2,
  height: 2,
  rowStrideSamples: 2,
  storageBits: 16,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 4095,
  whiteBalanceGains: [2, 1, 1.5],
};

function raw(samples: readonly number[]): ArrayBuffer {
  return new Uint16Array(samples).buffer;
}

function fakeGpu() {
  let textureCreates = 0;
  let textureDestroys = 0;
  let bufferCreates = 0;
  let bufferDestroys = 0;
  let rawUploads = 0;
  const device = {
    limits: { maxTextureDimension2D: 8192 },
    queue: {
      writeBuffer: () => undefined,
      writeTexture: () => { rawUploads += 1; },
      submit: () => undefined,
      onSubmittedWorkDone: async () => undefined,
    },
    createTexture: () => {
      textureCreates += 1;
      return { createView: () => ({}), destroy: () => { textureDestroys += 1; } };
    },
    createBuffer: () => {
      bufferCreates += 1;
      return { destroy: () => { bufferDestroys += 1; } };
    },
    createBindGroup: () => ({}),
    createCommandEncoder: () => ({
      beginComputePass: () => ({ setPipeline: () => undefined, setBindGroup: () => undefined, dispatchWorkgroups: () => undefined, end: () => undefined }),
      beginRenderPass: () => ({ setPipeline: () => undefined, setBindGroup: () => undefined, draw: () => undefined, end: () => undefined }),
      finish: () => ({}),
    }),
    createShaderModule: () => ({}),
    createComputePipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
  } as unknown as GPUDevice;
  const context = { getCurrentTexture: () => ({ width: 2, height: 2, createView: () => ({}) }) } as unknown as GPUCanvasContext;
  const gpu = { canvas: { width: 2, height: 2 } as OffscreenCanvas, device, context, canvasFormat: 'bgra8unorm' } satisfies GpuContext;
  return {
    gpu,
    counts: () => ({ textureCreates, textureDestroys, bufferCreates, bufferDestroys, rawUploads }),
  };
}

beforeEach(() => {
  Object.assign(globalThis, {
    GPUTextureUsage: { COPY_SRC: 1, COPY_DST: 2, TEXTURE_BINDING: 4, STORAGE_BINDING: 8 },
    GPUBufferUsage: { UNIFORM: 64, COPY_DST: 8, MAP_READ: 1 },
    GPUMapMode: { READ: 1 },
  });
});

describe('NormalGpuExecutor frame reuse', () => {
  it('uploads a same-extent frame without reallocating GPU resources', () => {
    const fake = fakeGpu();
    const executor = new NormalGpuExecutor(fake.gpu, raw([1, 2, 3, 4]), 0, 1, descriptor, normalGraphQuantization);

    executor.replaceFrame(raw([5, 6, 7, 8]), 0, descriptor);

    expect(fake.counts()).toEqual({ textureCreates: 8, textureDestroys: 0, bufferCreates: 1, bufferDestroys: 0, rawUploads: 2 });
  });
  it('reuses the uploaded raw source across reset and repeated graph execution', async () => {
    const fake = fakeGpu();
    const executor = new NormalGpuExecutor(fake.gpu, raw([1, 2, 3, 4]), 0, 1, descriptor, normalGraphQuantization);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };

    executor.prepare(identity);
    await executor.execute('output', identity);
    executor.reset();
    executor.setParameter('dem', 'ahd_l_threshold', 3.0);
    const secondIdentity = { ...identity, runRevision: 2, methodRevision: 2 };
    executor.prepare(secondIdentity);
    await executor.execute('output', secondIdentity);

    expect(fake.counts().rawUploads).toBe(1);
  });

  it('requires resource rebuild when frame extent changes', () => {
    const fake = fakeGpu();
    const executor = new NormalGpuExecutor(fake.gpu, raw([1, 2, 3, 4]), 0, 1, descriptor, normalGraphQuantization);

    expect(executor.canReplaceFrame({ ...descriptor, width: 4, rowStrideSamples: 4 })).toBe(false);
  });

  it('destroys frame-owned GPU resources on disposal', () => {
    const fake = fakeGpu();
    const executor = new NormalGpuExecutor(fake.gpu, raw([1, 2, 3, 4]), 0, 1, descriptor, normalGraphQuantization);

    executor.dispose();

    expect(fake.counts()).toEqual({ textureCreates: 8, textureDestroys: 8, bufferCreates: 1, bufferDestroys: 1, rawUploads: 1 });
  });
});
