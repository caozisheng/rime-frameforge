import { describe, expect, it } from 'vitest';

import { s0ToS8Display } from '../src/gpu/display.js';

describe('s0.Y preview display conversion', () => {
  it('shifts normalized s0.Y values to 8-bit display codes', () => {
    expect(s0ToS8Display(0.0)).toBe(0);
    expect(s0ToS8Display(0.5)).toBe(128);
    expect(s0ToS8Display(1.0)).toBe(255);
  });

  it('clips negative values to black and high values to white', () => {
    expect(s0ToS8Display(-0.25)).toBe(0);
    expect(s0ToS8Display(1.5)).toBe(255);
  });
});
