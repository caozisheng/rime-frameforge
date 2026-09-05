import { describe, expect, it } from 'vitest';
import { parseTuningProfile, serializeTuningProfile } from '../src/components/iq/profile-yaml.js';

describe('tuning profile YAML', () => {
  it('round trips the profile identity, Gamma exponent, and curves', () => {
    const gammaCurve = Array.from({ length: 9 }, (_, index) => ({ x: index / 8, y: index / 8 }));
    const yaml = serializeTuningProfile({ id: 'profile-a', name: 'Profile A', revision: 2, gamma: 2.2, gammaCurve, lCurve: [{ x: 0, y: 0.1 }], cCurve: [{ x: 0, y: 0.2 }] });
    expect(parseTuningProfile(yaml)).toEqual({ id: 'profile-a', name: 'Profile A', revision: 2, gamma: 2.2, gammaCurve, lCurve: [{ x: 0, y: 0.1 }], cCurve: [{ x: 0, y: 0.2 }] });
    expect(yaml).toContain('linear_luminance_y');
    expect(yaml).toContain('gamma_lut');
  });

  it('rejects non-monotone Gamma luminance curves', () => {
    const gammaCurve = Array.from({ length: 9 }, (_, index) => ({ x: index / 8, y: index / 8 }));
    gammaCurve[4] = { x: 0.5, y: 0.2 };
    const yaml = serializeTuningProfile({ id: 'invalid', name: 'Invalid', revision: 1, gamma: 2.2, gammaCurve, lCurve: [{ x: 0, y: 1 }], cCurve: [{ x: 0, y: 3 }] });
    expect(() => parseTuningProfile(yaml)).toThrow('IQ_PROFILE_INVALID');
  });

  it('rejects YAML without the DEM curve entries', () => {
    expect(() => parseTuningProfile('kind: rime.tuning_profile\nprofile: {}')).toThrow('IQ_PROFILE_INVALID');
  });
});
