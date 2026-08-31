import { createGpuContext } from '../../../web/src/gpu/device.js';
import { NormalGpuExecutor } from '../../../web/src/gpu/executor.js';
import { RuntimeController } from '../../../web/src/runtime-controller.js';
import { SerialCommandQueue } from '../../../web/src/serial-command-queue.js';
import type { RawFrameDescriptor, RuntimeCommand, RuntimeEnvelope, RuntimeEvent } from '../../../web/src/contracts.js';
import { WasmRuntimeAuthority } from './runtime/wasm-runtime.js';
import { canLoadNextDngFrame } from './runtime/dng-sequence.js';

let executor: NormalGpuExecutor | null = null;
let controller: RuntimeController | null = null;
let authority: WasmRuntimeAuthority | null = null;
let canvas: OffscreenCanvas | null = null;
let rawAsset: ArrayBuffer | null = null;
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
    descriptor = command.descriptor;
    authority = await WasmRuntimeAuthority.create();
    envelope = authority.load();
    await createExecutor(envelope.gpuGeneration);
    self.postMessage({ type: 'ready', envelope } satisfies RuntimeEvent);
    return;
  }
  if (authority === null) throw new Error('INVALID_STATE_TRANSITION: Worker is not initialized');
  if (command.type === 'load_frame') {
    if (!canLoadNextDngFrame(envelope.lifecycleState)) {
      throw new Error('INVALID_STATE_TRANSITION: DNG frame can only load while stopped or completed');
    }
    rawAsset = command.raw;
    descriptor = command.descriptor;
    envelope = authority.reset();
    await createExecutor(envelope.gpuGeneration);
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
      await createExecutor(envelope.gpuGeneration);
      deviceWasLost = false;
    } else {
      if (controller === null) throw new Error('INVALID_STATE_TRANSITION: GPU executor is unavailable');
      controller.reset();
      envelope = authority.reset();
    }
    self.postMessage({ type: 'snapshot', envelope } satisfies RuntimeEvent);
  }
}

async function createExecutor(generation: number): Promise<void> {
  if (canvas === null || rawAsset === null || descriptor === null || authority === null) {
    throw new Error('INVALID_STATE_TRANSITION: GPU inputs are unavailable');
  }
  const gpu = await createGpuContext(canvas, descriptor);
  executor = new NormalGpuExecutor(gpu, rawAsset.slice(0), generation, descriptor);
  executor.setQuantizationConfig(JSON.parse(authority.quantizationConfig()));
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
  void gpu.device.lost.then((info) => {
    void commands.enqueue(() => {
      if (authority === null) return;
      deviceWasLost = true;
      controller = null;
      executor = null;
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
