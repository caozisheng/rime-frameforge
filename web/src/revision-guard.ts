import type { RuntimeEnvelope } from './contracts.js';

export function acceptsEnvelope(
  current: RuntimeEnvelope | null,
  incoming: RuntimeEnvelope,
): boolean {
  if (current === null) {
    return true;
  }
  if (incoming.graphInstanceId !== current.graphInstanceId) {
    return incoming.graphInstanceId > current.graphInstanceId;
  }
  if (incoming.gpuGeneration !== current.gpuGeneration) {
    return incoming.gpuGeneration > current.gpuGeneration;
  }
  if (incoming.methodRevision !== current.methodRevision) {
    return incoming.methodRevision > current.methodRevision;
  }
  if (incoming.configRevision !== current.configRevision) {
    return incoming.configRevision > current.configRevision;
  }
  return incoming.runRevision >= current.runRevision;
}
