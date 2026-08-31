import { createGpuContext, type GpuContext } from '../../../web/src/gpu/device.js';
import { validateGpuInput } from '../../../web/src/gpu/capability.js';
import { NormalGpuExecutor } from '../../../web/src/gpu/executor.js';
import { RuntimeController } from '../../../web/src/runtime-controller.js';
import { SerialCommandQueue } from '../../../web/src/serial-command-queue.js';
import type { RawFrameDescriptor, RuntimeCommand, RuntimeEnvelope, RuntimeEvent } from '../../../web/src/contracts.js';
import { WasmRuntimeAuthority } from './runtime/wasm-runtime.js';
import { canLoadNextDngFrame } from './runtime/dng-sequence.js';

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
const parameterValues: Record<string, number> = {
  vng_threshold: 1.5,
  ahd_l_threshold: 2.0,
  ahd_c_threshold_sq: 4.0,
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
    rawAsset = command.raw;
    rawByteOffset = command.rawByteOffset;
    descriptor = command.descriptor;
    envelope = authority.reset();
    if (executor?.canReplaceFrame(descriptor) === true) {
      executor.replaceFrame(rawAsset, rawByteOffset, descriptor);
    } else {
      executor?.dispose();
      if (gpu === null) throw new Error('INVALID_STATE_TRANSITION: GPU context is unavailable');
      validateGpuInput(descriptor, 4096, gpu.device.limits.maxTextureDimension2D);
      createExecutor(envelope.gpuGeneration);
    }
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
    return;
  }
  if (command.type === 'set_quantization_config') {
    envelope = authority.setQuantizationConfig(command.config);
    executor?.setQuantizationConfig(JSON.parse(command.config) as Parameters<typeof executor.setQuantizationConfig>[0]);
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
    if (executor === null || command.nodeId !== 'dem') {
      throw new Error('INVALID_STATE_TRANSITION: DEM executor is unavailable');
    }
    executor.setParameter(command.parameter, command.value);
    parameterValues[command.parameter] = command.value;
    envelope = authority.changeMethod();
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
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

function createExecutor(generation: number): void {
  if (gpu === null || rawAsset === null || descriptor === null || authority === null) {
    throw new Error('INVALID_STATE_TRANSITION: GPU inputs are unavailable');
  }
  const quantization = JSON.parse(authority.quantizationConfig()) as Parameters<NormalGpuExecutor['setQuantizationConfig']>[0];
  executor = new NormalGpuExecutor(gpu, rawAsset, rawByteOffset, generation, descriptor, quantization);
  for (const [nodeId, method] of Object.entries(selectedMethods)) executor.setMethod(nodeId, method);
  for (const [parameter, value] of Object.entries(parameterValues)) executor.setParameter(parameter, value);
  controller = new RuntimeController(
    executor,
    (preview) => self.postMessage({ type: 'preview', envelope, preview } satisfies RuntimeEvent),
    (phase) => {
      if (authority === null) throw new Error('WASM authority is unavailable');
      envelope = phase === 'warmup' ? authority.completeWarmup() : authority.completeOutput();
      self.postMessage({
        type: 'log',
        envelope,
        entry: { level: 'info', message: `frame ${envelope.frameIndex ?? 0} ${phase} completed`, framePhase: phase },
      } satisfies RuntimeEvent);
    }
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

function postError(error: unknown, diagnosticCode: string): void {
  self.postMessage({
    type: 'log',
    envelope,
    entry: { level: 'error', message: String(error), diagnosticCode },
  } satisfies RuntimeEvent);
}
