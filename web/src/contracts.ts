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

export interface ColorReproduceAssets {
  readonly sensorToProphoto: readonly number[];
  readonly prophotoToSrgb: readonly number[];
  readonly hsDims: readonly [number, number];
  readonly hsEnable: boolean;
  readonly hsLut?: readonly number[] | null;
}
export interface FramePreprocessMetadata {
  readonly colorMatrix1: readonly number[];
  readonly colorMatrix2?: readonly number[] | null;
  readonly asShotNeutral?: readonly number[] | null;
  readonly asShotWhiteXY?: readonly number[] | null;
  readonly cameraCalibration1?: readonly number[] | null;
  readonly cameraCalibration2?: readonly number[] | null;
  readonly analogBalance?: readonly number[] | null;
  readonly baselineExposure?: number | null;
  readonly exifExposureTime?: readonly number[] | null;
  readonly exifFNumber?: readonly number[] | null;
  readonly exifIsoSpeed?: number | null;
  readonly exifBrightnessValue?: number | null;
  readonly exifExposureBiasValue?: number | null;
}


export interface RawFrameDescriptor {
  readonly width: number;
  readonly height: number;
  readonly rowStrideSamples: number;
  readonly storageBits: number;
  readonly cfa: BayerCfa;
  readonly blackLevel: number;
  readonly whiteLevel: number;
  readonly whiteBalanceGains: readonly [number, number, number];
  readonly baselineExposure?: number | null;
  readonly colorReproduce?: ColorReproduceAssets | null;
  readonly metadata: FramePreprocessMetadata;
}
export interface FramePacketBytes {
  readonly blcUniform: Uint8Array<ArrayBuffer>;
  readonly wbcUniform: Uint8Array<ArrayBuffer>;
  readonly drcUniform: Uint8Array<ArrayBuffer>;
  readonly demUniform: Uint8Array<ArrayBuffer>;
  readonly drcGlobalLut: Uint8Array<ArrayBuffer>;
  readonly drcLocalLut: Uint8Array<ArrayBuffer>;
  readonly drcModulationLuts: Uint8Array<ArrayBuffer>;
  readonly fusedUniform: Uint8Array<ArrayBuffer>;
  readonly colorReproduceHsLut: Uint8Array<ArrayBuffer>;
  readonly preprocessSnapshotJson: string;
}
export type FramePacketProvider = (identity: Readonly<{ frameIndex: number }>) => FramePacketBytes;
export type PreprocessParameterValue = number | boolean | string | readonly number[] | null;
export interface PreprocessModuleSnapshot {
  readonly method: string;
  readonly parameters: Readonly<Record<string, PreprocessParameterValue>>;
}
export interface PreprocessSnapshot {
  readonly frameIndex: number;
  readonly modules: Readonly<Record<string, PreprocessModuleSnapshot>>;
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

export interface DrcIqParameters {
  readonly drc_gain_offset_ev: number;
  readonly knee: number;
  readonly amplifier: number;
  readonly edge_curve?: readonly (readonly [number, number])[];
  readonly luma_curve?: readonly (readonly [number, number])[];
}

export type RuntimeCommand =
  | { readonly type: 'initialize'; readonly canvas: OffscreenCanvas; readonly raw: ArrayBuffer; readonly rawByteOffset: number; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'load_frame'; readonly raw: ArrayBuffer; readonly rawByteOffset: number; readonly descriptor: RawFrameDescriptor }
  | { readonly type: 'set_method'; readonly nodeId: string; readonly method: string }
  | { readonly type: 'set_parameter'; readonly nodeId: string; readonly parameter: string; readonly value: number }
  | { readonly type: 'set_lut'; readonly nodeId: string; readonly parameter: string; readonly values: readonly number[] }
  | { readonly type: 'set_quantization_config'; readonly config: string }
  | { readonly type: 'set_drc_iq_parameters'; readonly config: string }
  | { readonly type: 'set_bypass_config'; readonly config: string }
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
  | { readonly type: 'preprocess_snapshot'; readonly envelope: RuntimeEnvelope; readonly snapshot: PreprocessSnapshot }
  | { readonly type: 'log'; readonly envelope: RuntimeEnvelope; readonly entry: RuntimeLogEntry };
