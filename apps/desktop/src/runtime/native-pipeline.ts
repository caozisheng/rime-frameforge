import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { DngFrameDescriptor } from './worker-bridge.js';

export interface NativeRenderDescriptor {
  readonly frameIndex: number;
  readonly width: number;
  readonly height: number;
  readonly nodeId: 'rgb2yuv';
  readonly portId: 'out';
  readonly encoderBackend: 'cpu_readback';
  readonly previewDataUrl: string;
}

export type NativePipelineEvent =
  | { readonly event: 'frame_started'; readonly frame_index: number }
  | { readonly event: 'frame_completed'; readonly descriptor: NativeRenderDescriptor };

export async function inspectDngNative(path: string, frameIndex: number): Promise<DngFrameDescriptor> {
  return invoke<DngFrameDescriptor>('inspect_dng_native', { path, frameIndex });
}

export async function renderDngNative(path: string, frameIndex: number): Promise<NativeRenderDescriptor> {
  return invoke<NativeRenderDescriptor>('render_dng_native', { path, frameIndex });
}

export function listenNativePipeline(listener: (event: NativePipelineEvent) => void): Promise<UnlistenFn> {
  return listen<NativePipelineEvent>('native-pipeline', ({ payload }) => listener(payload));
}
