import { beforeEach, describe, expect, it } from 'vitest';

import type { RawFrameDescriptor } from '../src/contracts.js';
import { normalGraphQuantization } from '../src/generated/normal_quantization.generated.js';
import type { GpuContext } from '../src/gpu/device.js';
import { NormalGpuExecutor } from '../src/gpu/executor.js';

const descriptor: RawFrameDescriptor = {
  width: 2, height: 2, rowStrideSamples: 2, storageBits: 16,
  cfa: 'rggb', blackLevel: 64, whiteLevel: 4095,
};

function fusedGpu() {
  const counts = { computePasses: 0, renderPasses: 0, submits: 0, waits: 0, dispatches: 0 };
  const computePass = {
    setPipeline: () => undefined,
    setBindGroup: () => undefined,
    dispatchWorkgroups: () => { counts.dispatches += 1; },
    end: () => undefined,
  };
  const renderPass = {
    setPipeline: () => undefined,
    setBindGroup: () => undefined,
    draw: () => undefined,
    end: () => undefined,
  };
  const texture = { createView: () => ({}), destroy: () => undefined } as unknown as GPUTexture;
  const device = {
    queue: {
      writeBuffer: () => undefined,
      writeTexture: () => undefined,
      submit: () => { counts.submits += 1; },
      onSubmittedWorkDone: async () => { counts.waits += 1; },
    },
    createTexture: () => texture,
    createBuffer: () => ({ destroy: () => undefined }),
    createShaderModule: () => ({}),
    createComputePipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createBindGroup: () => ({}),
    createCommandEncoder: () => ({
      beginComputePass: () => { counts.computePasses += 1; return computePass; },
      beginRenderPass: () => { counts.renderPasses += 1; return renderPass; },
      finish: () => ({}),
    }),
  } as unknown as GPUDevice;
  const context = { getCurrentTexture: () => texture } as unknown as GPUCanvasContext;
  return { gpu: { device, context, canvasFormat: 'bgra8unorm' } satisfies GpuContext, counts };
}

beforeEach(() => {
  Object.assign(globalThis, {
    GPUTextureUsage: { COPY_DST: 2, TEXTURE_BINDING: 4, STORAGE_BINDING: 8 },
    GPUBufferUsage: { UNIFORM: 64, COPY_DST: 8 },
  });
});

describe('fused Normal GPU executor', () => {
  it('encodes compute and preview into one submission and one frame fence', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, normalGraphQuantization);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };

    executor.prepare(identity);
    await executor.execute('output', identity);

    expect(fake.counts).toEqual({ computePasses: 1, renderPasses: 1, submits: 1, waits: 1, dispatches: 1 });
  });

  it('encodes bounded complex DEM segments in one submission and one frame fence', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, normalGraphQuantization);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 2, gpuGeneration: 1 };
    executor.setMethod('dem', '02');

    executor.prepare(identity);
    await executor.execute('output', identity);

    expect(fake.counts).toEqual({ computePasses: 3, renderPasses: 1, submits: 1, waits: 1, dispatches: 3 });
  });
});
