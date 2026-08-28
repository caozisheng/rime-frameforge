export type PaneLayout = Readonly<Record<string, number>>;

export function readPaneLayout(storage: Storage, key: string, fallback: PaneLayout): Record<string, number> {
  const value = storage.getItem(key);
  if (value === null) return { ...fallback };
  try {
    const parsed: unknown = JSON.parse(value);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return { ...fallback };
    const candidate = parsed as Record<string, unknown>;
    const expectedKeys = Object.keys(fallback);
    if (Object.keys(candidate).length !== expectedKeys.length) return { ...fallback };
    if (!expectedKeys.every((key) => typeof candidate[key] === 'number' && Number.isFinite(candidate[key]) && candidate[key] >= 0)) {
      return { ...fallback };
    }
    const layout = Object.fromEntries(expectedKeys.map((key) => [key, candidate[key] as number]));
    const total = Object.values(layout).reduce((sum, size) => sum + size, 0);
    return Math.abs(total - 100) <= 0.5 ? layout : { ...fallback };
  } catch {
    return { ...fallback };
  }
}

export function writePaneLayout(storage: Storage, key: string, layout: PaneLayout): void {
  storage.setItem(key, JSON.stringify(layout));
}
