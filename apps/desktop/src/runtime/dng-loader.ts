import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import type { RawFrameDescriptor } from '../../../../web/src/contracts.js';
import type { DngFrameDescriptor, WorkerBridge } from './worker-bridge.js';

export interface LoadedDngSelection {
  readonly descriptor: DngFrameDescriptor;
  readonly paths: readonly string[];
}

export async function loadDngIntoWorker(bridge: WorkerBridge): Promise<LoadedDngSelection> {
  const selected = await open({
    multiple: true,
    directory: false,
    filters: [{ name: 'DNG RAW', extensions: ['dng'] }],
  });
  const paths = typeof selected === 'string' ? [selected] : selected;
  if (paths === null || paths.length === 0) {
    throw new Error('INPUT_CANCELLED: no DNG file selected');
  }
  const orderedPaths = [...paths].sort((left, right) => left.localeCompare(right, undefined, { numeric: true }));
  const descriptor = await loadDngPathIntoWorker(bridge, orderedPaths[0]!, 0);
  return { descriptor, paths: orderedPaths };
}

export async function loadDngPathIntoWorker(
  bridge: WorkerBridge,
  path: string,
  frameIndex = 0,
): Promise<DngFrameDescriptor> {
  const descriptor = await invoke<DngFrameDescriptor>('inspect_dng_frame', { path, frameIndex });
  const rawResponse = await invoke<ArrayBuffer>('read_dng_raw', { path });
  bridge.loadFrame(rawResponse, {
    width: descriptor.width,
    height: descriptor.height,
    rowStrideSamples: descriptor.rowStrideSamples,
    storageBits: descriptor.storageBits,
    cfa: descriptor.cfa,
    blackLevel: descriptor.blackLevel,
    whiteLevel: descriptor.whiteLevel,
  });
  return descriptor;
}
