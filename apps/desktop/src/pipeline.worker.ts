import { createGpuContext, type GpuContext } from '../../../web/src/gpu/device.js';
import { validateGpuInput } from '../../../web/src/gpu/capability.js';
import { NormalGpuExecutor } from '../../../web/src/gpu/executor.js';
import { RuntimeController } from '../../../web/src/runtime-controller.js';
import { SerialCommandQueue } from '../../../web/src/serial-command-queue.js';
import { DEFAULT_DRC_IQ_PARAMETERS, validateDrcIqParameters } from '../../../web/src/gpu/drc.js';
import { DEFAULT_GAMMA_PARAMETERS } from '../../../web/src/gpu/gamma.js';
import type { DrcIqParameters, PreprocessSnapshot, RawFrameDescriptor, RuntimeCommand, RuntimeEnvelope, RuntimeEvent } from '../../../web/src/contracts.js';
import { defaultGraphBypassConfig, validateGraphBypassConfig, type GraphBypassConfig } from '../../../web/src/gpu/bypass.js';
import { WasmRuntimeAuthority } from './runtime/wasm-runtime.js';
import { canLoadNextDngFrame, frameLoadInvalidatesRuntime } from './runtime/dng-sequence.js';
let executor: NormalGpuExecutor | null = null;
let gpu: GpuContext | null = null;
let controller: RuntimeController | null = null;
let authority: WasmRuntimeAuthority | null = null;
let canvas: OffscreenCanvas | null = null;
let rawAsset: ArrayBuffer | null = null;
let rawByteOffset = 0;
let descriptor: RawFrameDescriptor | null = null;
let deviceWasLost = false;
const selectedMethods: Record<string, string> = { dem: '00' };
let bypassConfig: GraphBypassConfig = defaultGraphBypassConfig();
const parameterValues: Record<string, number> = {
  enable_highlight_recovery: 1,
  enable_details_amplify: 1,
  vng_threshold: 1.5,
  ahd_l_threshold: 2.0,
  ahd_c_threshold_sq: 4.0,
  gamma: 2.2,
};
let drcIqParameters: DrcIqParameters = { ...DEFAULT_DRC_IQ_PARAMETERS };
const lutValues: Record<string, readonly number[]> = {
  gamma_lut: [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1],
};
let envelope: RuntimeEnvelope = {
  graphInstanceId: 1,
  configRevision: 0,
  runRevision: 0,
  methodRevision: 0,
  frameIndex: null,
  framePhase: null,
  visibleFrameCommitted: false,
  lifecycleState: 'unloaded',
  gpuGeneration: 0,
};
const commands = new SerialCommandQueue();

self.onmessage = (message: MessageEvent<RuntimeCommand>): void => {
  void commands.enqueue(() => handleCommand(message.data)).catch((error: unknown) => {
    envelope = authority?.fail() ?? { ...envelope, lifecycleState: 'error' };
    postError(error, 'NODE_EXECUTION_FAILED');
  });
};

async function handleCommand(command: RuntimeCommand): Promise<void> {
  if (command.type === 'initialize') {
    canvas = command.canvas;
    rawAsset = command.raw;
    rawByteOffset = command.rawByteOffset;
    descriptor = command.descriptor;
    authority = await WasmRuntimeAuthority.create();
    envelope = authority.load();
    gpu = await createGpuContext(canvas, descriptor);
    createExecutor(envelope.gpuGeneration);
    watchDeviceLoss(gpu.device);
    self.postMessage({ type: 'ready', envelope } satisfies RuntimeEvent);
    return;
  }
  if (authority === null) throw new Error('INVALID_STATE_TRANSITION: Worker is not initialized');
  if (command.type === 'dispose') {
    executor?.dispose();
    executor = null;
    controller = null;
    gpu = null;
    self.close();
    return;
  }
  if (command.type === 'load_frame') {
    if (!canLoadNextDngFrame(envelope.lifecycleState)) {
      throw new Error('INVALID_STATE_TRANSITION: DNG frame can only load while stopped or completed');
    }
    const previousExecutor = executor;
    const canReuseExecutor = previousExecutor?.canReplaceFrame(command.descriptor) === true;
    rawAsset = command.raw;
    rawByteOffset = command.rawByteOffset;
    descriptor = command.descriptor;
    if (frameLoadInvalidatesRuntime(canReuseExecutor)) {
      envelope = authority.reset();
    }
    if (canReuseExecutor && previousExecutor !== null) {
      executor = previousExecutor;
      executor.replaceFrame(rawAsset, rawByteOffset, descriptor);
      // The reused executor's textures stay valid: keep the lifecycle state
      // and GPU generation so committed previews survive until the next
      // frame commits (no empty-frame flash during sequence playback).
    } else {
      executor = null;
      controller = null;
      previousExecutor?.dispose();
      if (gpu === null) throw new Error('INVALID_STATE_TRANSITION: GPU context is unavailable');
      validateGpuInput(descriptor, 4096, gpu.device.limits.maxTextureDimension2D);
      createExecutor(envelope.gpuGeneration);
    }
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_quantization_config') {
    envelope = authority.setQuantizationConfig(command.config);
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_bypass_config') {
    const next = JSON.parse(command.config) as GraphBypassConfig;
    validateGraphBypassConfig(next);
    envelope = authority.changeConfig();
    bypassConfig = next;
    if (executor !== null && 'setBypassConfig' in executor) {
      (executor as NormalGpuExecutor & { setBypassConfig(config: GraphBypassConfig): void }).setBypassConfig(next);
    }
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_drc_iq_parameters') {
    const next = JSON.parse(command.config) as DrcIqParameters;
    validateDrcIqParameters(next);
    envelope = authority.changeConfig();
    drcIqParameters = { ...next };
    const curves = next.edge_curve !== undefined && next.luma_curve !== undefined
      ? { edge: next.edge_curve, luma: next.luma_curve }
      : undefined;
    executor?.setDrcIqParameters(next, curves);
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_method') {
    if (executor === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
    executor.setMethod(command.nodeId, command.method);
    selectedMethods[command.nodeId] = command.method;
    envelope = authority.changeMethod();
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_parameter') {
    if (executor === null) throw new Error('INVALID_STATE_TRANSITION: parameter executor is unavailable');
    executor.setParameter(command.nodeId, command.parameter, command.value);
    parameterValues[command.parameter] = command.value;
    envelope = authority.changeMethod();
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_lut') {
    if (executor === null || command.nodeId !== 'gamma') throw new Error('INVALID_STATE_TRANSITION: Gamma executor is unavailable');
    executor.setLut(command.parameter, command.values);
    lutValues[command.parameter] = [...command.values];
    envelope = authority.changeMethod();
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_preview') {
    if (executor === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
    await executor.present(command.nodeA, command.nodeB, command.curtain);
    return;
  }
  if (command.type === 'sample_preview') {
    if (executor === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
    const values = await executor.sample(command.nodeId, command.x, command.y);
    self.postMessage({ type: 'preview_sample', envelope, nodeId: command.nodeId, x: command.x, y: command.y, values, requestId: command.requestId } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'step' || command.type === 'run') {
    if (controller === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
    envelope = command.type === 'step' ? authority.step(command.frameIndex) : authority.run(command.frameIndex);
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    await controller.step({
      frameIndex: command.frameIndex,
      runRevision: envelope.runRevision,
      methodRevision: envelope.methodRevision,
      gpuGeneration: envelope.gpuGeneration,
    });
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'reset') {
    if (deviceWasLost) {
      envelope = authority.reset();
      if (canvas === null || descriptor === null) throw new Error('INVALID_STATE_TRANSITION: GPU inputs are unavailable');
      gpu = await createGpuContext(canvas, descriptor);
      createExecutor(envelope.gpuGeneration);
      watchDeviceLoss(gpu.device);
      deviceWasLost = false;
    } else {
      if (controller === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
      controller.reset();
      envelope = authority.reset();
    }
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
  }
}

function parameterNode(parameter: string): string {
  if (parameter === 'enable_highlight_recovery') return 'wbc';
  if (parameter === 'enable_details_amplify') return 'drc';
  if (parameter === 'gamma') return 'gamma';
  return 'dem';
}

function createExecutor(generation: number): void {
  if (gpu === null || rawAsset === null || descriptor === null || authority === null) {
    throw new Error('INVALID_STATE_TRANSITION: GPU inputs are unavailable');
  }
  executor = new NormalGpuExecutor(gpu, rawAsset, rawByteOffset, generation, descriptor, (identity) => {
    if (authority === null || rawAsset === null || descriptor === null) {
      throw new Error('INVALID_STATE_TRANSITION: frame packet inputs are unavailable');
    }
    const packets = authority.deriveFramePackets(
      descriptor,
      rawAsset,
      rawByteOffset,
      identity.frameIndex,
      selectedMethods,
      parameterValues,
      lutValues.gamma_lut ?? DEFAULT_GAMMA_PARAMETERS.lut,
      drcIqParameters,
      bypassConfig.modules,
    );
    self.postMessage({
      type: 'preprocess_snapshot',
      envelope,
      snapshot: parsePreprocessSnapshot(packets.preprocessSnapshotJson),
    } satisfies RuntimeEvent);
    return packets;
  });
  for (const [nodeId, method] of Object.entries(selectedMethods)) executor.setMethod(nodeId, method);
  if ('setBypassConfig' in executor) {
    (executor as NormalGpuExecutor & { setBypassConfig(config: GraphBypassConfig): void }).setBypassConfig(bypassConfig);
  }
  executor.setDrcIqParameters(drcIqParameters, drcIqParameters.edge_curve !== undefined && drcIqParameters.luma_curve !== undefined ? { edge: drcIqParameters.edge_curve, luma: drcIqParameters.luma_curve } : undefined);
  for (const [parameter, value] of Object.entries(parameterValues)) executor.setParameter(parameterNode(parameter), parameter, value);
  for (const [parameter, values] of Object.entries(lutValues)) executor.setLut(parameter, values);
  controller = new RuntimeController(
    executor,
    (previews) => self.postMessage({ type: 'preview', envelope, previews } satisfies RuntimeEvent),
    (phase) => {
      if (authority === null) throw new Error('WASM authority is unavailable');
      envelope = phase === 'warmup' ? authority.completeWarmup() : authority.completeOutput();
      self.postMessage({
        type: 'log',
        envelope,
        entry: { level: 'info', message: `frame ${envelope.frameIndex ?? 0} ${phase} completed`, framePhase: phase },
      } satisfies RuntimeEvent);
    },
    () => authority?.completeFrame(),
    () => authority?.abortFrame(),
  );
}

function watchDeviceLoss(device: GPUDevice): void {
  void device.lost.then((info) => {
    void commands.enqueue(() => {
      if (authority === null || gpu?.device !== device) return;
      deviceWasLost = true;
      controller = null;
      executor = null;
      gpu = null;
      envelope = authority.deviceLost();
      postError(`WebGPU device lost: ${info.message || info.reason}`, 'GPU_DEVICE_LOST');
    });
  });
}

function parsePreprocessSnapshot(json: string): PreprocessSnapshot {
  let parsed: PreprocessSnapshot;
  try {
    parsed = JSON.parse(json) as PreprocessSnapshot;
  } catch (error) {
    throw new Error(`WASM_PREPROCESS_SNAPSHOT_INVALID: malformed preprocess snapshot (${String(error)})`);
  }
  if (typeof parsed.frameIndex !== 'number' || parsed.modules === null || typeof parsed.modules !== 'object') {
    throw new Error('WASM_PREPROCESS_SNAPSHOT_INVALID: snapshot is missing frameIndex or modules');
  }
  return parsed;
}

function postError(error: unknown, diagnosticCode: string): void {
  self.postMessage({
    type: 'log',
    envelope,
    entry: { level: 'error', message: String(error), diagnosticCode },
  } satisfies RuntimeEvent);
}
