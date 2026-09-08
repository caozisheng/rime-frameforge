import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';

import { App, DEFAULT_DRC_IQ_PARAMETERS, drcIqParametersFromValues, hasDrcIqDraft } from '../src/App.js';

const styles = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8');
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

  it('uses the full viewport for the application shell in every mode', () => {
    const shellRule = styles.match(/\.app-shell\s*\{([^}]*)\}/)?.[1] ?? '';
    const focusedShellRule = styles.match(/\.app-shell\.is-preview-focused\s*\{([^}]*)\}/)?.[1] ?? '';

    expect(shellRule).toContain('width: 100vw');
    expect(shellRule).toContain('height: 100vh');
    expect(shellRule).not.toContain('aspect-ratio: 16 / 9');
    expect(shellRule).not.toContain('margin: 12px auto');
    expect(focusedShellRule).toContain('width: 100vw');
    expect(focusedShellRule).toContain('height: 100vh');
    expect(focusedShellRule).not.toContain('calc(100vw - 24px)');
    expect(focusedShellRule).not.toContain('calc(100vh - 24px)');
  });
  it('defines complete DRC IQ defaults and draft detection', () => {
    expect(DEFAULT_DRC_IQ_PARAMETERS).toEqual({ drc_gain_offset_ev: 0, knee: 1, amplifier: 3 });
    expect(drcIqParametersFromValues({})).toEqual(DEFAULT_DRC_IQ_PARAMETERS);
    expect(drcIqParametersFromValues({ drc_gain_offset_ev: -2, knee: 3, amplifier: 4 })).toEqual({ drc_gain_offset_ev: -2, knee: 3, amplifier: 4 });
    expect(hasDrcIqDraft({ drc_gain_offset_ev: 0, knee: 1, amplifier: 2 }, DEFAULT_DRC_IQ_PARAMETERS)).toBe(true);
  });
});
