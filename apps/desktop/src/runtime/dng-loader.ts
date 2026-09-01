import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import type { RawFrameDescriptor } from '../../../../web/src/contracts.js';
import { inspectDngNative } from './native-pipeline.js';
import type { DngFrameDescriptor, DngSequenceDescriptor, WorkerBridge } from './worker-bridge.js';
import { decodeDngFramePayload, type DecodedDngFramePayload } from './dng-frame-payload.js';

export interface LoadedDngSelection {
  readonly descriptor: DngFrameDescriptor;
  readonly paths: readonly string[];
}

export interface LoadedDngSequence {
  readonly descriptor: DngFrameDescriptor;
  readonly sequence: DngSequenceDescriptor;
}

export async function loadDngIntoWorker(bridge: WorkerBridge): Promise<LoadedDngSelection> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'DNG RAW', extensions: ['dng'] }],
  });
  if (typeof selected !== 'string') {
    throw new Error('INPUT_CANCELLED: no DNG file selected');
  }
  const descriptor = await loadDngPathIntoWorker(bridge, selected, 0);
  return { descriptor, paths: [selected] };
}

export async function loadDngSequencePathIntoWorker(bridge: WorkerBridge, selected: string): Promise<LoadedDngSequence> {
  const sequence = await invoke<DngSequenceDescriptor>('list_dng_sequence', { path: selected });
  const firstPath = sequence.paths[0];
  if (firstPath === undefined || sequence.frameCount !== sequence.paths.length || sequence.frameCount !== sequence.fileNames.length) {
    throw new Error('DNG_SEQUENCE_INVALID: native sequence descriptor is inconsistent');
  }
  const descriptor = await loadDngPathIntoWorker(bridge, firstPath, 0);
  return { descriptor, sequence };
}

export async function loadDngSequenceIntoWorker(bridge: WorkerBridge): Promise<LoadedDngSequence> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'DNG RAW', extensions: ['dng'] }],
  });
  if (typeof selected !== 'string') {
    throw new Error('INPUT_CANCELLED: no DNG file selected');
  }
  return loadDngSequencePathIntoWorker(bridge, selected);
}

export async function decodeDngPath(path: string, frameIndex = 0): Promise<DecodedDngFramePayload> {
  const payload = await invoke<ArrayBuffer>('read_dng_frame', { path, frameIndex });
  return decodeDngFramePayload(payload);
}

export function loadDecodedDngIntoWorker(bridge: WorkerBridge, decoded: DecodedDngFramePayload): void {
  const descriptor = decoded.descriptor;
  bridge.loadFrame(decoded.payload, decoded.rawByteOffset, {
    width: descriptor.width,
    height: descriptor.height,
    rowStrideSamples: descriptor.rowStrideSamples,
    storageBits: descriptor.storageBits,
    cfa: descriptor.cfa,
    blackLevel: descriptor.blackLevel,
    whiteLevel: descriptor.whiteLevel,
  });
}

export async function loadDngPathIntoWorker(
  bridge: WorkerBridge,
  path: string,
  frameIndex = 0,
): Promise<DngFrameDescriptor> {
  const payload = await invoke<ArrayBuffer>('read_dng_frame', { path, frameIndex });
  const decoded = decodeDngFramePayload(payload);
  loadDecodedDngIntoWorker(bridge, decoded);
  return decoded.descriptor;
}
