import { describe, expect, it } from 'vitest';

import { DEFAULT_DNG_EXPANDED, DNG_EXPANDED_KEY, DNG_FONT_SIZE_KEY, readDngExpanded, readDngFontSize } from '../src/components/DngMetadataTree.js';

function storage(values: Record<string, string>): Pick<Storage, 'getItem'> {
  return { getItem: (key) => values[key] ?? null };
}

describe('DNG inspector preferences', () => {
  it('loads persisted font size and clamps malformed values', () => {
    expect(readDngFontSize(storage({ [DNG_FONT_SIZE_KEY]: '13' }))).toBe(13);
    expect(readDngFontSize(storage({ [DNG_FONT_SIZE_KEY]: '99' }))).toBe(13);
    expect(readDngFontSize(storage({ [DNG_FONT_SIZE_KEY]: 'bad' }))).toBe(11);
  });

  it('loads persisted expansion state and falls back on malformed JSON', () => {
    expect([...readDngExpanded(storage({ [DNG_EXPANDED_KEY]: '["runtime","dng"]' }))]).toEqual(['runtime', 'dng']);
    expect([...readDngExpanded(storage({ [DNG_EXPANDED_KEY]: '{bad' }))]).toEqual(DEFAULT_DNG_EXPANDED);
  });
});
