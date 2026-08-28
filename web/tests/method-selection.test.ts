import { describe, expect, it } from 'vitest';

import { normalManifest } from '../src/generated/normal_manifest.generated.js';
import { resolveShaderEntry } from '../src/gpu/method-selection.js';

describe('resolveShaderEntry', () => {
  it('uses the selected DEM method instead of the manifest default', () => {
    const dem = normalManifest.nodes.find((node) => node.id === 'dem');
    expect(dem).toBeDefined();

    expect(resolveShaderEntry(dem!, { dem: '04' })).toBe('demosaic_ahd_main');
  });
});
