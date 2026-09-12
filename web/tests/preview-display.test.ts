import presentShader from '../../crates/rime-isp/src/shaders/present.wgsl?raw';

import { it, expect } from 'vitest';

it('encodes linear raw-gray previews before presenting them', () => {
  expect(presentShader).toContain('linear_to_srgb');
  expect(presentShader).toContain('display_code(vec3<f32>(linear_to_srgb(value)))');
});
