// Fused Normal Graph shader access — pure forwarding.
//
// The fused composition (FusedParams super-uniform, quantize helpers,
// CR/gamma helpers, WBC passthrough, demosaic adapters, postprocess chain)
// is owned by `crates/rime-isp/src/fused_view.rs` and lands here via
// `npm run generate:manifest` as generated assets. This file must contain
// NO shader text and NO composition logic — only re-exports and the
// dem-method dispatch table.
import { blcPipelineWgsl } from '../generated/blc_pipeline.generated.js';
import { fusedPipelineWgsl } from '../generated/fused_pipeline.generated.js';
import { segmented01Shaders, segmented02Shaders, segmented03Shaders, segmented04Shaders } from '../generated/segmented_fused.generated.js';

export interface SegmentedNormalShaders {
  readonly pre: string;
  readonly dem: string;
  readonly quantize: string;
  readonly post: string;
}

const SEGMENTED_SHADERS = {
  '01': segmented01Shaders,
  '02': segmented02Shaders,
  '03': segmented03Shaders,
  '04': segmented04Shaders,
} as const;

export type SegmentedDemMethod = keyof typeof SEGMENTED_SHADERS;

export function isSegmentedDemMethod(method: string): method is SegmentedDemMethod {
  return method in SEGMENTED_SHADERS;
}

/** Standalone pre-DRC BLC pass (single-source `blc00.wgsl`). */
export function compileBlcShader(): string {
  return blcPipelineWgsl;
}


/** Fused post-DRC single pass (dem method 00). */
export function compileFusedNormalShader(): string {
  return fusedPipelineWgsl;
}

/** Segmented complex-DEM path (methods 01–04). */
export function compileSegmentedNormalShaders(method: SegmentedDemMethod): SegmentedNormalShaders {
  const shaders = SEGMENTED_SHADERS[method];
  if (shaders === undefined) throw new Error(`FUSED_GRAPH_METHOD_INVALID: unknown DEM method ${method}`);
  return shaders;
}
