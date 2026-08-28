export type LifecycleState =
  | 'unloaded'
  | 'loading'
  | 'stop'
  | 'running'
  | 'stepping'
  | 'paused'
  | 'completed'
  | 'error';

export type FramePhase = 'warmup' | 'output';

export interface RuntimeEnvelope {
  readonly graphInstanceId: number;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly frameIndex: number | null;
  readonly framePhase: FramePhase | null;
  readonly visibleFrameCommitted: boolean;
  readonly lifecycleState: LifecycleState;
  readonly gpuGeneration: number;
}

export type BayerCfa = 'rggb' | 'grbg' | 'gbrg' | 'bggr';

export interface RawFrameDescriptor {
  readonly width: number;
  readonly height: number;
  readonly rowStrideSamples: number;
  readonly storageBits: number;
  readonly cfa: BayerCfa;
  readonly blackLevel: number;
  readonly whiteLevel: number;
}
export interface TransferAuditSnapshot {
  readonly hostReadBytes: number;
  readonly hostWriteBytes: number;
  readonly gpuCopyBytes: number;
}

export interface PreviewDescriptor {
  readonly nodeId: string;
  readonly portId: string;
  readonly frameIndex: 0;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly gpuGeneration: number;
  readonly width: number;
  readonly height: number;
  readonly format: 'rgba32float';
  readonly domain: 'yuv';
}

export interface NodeTiming {
  readonly nodeId: string;
  readonly framePhase: FramePhase;
  readonly milliseconds: number;
}

export interface RuntimeLogEntry {
  readonly level: 'info' | 'error';
  readonly message: string;
  readonly nodeId?: string;
  readonly framePhase?: FramePhase;
  readonly diagnosticCode?: string;
}

export type RuntimeCommand =
  | { readonly type: 'initialize'; readonly canvas: OffscreenCanvas; readonly raw: ArrayBuffer; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'load_frame'; readonly raw: ArrayBuffer; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'set_method'; readonly nodeId: string; readonly method: string }
  | { readonly type: 'set_parameter'; readonly nodeId: string; readonly parameter: string; readonly value: number }
  | { readonly type: 'run' }
  | { readonly type: 'step' }
  | { readonly type: 'reset' };

export type RuntimeEvent =
  | { readonly type: 'ready'; readonly envelope: RuntimeEnvelope }
  | { readonly type: 'snapshot'; readonly envelope: RuntimeEnvelope }
  | { readonly type: 'preview'; readonly envelope: RuntimeEnvelope; readonly preview: PreviewDescriptor }
  | { readonly type: 'timings'; readonly envelope: RuntimeEnvelope; readonly timings: readonly NodeTiming[] }
  | { readonly type: 'log'; readonly envelope: RuntimeEnvelope; readonly entry: RuntimeLogEntry };
