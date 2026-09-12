import { beforeEach, describe, expect, it } from 'vitest';

import type { FramePacketProvider, RawFrameDescriptor } from '../src/contracts.js';
import type { GpuContext } from '../src/gpu/device.js';
import { defaultGraphBypassConfig } from '../src/gpu/bypass.js';
import { NormalGpuExecutor } from '../src/gpu/executor.js';

const descriptor: RawFrameDescriptor = {
  width: 2, height: 2, rowStrideSamples: 2, storageBits: 16,
  cfa: 'rggb', blackLevel: 64, whiteLevel: 4095, whiteBalanceGains: [2, 1, 1.5],
  metadata: { colorMatrix1: [1, 0, 0, 0, 1, 0, 0, 0, 1] },
};
const packetProvider: FramePacketProvider = () => ({
  blcUniform: new Uint8Array(16),
  wbcUniform: new Uint8Array(48),
  drcUniform: new Uint8Array(32),
  demUniform: new Uint8Array(32),
  drcGlobalLut: new Uint8Array(1028),
  drcLocalLut: new Uint8Array(),
  drcModulationLuts: new Uint8Array(512),
  fusedUniform: new Uint8Array(1024),
  colorReproduceHsLut: new Uint8Array(),
});

function fusedGpu() {
  const counts = { computePasses: 0, renderPasses: 0, submits: 0, waits: 0, dispatches: 0, draws: 0, scissors: [] as number[], sampleCopies: 0 };
  const computePass = {
    setPipeline: () => undefined,
    setBindGroup: () => undefined,
    dispatchWorkgroups: () => { counts.dispatches += 1; },
    end: () => undefined,
  };
  const renderPass = {
    setPipeline: () => undefined,
    setBindGroup: () => undefined,
    draw: () => { counts.draws += 1; },
    setScissorRect: (x: number) => { counts.scissors.push(x); },
    end: () => undefined,
  };
  const texture = { width: 2, height: 2, createView: () => ({}), destroy: () => undefined } as unknown as GPUTexture;
  const device = {
    queue: {
      writeBuffer: () => undefined,
      writeTexture: () => undefined,
      submit: () => { counts.submits += 1; },
      onSubmittedWorkDone: async () => { counts.waits += 1; },
    },
    createTexture: () => texture,
    createBuffer: () => ({ destroy: () => undefined, mapAsync: async () => undefined, getMappedRange: () => new Uint16Array([0x2e66, 0x3266, 0x34cd, 0x3666]).buffer, unmap: () => undefined }),
    createShaderModule: () => ({}),
    createComputePipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createBindGroup: () => ({}),
    createCommandEncoder: () => ({
      beginComputePass: () => { counts.computePasses += 1; return computePass; },
      beginRenderPass: () => { counts.renderPasses += 1; return renderPass; },
      finish: () => ({}),
      copyTextureToBuffer: () => { counts.sampleCopies += 1; },
    }),
  } as unknown as GPUDevice;
  const context = { getCurrentTexture: () => texture } as unknown as GPUCanvasContext;
  return { gpu: { canvas: { width: 2, height: 2 } as OffscreenCanvas, device, context, canvasFormat: 'bgra8unorm' } satisfies GpuContext, counts };
}

beforeEach(() => {
  Object.assign(globalThis, {
    GPUTextureUsage: { COPY_SRC: 1, COPY_DST: 2, TEXTURE_BINDING: 4, STORAGE_BINDING: 8 },
    GPUBufferUsage: { UNIFORM: 64, STORAGE: 128, COPY_DST: 8, MAP_READ: 1 },
    GPUMapMode: { READ: 1 },
  });
});

describe('fused Normal GPU executor', () => {
  it('encodes compute and preview into one submission and one frame fence', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };

    executor.prepare(identity);
    await executor.execute('output', identity);
    expect(fake.counts).toMatchObject({ computePasses: 15, renderPasses: 1, submits: 1, waits: 1, dispatches: 15, draws: 1, scissors: [], sampleCopies: 0 });
  });

  it('encodes bounded complex DEM segments in one submission and one frame fence', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 2, gpuGeneration: 1 };
    executor.setMethod('dem', '02');

    executor.prepare(identity);
    await executor.execute('output', identity);
    expect(fake.counts).toMatchObject({ computePasses: 18, renderPasses: 1, submits: 1, waits: 1, dispatches: 18, draws: 1, scissors: [], sampleCopies: 0 });
  });

  it('rebinds two committed outputs for Compare without recomputing the graph', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };
    executor.prepare(identity);
    await executor.execute('output', identity);

    await executor.present('blc', 'dem', 0.4);
    expect(fake.counts.computePasses).toBe(15);
    expect(fake.counts.renderPasses).toBe(2);
    expect(fake.counts.draws).toBe(3);
    expect(fake.counts.scissors).toEqual([1]);
  });

  it('reads one native GPU sample without copying an image', async () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };
    executor.prepare(identity);
    await executor.execute('output', identity);

    const values = await executor.sample('dem', 1, 1);

    expect(values).toHaveLength(4);
    values.forEach((value, index) => expect(value).toBeCloseTo([0.1, 0.2, 0.3, 0.4][index] ?? 0));
    expect(fake.counts.sampleCopies).toBe(1);
  });
  it('skips DRC compute work and retains a presentable DRC preview alias', async () => {
    const normal = fusedGpu();
    const normalExecutor = new NormalGpuExecutor(normal.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const bypassed = fusedGpu();
    const executor = new NormalGpuExecutor(bypassed.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    const identity = { frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 };
    const bypassConfig = defaultGraphBypassConfig();

    normalExecutor.prepare(identity);
    await normalExecutor.execute('output', identity);
    executor.prepare(identity);
    await executor.execute('output', identity);
    executor.setBypassConfig({
      ...bypassConfig,
      modules: bypassConfig.modules.map((module) => module.module_id === 'drc' ? { ...module, bypass: true } : module),
    });
    executor.prepare(identity);
    await executor.execute('output', identity);
    await executor.present('drc', null, 0.5);
    expect(normal.counts.computePasses).toBe(15);
    expect(bypassed.counts.computePasses).toBe(18);
    expect(bypassed.counts.computePasses - normal.counts.computePasses).toBe(3);
    expect(bypassed.counts).toMatchObject({ dispatches: 18, renderPasses: 3, submits: 3, waits: 3, draws: 3 });
  });
});

  it('rejects parameters submitted to the wrong graph node', () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    expect(() => executor.setParameter('gamma', 'ahd_l_threshold', 3)).toThrow('PARAMETER_INVALID');
    expect(() => executor.setParameter('dem', 'gamma', 2.4)).toThrow('PARAMETER_INVALID');
    expect(() => executor.setParameter('gamma', 'gamma', 2.4)).not.toThrow();
  });
  it('accepts valid DRC IQ parameters and rejects invalid values', () => {
    const fake = fusedGpu();
    const executor = new NormalGpuExecutor(fake.gpu, new Uint16Array([1, 2, 3, 4]).buffer, 0, 1, descriptor, packetProvider);
    expect(() => executor.setDrcIqParameters({ drc_gain_offset_ev: 0.5, knee: 1.2, amplifier: 0.8 })).not.toThrow();
    expect(() => executor.setDrcIqParameters({ drc_gain_offset_ev: 5, knee: 1, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
  });
