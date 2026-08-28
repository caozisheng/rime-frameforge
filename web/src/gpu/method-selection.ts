import type { normalManifest } from '../generated/normal_manifest.generated.js';

type ManifestNode = (typeof normalManifest.nodes)[number];

export function resolveShaderEntry(
  node: ManifestNode,
  selectedMethods: Readonly<Record<string, string>>,
): string | null {
  if (node.shader_entry === null) return null;
  const selectedMethod = selectedMethods[node.id] ?? node.default_method;
  return node.methods.find((method) => method.method === selectedMethod)?.shader_entry ?? node.shader_entry;
}
