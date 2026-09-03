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
const NORMAL_GRAPH_STORAGE_TEXTURES = 6;
const DEFAULT_MAX_BUFFER_SIZE = 256 * 1024 * 1024;
const GPU_COPY_ALIGNMENT = 256;
const RGBA32_BYTES_PER_PIXEL = 16;

export function normalGraphRequiredLimits(descriptor: RawFrameDescriptor): Record<'maxStorageTexturesPerShaderStage' | 'maxBufferSize', number> {
  const textureBytesPerRow = Math.ceil((descriptor.width * RGBA32_BYTES_PER_PIXEL) / GPU_COPY_ALIGNMENT) * GPU_COPY_ALIGNMENT;
  const textureBytes = textureBytesPerRow * descriptor.height;
  return {
    maxStorageTexturesPerShaderStage: NORMAL_GRAPH_STORAGE_TEXTURES,
    maxBufferSize: Math.max(DEFAULT_MAX_BUFFER_SIZE, textureBytes),
  };
}

export function validateNormalGraphAdapterLimits(
  limits: Pick<GPUSupportedLimits, 'maxStorageTexturesPerShaderStage' | 'maxBufferSize'>,
  descriptor: RawFrameDescriptor,
): void {
  if (limits.maxStorageTexturesPerShaderStage < NORMAL_GRAPH_STORAGE_TEXTURES) {
    throw new GpuCapabilityError(`GPU_CAPABILITY_UNSUPPORTED: Normal Graph Preview requires ${NORMAL_GRAPH_STORAGE_TEXTURES} storage textures per compute stage; adapter supports ${limits.maxStorageTexturesPerShaderStage}`);
  }
  const requiredBufferSize = normalGraphRequiredLimits(descriptor).maxBufferSize;
  if (limits.maxBufferSize < requiredBufferSize) {
    throw new GpuCapabilityError(`GPU_CAPABILITY_UNSUPPORTED: Normal Graph Preview requires ${requiredBufferSize} bytes of buffer capacity; adapter supports ${limits.maxBufferSize}`);
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
