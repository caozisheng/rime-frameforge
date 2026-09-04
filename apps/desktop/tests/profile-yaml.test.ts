import { describe, expect, it } from 'vitest';
import { parseTuningProfile, serializeTuningProfile } from '../src/components/iq/profile-yaml.js';

describe('tuning profile YAML', () => {
  it('round trips the profile identity and curves', () => {
    const yaml = serializeTuningProfile({ id: 'profile-a', name: 'Profile A', revision: 2, lCurve: [{ x: 0, y: 0.1 }], cCurve: [{ x: 0, y: 0.2 }] });
    expect(parseTuningProfile(yaml)).toEqual({ id: 'profile-a', name: 'Profile A', revision: 2, lCurve: [{ x: 0, y: 0.1 }], cCurve: [{ x: 0, y: 0.2 }] });
  });

  it('rejects YAML without the DEM curve entries', () => {
    expect(() => parseTuningProfile('kind: rime.tuning_profile\nprofile: {}')).toThrow('IQ_PROFILE_INVALID');
  });
});
