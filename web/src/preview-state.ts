export const ZOOM_LEVELS = [0.25, 0.5, 1, 2, 4, 8] as const;

export interface PreviewPoint {
  readonly x: number;
  readonly y: number;
}

export interface PreviewSize {
  readonly width: number;
  readonly height: number;
}

export interface PreviewIdentity {
  readonly frameIndex: number;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly gpuGeneration: number;
}

export function zoomIn(value: number): number {
  const index = ZOOM_LEVELS.findIndex((level) => level > value);
  return index === -1 ? 8 : ZOOM_LEVELS[index] ?? 8;
}

export function zoomOut(value: number): number {
  const index = [...ZOOM_LEVELS].reverse().findIndex((level) => level < value);
  return index === -1 ? 0.25 : ZOOM_LEVELS[ZOOM_LEVELS.length - 1 - index] ?? 0.25;
}

export function wheelZoom(value: number, deltaY: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(deltaY)) return value;
  const next = value * 1.05 ** (-deltaY / 100);
  return Math.max(0.1, Math.min(16, next));
}

export function clampCurtain(value: number): number {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0.5));
}

export function clampPan(pan: PreviewPoint, image: PreviewSize, viewport: PreviewSize, zoom: number): PreviewPoint {
  const scaledWidth = image.width * zoom;
  const scaledHeight = image.height * zoom;
  const limitX = Math.max(0, (scaledWidth - viewport.width) / 2);
  const limitY = Math.max(0, (scaledHeight - viewport.height) / 2);
  return {
    x: Math.max(-limitX, Math.min(limitX, pan.x)),
    y: Math.max(-limitY, Math.min(limitY, pan.y)),
  };
}

export function zoomAroundPoint(pan: PreviewPoint, oldZoom: number, newZoom: number, point: PreviewPoint): PreviewPoint {
  const ratio = newZoom / oldZoom;
  return {
    x: point.x - (point.x - pan.x) * ratio,
    y: point.y - (point.y - pan.y) * ratio,
  };
}

export function fitZoom(image: PreviewSize, viewport: PreviewSize): number {
  if (image.width <= 0 || image.height <= 0 || viewport.width <= 0 || viewport.height <= 0) return 1;
  return Math.min(viewport.width / image.width, viewport.height / image.height);
}

export function imagePointAt(point: PreviewPoint, viewport: PreviewSize, image: PreviewSize, pan: PreviewPoint, zoom: number): { readonly x: number; readonly y: number } | null {
  const x = (point.x - viewport.width / 2 - pan.x) / zoom + image.width / 2;
  const y = (point.y - viewport.height / 2 - pan.y) / zoom + image.height / 2;
  if (x < 0 || y < 0 || x >= image.width || y >= image.height) return null;
  return { x: Math.floor(x), y: Math.floor(y) };
}

export function resizePreviewCanvas(canvas: { width: number; height: number }, extent: PreviewSize): void {
  if (canvas.width === extent.width && canvas.height === extent.height) return;
  canvas.width = extent.width;
  canvas.height = extent.height;
}

export function previewIdentityMismatch(a: PreviewIdentity, b: PreviewIdentity): string | null {
  if (a.frameIndex !== b.frameIndex) return `frame mismatch: ${a.frameIndex} vs ${b.frameIndex}`;
  if (a.runRevision !== b.runRevision) return `run revision mismatch: ${a.runRevision} vs ${b.runRevision}`;
  if (a.methodRevision !== b.methodRevision) return `method revision mismatch: ${a.methodRevision} vs ${b.methodRevision}`;
  if (a.gpuGeneration !== b.gpuGeneration) return `GPU generation mismatch: ${a.gpuGeneration} vs ${b.gpuGeneration}`;
  return null;
}
