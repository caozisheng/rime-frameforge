export type NormalNodeAppearance = 'implemented' | 'neutral';

export function normalNodeAppearance(
  kind: 'group' | 'operator' | 'endpoint',
  mode: 'enabled' | 'bypass' | 'disabled',
): NormalNodeAppearance {
  return kind === 'operator' && mode === 'enabled' ? 'implemented' : 'neutral';
}
