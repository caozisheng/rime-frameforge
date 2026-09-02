import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';

import { App } from '../src/App.js';

class MemoryStorage implements Storage {
  readonly #values = new Map<string, string>();
  get length(): number { return this.#values.size; }
  clear(): void { this.#values.clear(); }
  getItem(key: string): string | null { return this.#values.get(key) ?? null; }
  key(index: number): string | null { return [...this.#values.keys()][index] ?? null; }
  removeItem(key: string): void { this.#values.delete(key); }
  setItem(key: string, value: string): void { this.#values.set(key, value); }
}

beforeEach(() => {
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { localStorage: new MemoryStorage() },
  });
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: window.localStorage,
  });
});

describe('desktop workspace layout', () => {
  it('places transport in the graph header and trace below the graph', () => {
    const html = renderToStaticMarkup(<App />);
    const graphHeading = html.indexOf('id="graph-heading"');
    const transport = html.indexOf('class="transport-toolbar"');
    const graphCanvas = html.indexOf('class="react-flow-canvas"');
    const trace = html.indexOf('id="logs-heading"');
    const inspector = html.indexOf('id="inspector-heading"');
    const preview = html.indexOf('id="preview-heading"');

    expect(graphHeading).toBeGreaterThan(-1);
    expect(transport).toBeGreaterThan(graphHeading);
    expect(transport).toBeLessThan(graphCanvas);
    expect(trace).toBeGreaterThan(graphCanvas);
    expect(trace).toBeLessThan(inspector);
    expect(preview).toBeGreaterThan(inspector);
  });

  it('provides Preview focus entry while keeping the GPU canvas mounted', () => {
    const html = renderToStaticMarkup(<App />);

    expect(html).toContain('class="app-shell"');
    expect(html).toContain('Focus Preview');
    expect(html).toContain('aria-label="Normal Graph GPU preview"');
  });
});
