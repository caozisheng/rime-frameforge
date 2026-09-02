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
  readonly configRevision: number;
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
  readonly frameIndex: number;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly gpuGeneration: number;
  readonly width: number;
  readonly height: number;
  readonly format: 'r16_uint' | 'r32_float' | 'rgba32_float';
  readonly domain: 'raw_bayer_sensor' | 'raw_bayer_rime_q' | 'linear_rgb' | 'encoded_rgb' | 'yuv';
  readonly range: string;
  readonly channelLayout: string;
  readonly presentation: 'raw_gray' | 'rgb' | 'yuv';
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
  | { readonly type: 'initialize'; readonly canvas: OffscreenCanvas; readonly raw: ArrayBuffer; readonly rawByteOffset: number; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'load_frame'; readonly raw: ArrayBuffer; readonly rawByteOffset: number; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'set_method'; readonly nodeId: string; readonly method: string }
  | { readonly type: 'set_parameter'; readonly nodeId: string; readonly parameter: string; readonly value: number }
  | { readonly type: 'set_quantization_config'; readonly config: string }
  | { readonly type: 'set_preview'; readonly nodeA: string; readonly nodeB: string | null; readonly curtain: number }
  | { readonly type: 'sample_preview'; readonly nodeId: string; readonly x: number; readonly y: number; readonly requestId: number }
  | { readonly type: 'run'; readonly frameIndex: number }
  | { readonly type: 'step'; readonly frameIndex: number }
  | { readonly type: 'reset' }
  | { readonly type: 'dispose' };

export type RuntimeEvent =
  | { readonly type: 'ready'; readonly envelope: RuntimeEnvelope }
  | { readonly type: 'snapshot'; readonly envelope: RuntimeEnvelope }
  | { readonly type: 'preview'; readonly envelope: RuntimeEnvelope; readonly previews: readonly PreviewDescriptor[] }
  | { readonly type: 'preview_sample'; readonly envelope: RuntimeEnvelope; readonly nodeId: string; readonly x: number; readonly y: number; readonly values: readonly number[]; readonly requestId: number }
  | { readonly type: 'timings'; readonly envelope: RuntimeEnvelope; readonly timings: readonly NodeTiming[] }
  | { readonly type: 'log'; readonly envelope: RuntimeEnvelope; readonly entry: RuntimeLogEntry };
