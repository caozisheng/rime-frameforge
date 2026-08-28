import { describe, expect, it } from 'vitest';

import { readPaneLayout, writePaneLayout } from '../src/pane-layout.js';

class MemoryStorage implements Storage {
  readonly #values = new Map<string, string>();
  get length(): number { return this.#values.size; }
  clear(): void { this.#values.clear(); }
  getItem(key: string): string | null { return this.#values.get(key) ?? null; }
  key(index: number): string | null { return [...this.#values.keys()][index] ?? null; }
  removeItem(key: string): void { this.#values.delete(key); }
  setItem(key: string, value: string): void { this.#values.set(key, value); }
}

const fallback = { graph: 68, diagnostics: 32 };

describe('pane layout persistence', () => {
  it('round trips a valid layout map', () => {
    const storage = new MemoryStorage();
    writePaneLayout(storage, 'layout', { graph: 65, diagnostics: 35 });
    expect(readPaneLayout(storage, 'layout', fallback)).toEqual({ graph: 65, diagnostics: 35 });
  });

  it('uses defaults for damaged or incompatible data', () => {
    const storage = new MemoryStorage();
    storage.setItem('layout', '{"graph":70}');
    expect(readPaneLayout(storage, 'layout', fallback)).toEqual(fallback);
    storage.setItem('layout', 'not json');
    expect(readPaneLayout(storage, 'layout', fallback)).toEqual(fallback);
  });
});
