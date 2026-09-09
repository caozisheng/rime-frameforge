import { useEffect, useRef, useState, type ReactNode } from 'react';

import type { ColorReproduceAssets } from '../../../../../web/src/contracts.js';

interface HsLutPanelProps {
  readonly assets: ColorReproduceAssets | null | undefined;
}

export interface HsLutGrid {
  readonly hueDivs: number;
  readonly satDivs: number;
  readonly lut: readonly number[];
}

// DNG grid order with ValueDivs == 1: hue outer, saturation inner, three
// floats per entry (hueShift deg, satScale, valScale).
export function hsLutGrid(assets: ColorReproduceAssets): HsLutGrid | null {
  const lut = assets.hsLut;
  if (lut === undefined || lut === null || lut.length === 0) return null;
  const [hueDivs, satDivs] = assets.hsDims;
  if (hueDivs < 1 || satDivs < 1 || lut.length !== 3 * hueDivs * satDivs) return null;
  return { hueDivs, satDivs, lut };
}

export interface HsLutSample {
  readonly hueShift: number;
  readonly satScale: number;
  readonly valScale: number;
  readonly hueBin: number;
  readonly satBin: number;
}

// Nearest-neighbour lookup, identical to the WGSL hs_lut_apply semantics:
// h = min(floor(hue/360 * H), H-1); s = min(floor(sat * S), S-1); v index 0.
export function sampleHsLut(grid: HsLutGrid, hueDeg: number, sat: number): HsLutSample {
  const hueBin = Math.min(Math.floor(((hueDeg % 360) + 360) % 360 / 360 * grid.hueDivs), grid.hueDivs - 1);
  const satBin = Math.min(Math.floor(Math.min(Math.max(sat, 0), 1) * grid.satDivs), grid.satDivs - 1);
  const base = 3 * (hueBin * grid.satDivs + satBin);
  return {
    hueShift: grid.lut[base] ?? 0,
    satScale: grid.lut[base + 1] ?? 1,
    valScale: grid.lut[base + 2] ?? 1,
    hueBin,
    satBin,
  };
}

// Apply the LUT entry to an input HSV triple (v fixed by the wheel layout).
export function applyHsLut(hueDeg: number, sat: number, val: number, sample: HsLutSample): { hue: number; sat: number; val: number } {
  return {
    hue: (((hueDeg + sample.hueShift) % 360) + 360) % 360,
    sat: Math.min(Math.max(sat * sample.satScale, 0), 1),
    val: Math.min(Math.max(val * sample.valScale, 0), 1),
  };
}
export type HsLutWheelSource = 'original' | 'corrected' | 'delta';

export const HS_LUT_WHEEL_SOURCES: readonly { readonly id: HsLutWheelSource; readonly label: string; readonly title: string }[] = [
  { id: 'original', label: 'original', title: 'evenly sampled input HSV(h, s, 1) shown without LUT lookup' },
  { id: 'corrected', label: 'corrected', title: "sampled (h, s) pushed through the HS LUT lookup, HSV(h', s', v') shown" },
  { id: 'delta', label: 'delta', title: 'monochrome grayscale wheel: darker = larger per-point dCab (CIELAB chroma difference) between input and LUT-corrected colours; white = identity' },
];


// HSV -> sRGB 8-bit (standard sector math, h in deg, s/v in [0,1]).
export function hsvToRgb8(hueDeg: number, sat: number, val: number): { r: number; g: number; b: number } {
  const h = (((hueDeg % 360) + 360) % 360) / 60;
  const s = Math.min(Math.max(sat, 0), 1);
  const v = Math.min(Math.max(val, 0), 1);
  const c = v * s;
  const x = c * (1 - Math.abs((h % 2) - 1));

  let rgb: [number, number, number];
  if (h < 1) rgb = [c, x, 0];
  else if (h < 2) rgb = [x, c, 0];
  else if (h < 3) rgb = [0, c, x];
  else if (h < 4) rgb = [0, x, c];
  else if (h < 5) rgb = [x, 0, c];
  else rgb = [c, 0, x];
  const m = v - c;
  return { r: Math.round((rgb[0] + m) * 255), g: Math.round((rgb[1] + m) * 255), b: Math.round((rgb[2] + m) * 255) };
}

export function hsLutBypassReason(assets: ColorReproduceAssets | null | undefined): string | null {
  if (assets === null || assets === undefined) return 'no frame loaded';
  if (!assets.hsEnable) return 'disabled (no profile map)';
  if (assets.hsLut === undefined || assets.hsLut === null || assets.hsLut.length === 0) return 'bypassed (ValueDivs>1 or missing table)';
  return null;
}

// Canvas wheel: 0deg hue at 12 o'clock, clockwise; saturation is the
// radius (center 0 -> rim 1); v fixed at 1. Every pixel evaluates the LUT
// exactly like the shader does and shows the corrected colour.
const SIZE = 512;

interface WheelPixel {
  readonly hue: number;
  readonly sat: number;
}

function pixelToHs(x: number, y: number): { readonly r: number; readonly hs: WheelPixel | null } {
  const dx = x - SIZE / 2;
  const dy = y - SIZE / 2;
  const r = Math.hypot(dx, dy);
  const rMax = SIZE / 2 - 1;
  if (r > rMax) return { r, hs: null };
  // theta measured from 12 o'clock, clockwise: atan2(dx, -dy) in [-pi, pi].
  const hue = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
  return { r, hs: { hue, sat: r / rMax } };
}

// sRGB 8-bit -> CIELAB chroma C_ab, computed in native D65 Lab: inverse
// sRGB transfer, sRGB(D65)->XYZ, then the CIE L*a*b* forward transform
// with the D65 reference white. No D50 CAT — dCab is an auto-gain ratio
// metric, so the working space only sets a constant scale.
export function srgb8ToCab(r8: number, g8: number, b8: number): number {
  const linear = (v8: number): number => {
    const c = v8 / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const rl = linear(r8);
  const gl = linear(g8);
  const bl = linear(b8);
  const x = 0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl;
  const y = 0.2126729 * rl + 0.7151522 * gl + 0.072175 * bl;
  const z = 0.0193339 * rl + 0.119192 * gl + 0.9503041 * bl;
  // XYZ(D65) -> L*a*b*, D65 reference white (0.95047, 1.0, 1.08883).
  const Xn = 0.95047;
  const Yn = 1;
  const Zn = 1.08883;
  const eps = 216 / 24389;
  const kappa = 24389 / 27;
  const f = (t: number): number => (t > eps ? Math.cbrt(t) : (kappa * t + 16) / 116);
  const fx = f(x / Xn);
  const fy = f(y / Yn);
  const fz = f(z / Zn);
  const a = 500 * (fx - fy);
  const b = 200 * (fy - fz);
  return Math.hypot(a, b);
}

// Per-bin dCab: the chroma difference the LUT applies to the bin's centre
// sample. This is a property of the LUT cell, so it is computed once per
// bin and cached — pixels falling into the bin reuse the cached value.
export function hsLutDeltaCab(grid: HsLutGrid, hs: WheelPixel): number {
  const sample = sampleHsLut(grid, hs.hue, hs.sat);
  const corrected = applyHsLut(hs.hue, hs.sat, 1, sample);
  const inRgb = hsvToRgb8(hs.hue, hs.sat, 1);
  const outRgb = hsvToRgb8(corrected.hue, corrected.sat, corrected.val);
  return Math.abs(srgb8ToCab(outRgb.r, outRgb.g, outRgb.b) - srgb8ToCab(inRgb.r, inRgb.g, inRgb.b));
}

export interface HsLutDeltaTable {
  readonly grid: HsLutGrid;
  readonly deltas: readonly number[];   // H*S, DNG grid order (hue outer)
  readonly max: number;                 // peak dCab, 0 when identity
}

const deltaTableCache = new WeakMap<HsLutGrid, HsLutDeltaTable>();

export function hsLutDeltaTable(grid: HsLutGrid): HsLutDeltaTable {
  const cached = deltaTableCache.get(grid);
  if (cached !== undefined) return cached;
  const deltas: number[] = [];
  let max = 0;
  for (let hueBin = 0; hueBin < grid.hueDivs; hueBin++) {
    for (let satBin = 0; satBin < grid.satDivs; satBin++) {
      const delta = hsLutDeltaCab(grid, { hue: ((hueBin + 0.5) / grid.hueDivs) * 360, sat: (satBin + 0.5) / grid.satDivs });
      deltas.push(delta);
      if (delta > max) max = delta;
    }
  }
  const table = { grid, deltas, max };
  deltaTableCache.set(grid, table);
  return table;
}

// Peak dCab across the grid (auto-gain reference). Kept for tests/meta.
export function hsLutMaxDeltaCab(grid: HsLutGrid): number {
  return hsLutDeltaTable(grid).max;
}

export function wheelPixelColor(grid: HsLutGrid, hs: WheelPixel, source: HsLutWheelSource = 'corrected'): { r: number; g: number; b: number } {
  if (source === 'original') {
    // Reference view: the evenly sampled input colour, no LUT lookup.
    return hsvToRgb8(hs.hue, hs.sat, 1);
  }
  if (source === 'delta') {
    // Monochrome grayscale wheel: darker means a larger per-bin dCab,
    // lighter means smaller — identity bins are pure white and the peak
    // bin is the darkest (auto-gain maps it to black). Alpha is clamped
    // by construction (ratio of a table entry to the table max).
    const table = hsLutDeltaTable(grid);
    const sample = sampleHsLut(grid, hs.hue, hs.sat);
    const alpha = table.max <= 0 ? 0 : Math.min((table.deltas[sample.hueBin * grid.satDivs + sample.satBin] ?? 0) / table.max, 1);
    const gray = Math.round(255 * (1 - alpha));
    return { r: gray, g: gray, b: gray };
  }
  const sample = sampleHsLut(grid, hs.hue, hs.sat);
  const corrected = applyHsLut(hs.hue, hs.sat, 1, sample);
  return hsvToRgb8(corrected.hue, corrected.sat, corrected.val);
}

export function HsLutPanel({ assets }: HsLutPanelProps): ReactNode {
  const reason = hsLutBypassReason(assets);
  const grid = assets === null || assets === undefined ? null : hsLutGrid(assets);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [picked, setPicked] = useState<{ readonly x: number; readonly y: number } | null>(null);

  const [source, setSource] = useState<HsLutWheelSource>('original');
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null || grid === null) return;
    const ctx = canvas.getContext('2d');
    if (ctx === null) return;
    const image = ctx.createImageData(SIZE, SIZE);
    const data = image.data;
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        const { hs } = pixelToHs(x + 0.5, y + 0.5);
        const offset = (y * SIZE + x) * 4;
        if (hs === null) {
          data[offset + 3] = 0; // outside the disc: transparent
          continue;
        }
        const { r, g, b } = wheelPixelColor(grid, hs, source);
        data[offset] = r;
        data[offset + 1] = g;
        data[offset + 2] = b;
        data[offset + 3] = 255;
      }
    }
    ctx.putImageData(image, 0, 0);
  }, [grid, source]);

  const readout = (() => {
    if (grid === null || picked === null) return null;
    const { hs } = pixelToHs(picked.x + 0.5, picked.y + 0.5);
    if (hs === null) return null;
    const sample = sampleHsLut(grid, hs.hue, hs.sat);
    const corrected = applyHsLut(hs.hue, hs.sat, 1, sample);
    const rgb = wheelPixelColor(grid, hs, source);
    const hex = `#${[rgb.r, rgb.g, rgb.b].map((c) => c.toString(16).padStart(2, '0')).join('')}`;
    return { hs, sample, corrected, rgb, hex };
  })();


  return <section aria-label="IQ LUT hs_lut" className="iq-tuning-panel" data-iq-parameter="hs_lut">
    <div className="iq-tuning-heading">
      <div>
        <span className="section-label">IQ LUT · read-only</span>
        <strong>hs_lut</strong>
        <small>lut_2d · preprocess frozen</small>
      </div>
      <span className="tree-mode-badge mode-enabled">
        {grid === null ? 'bypassed' : `${grid.hueDivs}×${grid.satDivs}`}
      </span>
    </div>
    {grid === null ? <div className="dng-empty-state"><strong>HS calibration unavailable</strong><span>{reason ?? 'no data'}</span></div> : <>
      <div className="iq-hslut-wheel">
      <div className="iq-hslut-sources" role="group" aria-label="hs_lut wheel source">
        {HS_LUT_WHEEL_SOURCES.map((entry) => <button aria-pressed={source === entry.id} className={`iq-hslut-source${source === entry.id ? ' is-active' : ''}`} key={entry.id} onClick={() => setSource(entry.id)} title={entry.title} type="button">{entry.label}</button>)}
      </div>
        <canvas
          aria-label="hs_lut color wheel"
          className="iq-hslut-canvas"
          data-hue-divs={grid.hueDivs}
          data-sat-divs={grid.satDivs}
          height={SIZE}
          onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            setPicked({ x: event.clientX - bounds.left, y: event.clientY - bounds.top });
          }}
          ref={canvasRef}
          width={SIZE}
        />
        <span className="iq-hslut-axis iq-hslut-axis-x">hue 0° → 360° clockwise · radius = saturation 0 → 1 · v = 1</span>
      </div>
      <div className="iq-hslut-meta">
        <span>grid</span><strong>H {grid.hueDivs} × S {grid.satDivs} · ValueDivs 1</strong>
        <span>max ΔC_ab</span><strong data-testid="hs-lut-max-delta">{hsLutMaxDeltaCab(grid).toFixed(2)}</strong>
        {readout === null ? null : <>
          <span>input</span><strong>h {readout.hs.hue.toFixed(1)}° · s {readout.hs.sat.toFixed(3)}</strong>
          <span>bin</span><strong>({readout.sample.hueBin}, {readout.sample.satBin})</strong>
          <span>entry</span><strong>hueShift {readout.sample.hueShift.toFixed(4)}° · satScale {readout.sample.satScale.toFixed(4)} · valScale {readout.sample.valScale.toFixed(4)}</strong>
          <span>corrected</span><strong>h {readout.corrected.hue.toFixed(1)}° · s {readout.corrected.sat.toFixed(3)} · v {readout.corrected.val.toFixed(3)}</strong>
          <span>rgb</span><strong data-testid="hs-lut-picked-rgb">{readout.rgb.r}, {readout.rgb.g}, {readout.rgb.b} · {readout.hex}</strong>
        </>}
      </div>
    </>}
  </section>;
}
