import { describe, expect, it } from 'vitest';

import { normalNodeAppearance } from '../src/normal-node-appearance.js';

describe('normalNodeAppearance', () => {
  it('marks only enabled operators as implemented', () => {
    expect(normalNodeAppearance('operator', 'enabled')).toBe('implemented');
    expect(normalNodeAppearance('operator', 'bypass')).toBe('neutral');
    expect(normalNodeAppearance('operator', 'disabled')).toBe('neutral');
    expect(normalNodeAppearance('group', 'enabled')).toBe('neutral');
    expect(normalNodeAppearance('endpoint', 'enabled')).toBe('neutral');
  });
});
