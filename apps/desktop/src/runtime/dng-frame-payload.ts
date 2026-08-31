import type { DngFrameDescriptor } from './worker-bridge.js';

const HEADER_BYTES = 4;
const textDecoder = new TextDecoder();

export interface DecodedDngFramePayload {
  readonly descriptor: DngFrameDescriptor;
  readonly payload: ArrayBuffer;
  readonly rawByteOffset: number;
}

export function decodeDngFramePayload(payload: ArrayBuffer): DecodedDngFramePayload {
  if (payload.byteLength < HEADER_BYTES) {
    throw new Error('DNG_FRAME_PAYLOAD_INVALID: missing descriptor length');
  }
  const descriptorLength = new DataView(payload).getUint32(0, true);
  const descriptorEnd = HEADER_BYTES + descriptorLength;
  const rawByteOffset = descriptorEnd + (descriptorLength & 1);
  if (rawByteOffset > payload.byteLength) {
    throw new Error('DNG_FRAME_PAYLOAD_INVALID: descriptor exceeds payload');
  }
  const descriptorBytes = new Uint8Array(payload, HEADER_BYTES, descriptorLength);
  const descriptor = JSON.parse(textDecoder.decode(descriptorBytes)) as DngFrameDescriptor;
  return { descriptor, payload, rawByteOffset };
}
