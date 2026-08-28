import type { RawFrameDescriptor, RuntimeCommand, RuntimeEvent } from '../../../../web/src/contracts.js';
import rawAssetUrl from '../../../../pipeline/normal/frame0.raw?url';

export interface DngRawTagDescriptor {
  readonly tag: number;
  readonly fieldType: string;
  readonly count: number;
  readonly value: string;
}

export interface DngMetadataDescriptor {
  readonly dngVersion: readonly number[];
  readonly backwardVersion: readonly number[] | null;
  readonly blackRepeat: readonly number[];
  readonly blackLevels: readonly number[];
  readonly blackDeltaH: readonly number[] | null;
  readonly blackDeltaV: readonly number[] | null;
  readonly whiteLevels: readonly number[];
  readonly linearizationTable: readonly number[] | null;
  readonly cameraModel: string;
  readonly colorMatrix1: readonly number[];
  readonly calibrationIlluminant1: string;
  readonly asShotNeutral: readonly number[];
  readonly colorMatrix2: readonly number[] | null;
  readonly cameraCalibration1: readonly number[] | null;
  readonly cameraCalibration2: readonly number[] | null;
  readonly forwardMatrix1: readonly number[] | null;
  readonly forwardMatrix2: readonly number[] | null;
  readonly analogBalance: readonly number[] | null;
  readonly baselineExposure: number | null;
  readonly profileName: string | null;
  readonly exifExposureTime: readonly number[] | null;
  readonly exifFNumber: readonly number[] | null;
  readonly exifIsoSpeed: number | null;
  readonly exifDateTimeOriginal: string | null;
  readonly exifFocalLength: readonly number[] | null;
  readonly xmpByteLength: number | null;
  readonly iptcByteLength: number | null;
  readonly iccByteLength: number | null;
  readonly newRawImageDigest: string | null;
  readonly ifd0Extra: readonly DngRawTagDescriptor[];
  readonly rawExtra: readonly DngRawTagDescriptor[];
  readonly exifExtra: readonly DngRawTagDescriptor[];
}

export interface DngFrameDescriptor {
  readonly frameIndex: number;
  readonly fileName: string;
  readonly width: number;
  readonly height: number;
  readonly rowStrideSamples: number;
  readonly storageBits: number;
  readonly cfa: 'rggb' | 'grbg' | 'gbrg' | 'bggr';
  readonly blackLevel: number;
  readonly whiteLevel: number;
  readonly dngVersion: readonly number[];
  readonly backwardVersion: readonly number[] | null;
  readonly cameraModel: string;
  readonly metadataHash: string;
  readonly rawDigest: string;
  readonly metadata: DngMetadataDescriptor;
}

export interface WorkerBridge {
  readonly worker: Worker;
  initialize(canvas: OffscreenCanvas): Promise<void>;
  loadFrame(raw: ArrayBuffer, descriptor: RawFrameDescriptor): void;
  setMethod(nodeId: string, method: string): void;
  setParameter(nodeId: string, parameter: string, value: number): void;
  run(): void;
  step(): void;
  reset(): void;
  dispose(): void;
}

export function createWorkerBridge(onEvent: (event: RuntimeEvent) => void): WorkerBridge {
  const worker = new Worker(new URL('../pipeline.worker.ts', import.meta.url), { type: 'module' });
  worker.addEventListener('message', (message: MessageEvent<RuntimeEvent>) => onEvent(message.data));

  const send = (command: RuntimeCommand, transfer: Transferable[] = []): void => {
    worker.postMessage(command, transfer);
  };

  return {
    worker,
    initialize: async (canvas) => {
      const response = await fetch(rawAssetUrl);
      if (!response.ok) throw new Error(`INPUT_INVALID: failed to fetch RAW asset (${response.status})`);
      const raw = await response.arrayBuffer();
      const descriptor: RawFrameDescriptor = {
        width: 32,
        height: 24,
        rowStrideSamples: 32,
        storageBits: 16,
        cfa: 'rggb',
        blackLevel: 64,
        whiteLevel: 4095,
      };
      send({ type: 'initialize', canvas, raw, descriptor }, [canvas, raw]);
    },
    loadFrame: (raw, descriptor) => send({ type: 'load_frame', raw, descriptor }, [raw]),
    setMethod: (nodeId, method) => send({ type: 'set_method', nodeId, method }),
    setParameter: (nodeId, parameter, value) => send({ type: 'set_parameter', nodeId, parameter, value }),
    run: () => send({ type: 'run' }),
    step: () => send({ type: 'step' }),
    reset: () => send({ type: 'reset' }),
    dispose: () => worker.terminate(),
  };
}
