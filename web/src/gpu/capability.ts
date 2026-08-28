import type { RawFrameDescriptor } from '../contracts.js';

const BYTES_PER_PIXEL_BY_STAGE = [2, 4, 4, 16, 16, 16, 16] as const;
const DEFAULT_MAX_TEXTURE_DIMENSION = 8192;
const SAFE_MEMORY_FRACTION = 0.7;

export class GpuCapabilityError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = 'GpuCapabilityError';
  }
}

export function estimateNormalGraphLivePeakBytes(descriptor: RawFrameDescriptor): number {
  const pixels = descriptor.width * descriptor.height;
  let maxAdjacent = 0;
  for (let index = 0; index < BYTES_PER_PIXEL_BY_STAGE.length - 1; index += 1) {
    const adjacent = BYTES_PER_PIXEL_BY_STAGE[index]! + BYTES_PER_PIXEL_BY_STAGE[index + 1]!;
    maxAdjacent = Math.max(maxAdjacent, adjacent);
  }
  return pixels * (BYTES_PER_PIXEL_BY_STAGE[0] + maxAdjacent);
}

export function estimateNormalGraphPoolBytes(descriptor: RawFrameDescriptor): number {
  const pixels = descriptor.width * descriptor.height;
  return BYTES_PER_PIXEL_BY_STAGE.reduce((total, bytesPerPixel) => total + pixels * bytesPerPixel, 0);
}

export interface GpuMemoryEstimate {
  readonly livePeakBytes: number;
  readonly poolResidentBytes: number;
}

export function estimateGpuMemory(descriptor: RawFrameDescriptor): GpuMemoryEstimate {
  return {
    livePeakBytes: estimateNormalGraphLivePeakBytes(descriptor),
    poolResidentBytes: estimateNormalGraphPoolBytes(descriptor),
  };
}

export function validateGpuInput(
  descriptor: RawFrameDescriptor,
  deviceMemoryMiB: number,
  maxTextureDimension = DEFAULT_MAX_TEXTURE_DIMENSION,
): void {
  if (descriptor.width <= 0 || descriptor.height <= 0) {
    throw new GpuCapabilityError('GPU_INPUT_INVALID: extent must be positive');
  }
  if (descriptor.width > maxTextureDimension || descriptor.height > maxTextureDimension) {
    throw new GpuCapabilityError(
      `GPU_INPUT_UNSUPPORTED: extent ${descriptor.width}x${descriptor.height} exceeds ${maxTextureDimension}`,
    );
  }
  const estimate = estimateGpuMemory(descriptor);
  const budgetBytes = deviceMemoryMiB * 1024 * 1024 * SAFE_MEMORY_FRACTION;
  if (estimate.poolResidentBytes > budgetBytes) {
    throw new GpuCapabilityError(
      `GPU_MEMORY_BUDGET_EXCEEDED: pool ${estimate.poolResidentBytes} bytes, live peak ${estimate.livePeakBytes} bytes, budget ${Math.floor(budgetBytes)} bytes`,
    );
  }
}
