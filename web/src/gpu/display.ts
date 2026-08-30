export function s0ToS8Display(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 0;
  return Math.min(Math.trunc(value * 256), 255);
}
