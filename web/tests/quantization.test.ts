import { describe, expect, it } from 'vitest';

import { buildGpuQuantizationPlans } from '../src/gpu/quantization.js';

describe('GPU quantization plans', () => {
  const config = {
    graph_id: 'normal',
    enabled: true,
    modules: [{ module_id: 'blc', output_enabled: true, output_profile: 's0.8', clip_type: 'dither_gpu' as const }],
  };

  it('maps dither_gpu to the GPU rounding mode and s0 range', () => {
    const plans = buildGpuQuantizationPlans(config, 32, 24);
    expect(plans.get('blc')).toMatchObject({
      outputEnabled: true,
      scale: 256,
      qmin: -1,
      qmax: 255 / 256,
      roundingMode: 3,
      seed: 0x1a5b6cfd,
      groupsPerRow: 32,
      groupsPerFrame: 768,
    });
  });

  it('turns graph disable into disabled output plans without changing mode', () => {
    const plans = buildGpuQuantizationPlans(config, 32, 24, 4, false);
    expect(plans.get('blc')).toMatchObject({ outputEnabled: false, roundingMode: 3, frameIndex: 4 });
  });
});
