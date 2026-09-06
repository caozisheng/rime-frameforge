import type { RawFrameDescriptor } from '../contracts.js';

const NORMAL_FULL_RESOLUTION_BYTES_PER_PIXEL = 54;
const DRC_LEVEL_BYTES_PER_PIXEL = 76;
const DRC_BUFFER_BYTES = 48 + 257 * (1 + 8 * 6) * 4;
const DEFAULT_MAX_TEXTURE_DIMENSION = 8192;
const SAFE_MEMORY_FRACTION = 0.7;

export class GpuCapabilityError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = 'GpuCapabilityError';
  }
}

const NORMAL_GRAPH_STORAGE_TEXTURES = 6;

export function normalGraphRequiredLimits(): Record<'maxStorageTexturesPerShaderStage', number> {
  return { maxStorageTexturesPerShaderStage: NORMAL_GRAPH_STORAGE_TEXTURES };
}

export function validateNormalGraphAdapterLimits(limits: Pick<GPUSupportedLimits, 'maxStorageTexturesPerShaderStage'>): void {
  if (limits.maxStorageTexturesPerShaderStage < NORMAL_GRAPH_STORAGE_TEXTURES) {
    throw new GpuCapabilityError(`GPU_CAPABILITY_UNSUPPORTED: Normal Graph Preview requires ${NORMAL_GRAPH_STORAGE_TEXTURES} storage textures per compute stage; adapter supports ${limits.maxStorageTexturesPerShaderStage}`);
  }
}

export function estimateNormalGraphLivePeakBytes(descriptor: RawFrameDescriptor): number {
  return retainedGraphBytes(descriptor);
}

export function estimateNormalGraphPoolBytes(descriptor: RawFrameDescriptor): number {
  return retainedGraphBytes(descriptor);
}

function retainedGraphBytes(descriptor: RawFrameDescriptor): number {
  const fullPixels = descriptor.width * descriptor.height;
  let width = descriptor.width;
  let height = descriptor.height;
  let pyramidPixels = 0;
  for (let level = 0; level < 3; level += 1) {
    pyramidPixels += width * height;
    width = Math.max(1, Math.floor(width / 2));
    height = Math.max(1, Math.floor(height / 2));
  }
  return fullPixels * NORMAL_FULL_RESOLUTION_BYTES_PER_PIXEL
    + pyramidPixels * DRC_LEVEL_BYTES_PER_PIXEL
    + DRC_BUFFER_BYTES;
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
