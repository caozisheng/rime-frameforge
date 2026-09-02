import { describe, expect, it } from 'vitest';

import {
  clampCurtain,
  clampPan,
  fitZoom,
  imagePointAt,
  resizePreviewCanvas,
  previewIdentityMismatch,
  zoomAroundPoint,
  zoomIn,
  zoomOut,
  wheelZoom,
  ZOOM_LEVELS,
} from '../src/preview-state.js';

describe('preview interaction state', () => {
  it('steps through the discrete zoom levels and clamps at both ends', () => {
    expect(ZOOM_LEVELS).toEqual([0.25, 0.5, 1, 2, 4, 8]);
    expect(zoomOut(0.25)).toBe(0.25);
    expect(zoomIn(8)).toBe(8);
    expect(zoomIn(1)).toBe(2);
    expect(zoomOut(1)).toBe(0.5);
  });

  it('maps raw mouse-wheel delta to fine five-percent zoom steps', () => {
    expect(wheelZoom(1, -100)).toBeCloseTo(1.05);
    expect(wheelZoom(1, 100)).toBeCloseTo(1 / 1.05);
    expect(wheelZoom(0.1, 100)).toBe(0.1);
  });

  it('keeps the image coordinate under the pointer while zooming', () => {
    expect(zoomAroundPoint({ x: 20, y: 30 }, 1, 2, { x: 100, y: 80 })).toEqual({ x: -60, y: -20 });
  });

  it('clamps pan to the visible image bounds and disables undersized axes', () => {
    expect(clampPan({ x: 500, y: -100 }, { width: 400, height: 200 }, { width: 100, height: 100 }, 2)).toEqual({ x: 350, y: -100 });
    expect(clampPan({ x: 20, y: 20 }, { width: 50, height: 100 }, { width: 100, height: 100 }, 1)).toEqual({ x: 0, y: 0 });
  });

  it('fits the complete native extent and maps viewport hover to a sample coordinate', () => {
    expect(fitZoom({ width: 100, height: 50 }, { width: 400, height: 200 })).toBe(4);
    expect(fitZoom({ width: 4000, height: 2000 }, { width: 400, height: 200 })).toBe(0.1);
    expect(imagePointAt({ x: 200, y: 100 }, { width: 400, height: 200 }, { width: 100, height: 50 }, { x: 0, y: 0 }, 2)).toEqual({ x: 50, y: 25 });
    expect(imagePointAt({ x: 0, y: 0 }, { width: 400, height: 200 }, { width: 100, height: 50 }, { x: 0, y: 0 }, 2)).toBeNull();
  });

  it('keeps the GPU canvas backing store at the native sample extent', () => {
    let assignments = 0;
    const canvas = { _width: 640, _height: 480, get width() { return this._width; }, set width(value: number) { assignments += 1; this._width = value; }, get height() { return this._height; }, set height(value: number) { assignments += 1; this._height = value; } };
    resizePreviewCanvas(canvas, { width: 640, height: 480 });
    expect(assignments).toBe(0);
    resizePreviewCanvas(canvas, { width: 3744, height: 2800 });
    expect(assignments).toBe(2);
    expect({ width: canvas.width, height: canvas.height }).toEqual({ width: 3744, height: 2800 });
  });

  it('clamps the compare curtain and supports its center reset', () => {
    expect(clampCurtain(-0.2)).toBe(0);
    expect(clampCurtain(1.2)).toBe(1);
    expect(clampCurtain(0.5)).toBe(0.5);
  });

  it('reports incompatible compare identities without treating them as valid', () => {
    const a = { frameIndex: 1, runRevision: 2, methodRevision: 3, gpuGeneration: 4 };
    expect(previewIdentityMismatch(a, { ...a })).toBeNull();
    expect(previewIdentityMismatch(a, { ...a, frameIndex: 2 })).toContain('frame');
  });
});
