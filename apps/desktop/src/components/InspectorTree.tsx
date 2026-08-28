import { useState, type CSSProperties, type ReactNode } from 'react';

import {
  DEFAULT_DNG_FONT_SIZE,
  MAX_DNG_FONT_SIZE,
  MIN_DNG_FONT_SIZE,
  clampDngFontSize,
} from './dng-metadata.js';

export interface InspectorTreeNode {
  readonly id: string;
  readonly label: string;
  readonly value?: string;
  readonly summary?: string;
  readonly control?: ReactNode;
  readonly children?: readonly InspectorTreeNode[];
}

export interface InspectorTreeGroup extends InspectorTreeNode {
  readonly children: readonly InspectorTreeNode[];
  readonly defaultExpanded: boolean;
}

interface InspectorTreeProps {
  readonly ariaLabel: string;
  readonly groups: readonly InspectorTreeGroup[];
  readonly storageKey: string;
}

function storage(): Storage | null {
  return typeof window === 'undefined' ? null : window.localStorage;
}

export function readInspectorFontSize(source: Pick<Storage, 'getItem'> | null, key: string): number {
  const stored = Number.parseInt(source?.getItem(`${key}:font-size:v1`) ?? '', 10);
  return clampDngFontSize(Number.isNaN(stored) ? undefined : stored);
}

export function readInspectorExpanded(source: Pick<Storage, 'getItem'> | null, key: string, groups: readonly InspectorTreeGroup[]): Set<string> {
  try {
    const parsed: unknown = JSON.parse(source?.getItem(`${key}:expanded:v1`) ?? 'null');
    if (Array.isArray(parsed) && parsed.every((value) => typeof value === 'string')) return new Set(parsed);
  } catch { /* use group defaults */ }
  return new Set(groups.filter((group) => group.defaultExpanded).map((group) => group.id));
}

function TreeNode({ node, depth, expanded, onToggle }: { readonly node: InspectorTreeNode; readonly depth: number; readonly expanded: Set<string>; readonly onToggle: (id: string) => void }) {
  const hasChildren = (node.children?.length ?? 0) > 0;
  const isExpanded = hasChildren && expanded.has(node.id);
  const displayValue = node.summary ?? node.value ?? '—';
  const [showFull, setShowFull] = useState(false);
  const [copied, setCopied] = useState(false);
  const copyValue = (): void => {
    if (node.control !== undefined || typeof navigator === 'undefined') return;
    void navigator.clipboard.writeText(node.value ?? node.summary ?? '').then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 900);
    });
  };
  return <li role="treeitem" aria-expanded={hasChildren ? isExpanded : undefined}>
    <div className="dng-tree-row" style={{ paddingLeft: `${8 + depth * 12}px` }}>
      <button className="dng-tree-chevron" disabled={!hasChildren} onClick={() => onToggle(node.id)} type="button" aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${node.label}`}>{hasChildren ? (isExpanded ? '▾' : '▸') : '·'}</button>
      <span className="dng-tree-key">{node.label}</span>
      {node.control === undefined
        ? <button className={`dng-tree-value${showFull ? ' is-expanded' : ''}`} title={displayValue} type="button" onClick={() => setShowFull((current) => !current)} onDoubleClick={copyValue}>{copied ? 'Copied' : displayValue}</button>
        : <span className="inspector-tree-control">{node.control}</span>}
    </div>
    {isExpanded && <ul role="group">{node.children?.map((child) => <TreeNode key={child.id} node={child} depth={depth + 1} expanded={expanded} onToggle={onToggle} />)}</ul>}
  </li>;
}

export function InspectorTree({ ariaLabel, groups, storageKey }: InspectorTreeProps) {
  const [fontSize, setFontSize] = useState(() => readInspectorFontSize(storage(), storageKey));
  const [expanded, setExpanded] = useState(() => readInspectorExpanded(storage(), storageKey, groups));
  const changeFontSize = (next: number): void => {
    const clamped = clampDngFontSize(next);
    setFontSize(clamped);
    storage()?.setItem(`${storageKey}:font-size:v1`, String(clamped));
  };
  const persistExpanded = (next: Set<string>): void => {
    setExpanded(next);
    storage()?.setItem(`${storageKey}:expanded:v1`, JSON.stringify([...next]));
  };
  const toggle = (id: string): void => {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id); else next.add(id);
    persistExpanded(next);
  };
  const reset = (): void => {
    changeFontSize(DEFAULT_DNG_FONT_SIZE);
    persistExpanded(new Set(groups.filter((group) => group.defaultExpanded).map((group) => group.id)));
  };

  return <div className="inspector-tree-body">
    <div className="dng-tree-toolbar" aria-label={`${ariaLabel} display controls`}><button disabled={fontSize <= MIN_DNG_FONT_SIZE} onClick={() => changeFontSize(fontSize - 1)} type="button">A−</button><output>{fontSize}px</output><button disabled={fontSize >= MAX_DNG_FONT_SIZE} onClick={() => changeFontSize(fontSize + 1)} type="button">A+</button><button className="dng-tree-reset" onClick={reset} type="button">Reset</button></div>
    <div className="dng-tree-scroll" style={{ '--dng-tree-font-size': `${fontSize}px` } as CSSProperties}><ul className="dng-tree-root" role="tree" aria-label={ariaLabel}>{groups.map((group) => <TreeNode key={group.id} node={{ ...group, summary: group.summary ?? `${group.children.length}` }} depth={0} expanded={expanded} onToggle={toggle} />)}</ul></div>
  </div>;
}
