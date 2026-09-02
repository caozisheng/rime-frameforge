import type { RawFrameDescriptor } from '../contracts.js';
import { normalGraphRequiredLimits, validateGpuInput, validateNormalGraphAdapterLimits } from './capability.js';
import { resizePreviewCanvas } from '../preview-state.js';

export interface GpuContext {
  readonly canvas: OffscreenCanvas;
  readonly device: GPUDevice;
  readonly context: GPUCanvasContext;
  readonly canvasFormat: GPUTextureFormat;
}

export async function createGpuContext(
  canvas: OffscreenCanvas,
  descriptor: RawFrameDescriptor,
): Promise<GpuContext> {
  if (!navigator.gpu) {
    throw new Error('GPU_CAPABILITY_UNSUPPORTED: WebGPU is unavailable');
  }
  const adapter = await navigator.gpu.requestAdapter();
  if (adapter === null) {
    throw new Error('GPU_CAPABILITY_UNSUPPORTED: no WebGPU adapter');
  }
  validateGpuInput(descriptor, 4096, adapter.limits.maxTextureDimension2D);
  validateNormalGraphAdapterLimits(adapter.limits);
  resizePreviewCanvas(canvas, descriptor);
  const device = await adapter.requestDevice({ requiredLimits: normalGraphRequiredLimits() });
  const context = canvas.getContext('webgpu');
  if (context === null) {
    throw new Error('GPU_CAPABILITY_UNSUPPORTED: canvas has no WebGPU context');
  }
  const canvasFormat = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format: canvasFormat, alphaMode: 'opaque' });
  return { canvas, device, context, canvasFormat };
}
