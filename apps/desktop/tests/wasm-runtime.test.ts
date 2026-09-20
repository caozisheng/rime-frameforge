import { describe, expect, it, vi } from 'vitest';

import type { RawFrameDescriptor } from '../../../web/src/contracts.js';

const mock = vi.hoisted(() => {
  const state = {
    beginArgs: undefined as unknown[] | undefined,
    prepareArgs: undefined as unknown[] | undefined,
    stagedPayload: undefined as Uint8Array | undefined,
    completeCalls: 0,
    completeThrows: false,
    abortCalls: 0,
    resetCalls: 0,
    beginFreed: 0,
    consumersFreed: 0,
  };

  class BeginPackets {
    blc_uniform() {
      return new Uint8Array([1, 2]);
    }
    lcst_uniform() {
      return new Uint8Array([3, 4]);
    }
    free() {
      state.beginFreed += 1;
    }
  }

  class ConsumerPackets {
    private bytes() {
      return new Uint8Array([5]);
    }
    tintless_uniform() { return this.bytes(); }
    tintless_mesh() { return this.bytes(); }
    tintless_audit() { return this.bytes(); }
    lsc_uniform() { return this.bytes(); }
    lsc_mesh_headers() { return this.bytes(); }
    lsc_mesh_entries() { return this.bytes(); }
    lsc_active() { return true; }
    wbc_uniform() { return this.bytes(); }
    drc_uniform() { return this.bytes(); }
    dem_uniform() { return this.bytes(); }
    drc_global_lut() { return this.bytes(); }
    drc_local_lut() { return this.bytes(); }
    drc_modulation_luts() { return this.bytes(); }
    fused_uniform() { return this.bytes(); }
    color_reproduce_hs_lut() { return this.bytes(); }
    preprocess_snapshot_json() { return '{"frameIndex":7,"modules":{}}'; }
    free() { state.consumersFreed += 1; }
  }

  class FramePacketDeriver {
    begin_frame(...args: unknown[]) {
      state.beginArgs = args;
      return new BeginPackets();
    }
    prepare_consumers(...args: unknown[]) {
      state.prepareArgs = args;
      return new ConsumerPackets();
    }
    stage_lcst_statistics(payload: Uint8Array) {
      state.stagedPayload = payload;
    }
    complete_frame() {
      state.completeCalls += 1;
      if (state.completeThrows) throw new Error('complete failed');
    }
    abort_frame() {
      state.abortCalls += 1;
    }
    reset() {
      state.resetCalls += 1;
    }
    free() {}
  }

  class NormalRuntime {
    quantization_config_json() { return '{}'; }
    reset() {
      return JSON.stringify({
        lifecycle_state: 'stop',
        run_revision: 0,
        config_revision: 0,
        method_revision: 0,
        frame_index: 0,
        frame_phase: null,
        visible_frame: null,
        gpu_generation: 0,
      });
    }
    device_lost() {
      return JSON.stringify({
        lifecycle_state: 'error',
        run_revision: 0,
        config_revision: 0,
        method_revision: 0,
        frame_index: null,
        frame_phase: null,
        visible_frame: null,
        gpu_generation: 0,
      });
    }
    free() {}
  }

  return { state, FramePacketDeriver, NormalRuntime };
});

vi.mock('../../../crates/rime-wasm/pkg/rime_wasm.js', () => ({
  default: vi.fn(async () => undefined),
  FramePacketDeriver: mock.FramePacketDeriver,
  NormalRuntime: mock.NormalRuntime,
}));

import { WasmRuntimeAuthority } from '../src/runtime/wasm-runtime.js';

const descriptor = {
  width: 2,
  height: 1,
  rowStrideSamples: 2,
  storageBits: 12,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 4095,
  whiteBalanceGains: [1, 1, 1],
  metadata: {},
} as RawFrameDescriptor;

const identity = { frameIndex: 7, runRevision: 11, methodRevision: 13 } as const;

function beginArgs() {
  return [descriptor, new Uint16Array([10, 20]).buffer, 0, identity, 'single' as const, {}, {}, [], {
    drc_gain_offset_ev: 0,
    knee: 1,
    amplifier: 1,
  }, [] as const] as const;
}

describe('WASM runtime staged frame authority', () => {
  it('transports identity and mode, copies begin packets, and keeps transaction pending', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass] = beginArgs();

    const begin = authority.beginFrame(frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass);

    expect(mock.state.beginArgs?.slice(0, 7)).toEqual([
      JSON.stringify(descriptor),
      expect.any(Uint16Array),
      expect.any(String),
      BigInt(identity.frameIndex),
      BigInt(identity.runRevision),
      BigInt(identity.methodRevision),
      'single',
    ]);
    expect(begin.blcUniform).toEqual(new Uint8Array([1, 2]));
    expect(begin.lcstUniform).toEqual(new Uint8Array([3, 4]));
    expect(mock.state.beginFreed).toBe(1);

    const consumers = authority.prepareConsumers(undefined, true);
    expect(consumers.preprocessSnapshotJson).toContain('frameIndex');
    expect(consumers.tintlessUniform).toEqual(new Uint8Array([5]));
    expect(mock.state.consumersFreed).toBe(1);

    const payload = new Uint8Array([9, 8]);
    authority.stageLcstStatistics(payload);
    expect(mock.state.stagedPayload).toBe(payload);
    authority.completeFrame();
    expect(mock.state.completeCalls).toBe(1);

    // A completed transaction cannot be staged again.
    expect(() => authority.stageLcstStatistics(payload)).toThrow('WASM_FRAME_NOT_PREPARED');
  });

  it('preserves unsigned 64-bit identities at the WASM boundary', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [frameDescriptor, raw, offset, , mode, methods, parameters, gamma, drc, bypass] = beginArgs();
    const identity64 = {
      frameIndex: 0xffff_ffff_ffff_fffdn,
      runRevision: 0xffff_ffff_ffff_ffen,
      methodRevision: 0xffff_ffff_ffff_ffffn,
    } as const;

    authority.beginFrame(frameDescriptor, raw, offset, identity64, mode, methods, parameters, gamma, drc, bypass);

    expect(mock.state.beginArgs?.slice(3, 6)).toEqual([
      identity64.frameIndex,
      identity64.runRevision,
      identity64.methodRevision,
    ]);
    authority.abortFrame();
  });

  it('keeps the transaction pending when completion fails so it can be aborted', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass] = beginArgs();
    authority.beginFrame(descriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass);
    mock.state.completeThrows = true;

    expect(() => authority.completeFrame()).toThrow('complete failed');
    mock.state.completeThrows = false;
    authority.abortFrame();
    expect(mock.state.abortCalls).toBeGreaterThan(0);
  });

  it('passes opaque LCST payloads and preserves the pending transaction through abort', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [, raw, offset, frameIdentity, , methods, parameters, gamma, drc, bypass] = beginArgs();
    authority.beginFrame(descriptor, raw, offset, frameIdentity, 'sequence', methods, parameters, gamma, drc, bypass);

    const payload = new Uint8Array([0, 255, 1]);
    authority.prepareConsumers(payload, false);
    expect(mock.state.prepareArgs).toEqual([payload, false]);
    authority.abortFrame();
    expect(mock.state.abortCalls).toBeGreaterThan(0);
    expect(() => authority.prepareConsumers(undefined, false)).toThrow('WASM_FRAME_NOT_PREPARED');
  });

  it('clears the staged transaction when the runtime resets', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass] = beginArgs();
    authority.beginFrame(frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass);

    authority.reset();

    expect(mock.state.resetCalls).toBe(1);
    expect(() => authority.prepareConsumers(undefined, false)).toThrow('WASM_FRAME_NOT_PREPARED');
  });

  it('clears the staged transaction when the device is lost', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const [frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass] = beginArgs();
    authority.beginFrame(frameDescriptor, raw, offset, frameIdentity, mode, methods, parameters, gamma, drc, bypass);
    const resetsBefore = mock.state.resetCalls;

    authority.deviceLost();

    expect(mock.state.resetCalls).toBe(resetsBefore + 1);
    expect(() => authority.prepareConsumers(undefined, false)).toThrow('WASM_FRAME_NOT_PREPARED');
  });

  it('resets the deriver before disposal', async () => {
    const authority = await WasmRuntimeAuthority.create();
    const resetsBefore = mock.state.resetCalls;

    authority.dispose();

    expect(mock.state.resetCalls).toBe(resetsBefore + 1);
  });
 });
