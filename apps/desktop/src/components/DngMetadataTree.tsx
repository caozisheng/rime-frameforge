import type { LifecycleState } from '../../../../web/src/contracts.js';
import type { DngFrameDescriptor } from '../runtime/worker-bridge.js';
import { buildDngMetadataGroups } from './dng-metadata.js';
import { InspectorTree, readInspectorExpanded, readInspectorFontSize, type InspectorTreeGroup } from './InspectorTree.js';

export const DNG_FONT_SIZE_KEY = 'rime:dng-inspector:font-size:v1';
export const DNG_EXPANDED_KEY = 'rime:dng-inspector:expanded:v1';
export const DEFAULT_DNG_EXPANDED = ['runtime', 'frame', 'image', 'sensor'] as const;

interface DngMetadataTreeProps { readonly descriptor: DngFrameDescriptor; readonly lifecycleState: LifecycleState; readonly frameIndex: number | null; readonly frameCount: number }
export function readDngFontSize(storage: Pick<Storage, 'getItem'>): number { return readInspectorFontSize(storage, 'rime:dng-inspector'); }
export function readDngExpanded(storage: Pick<Storage, 'getItem'>): Set<string> {
  const groups = DEFAULT_DNG_EXPANDED.map((id) => ({ id, label: id, defaultExpanded: true, children: [] }));
  return readInspectorExpanded(storage, 'rime:dng-inspector', groups);
}


export function DngMetadataTree({ descriptor, lifecycleState, frameIndex, frameCount }: DngMetadataTreeProps) {
  const groups: readonly InspectorTreeGroup[] = buildDngMetadataGroups(descriptor);
  const visibleFrame = frameIndex ?? descriptor.frameIndex;
  const frameLabel = `Frame ${String(visibleFrame + 1).padStart(3, '0')}${frameCount > 1 ? ` / ${String(frameCount).padStart(3, '0')}` : ''}`;
  return <div className="dng-inspector-body">
    <div className="dng-frame-context"><div><span className={`dng-state state-${lifecycleState}`}>{lifecycleState.toUpperCase()}</span><strong>{frameLabel}</strong></div><span title={descriptor.fileName}>{descriptor.fileName}</span></div>
    <InspectorTree ariaLabel="DNG metadata" groups={groups} storageKey="rime:dng-inspector" />
  </div>;
}
