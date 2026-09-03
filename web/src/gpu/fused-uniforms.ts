import type { BayerCfa, RawFrameDescriptor } from '../contracts.js';
import { buildGpuQuantizationPlans, type QuantizationConfig } from './quantization.js';

const QUANT_BLOCK_BYTES = 64;
const QUANT_BLOCK_OFFSET = 64;
const QUANT_MODULE_IDS = ['blc', 'wbc', 'dem', 'color_correction', 'gamma', 'rgb2yuv'] as const;
export const FUSED_UNIFORM_BYTES = QUANT_BLOCK_OFFSET + QUANT_BLOCK_BYTES * QUANT_MODULE_IDS.length + 32;

export interface FusedDemosaicParameters {
  readonly vng_threshold: number;
  readonly ahd_l_threshold: number;
  readonly ahd_c_threshold_sq: number;
}

export function packFusedUniforms(
  descriptor: RawFrameDescriptor,
  frameIndex: number,
  demosaicParameters: FusedDemosaicParameters,
  quantization: QuantizationConfig,
): ArrayBuffer {
  const bytes = new ArrayBuffer(FUSED_UNIFORM_BYTES);
  const view = new DataView(bytes);
  view.setUint32(0, descriptor.width, true);
  view.setUint32(4, descriptor.height, true);
  view.setFloat32(8, descriptor.blackLevel, true);
  view.setFloat32(12, descriptor.whiteLevel, true);
  cfaPattern(descriptor.cfa).forEach((channel, index) => view.setUint32(16 + index * 4, channel, true));
  descriptor.whiteBalanceGains.forEach((gain, index) => view.setFloat32(32 + index * 4, gain, true));
  view.setFloat32(48, demosaicParameters.vng_threshold, true);
  view.setFloat32(52, demosaicParameters.ahd_l_threshold, true);
  view.setFloat32(56, demosaicParameters.ahd_c_threshold_sq, true);
  view.setUint32(60, frameIndex, true);

  const plans = buildGpuQuantizationPlans(quantization, descriptor.width, descriptor.height, frameIndex);
  QUANT_MODULE_IDS.forEach((moduleId, index) => {
    const plan = plans.get(moduleId);
    if (plan === undefined) return;
    const offset = QUANT_BLOCK_OFFSET + index * QUANT_BLOCK_BYTES;
    view.setFloat32(offset, plan.scale, true);
    view.setFloat32(offset + 4, plan.qmin, true);
    view.setFloat32(offset + 8, plan.qmax, true);
    view.setUint32(offset + 12, plan.roundingMode, true);
    view.setUint32(offset + 16, plan.seed, true);
    view.setUint32(offset + 20, plan.streamId, true);
    view.setUint32(offset + 24, plan.frameIndex, true);
    view.setUint32(offset + 28, plan.plane, true);
    view.setUint32(offset + 32, descriptor.width, true);
    view.setUint32(offset + 36, descriptor.height, true);
    view.setUint32(offset + 40, plan.ppc, true);
    view.setUint32(offset + 44, 0, true);
    view.setUint32(offset + 48, plan.groupsPerRow, true);
    view.setUint32(offset + 52, plan.groupsPerFrame, true);
    view.setUint32(offset + 56, 0, true);
    view.setUint32(offset + 60, 0, true);
    if (plan.outputEnabled) {
      const flagOffset = 448 + index * 4;
      view.setUint32(flagOffset, 1, true);
    }
  });
  return bytes;
}

function cfaPattern(cfa: BayerCfa): readonly number[] {
  const patterns: Record<BayerCfa, readonly number[]> = {
    rggb: [0, 1, 1, 2],
    grbg: [1, 0, 2, 1],
    gbrg: [1, 2, 0, 1],
    bggr: [2, 1, 1, 0],
  };
  return patterns[cfa];
}
