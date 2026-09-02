import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';

import type { PreviewDescriptor } from '../../../../web/src/contracts.js';
import { clampCurtain, clampPan, fitZoom, imagePointAt, previewIdentityMismatch, wheelZoom, zoomAroundPoint, zoomIn, zoomOut } from '../../../../web/src/preview-state.js';

export interface PreviewNodeOption {
  readonly id: string;
  readonly label: string;
}

export interface NativePreviewDescriptor {
  readonly dataUrl: string;
  readonly width: number;
  readonly height: number;
  readonly outputWidth: number;
  readonly outputHeight: number;
  readonly nodeId: string;
  readonly portId: string;
  readonly frameIndex: number;
}
export interface PreviewSampleValue {
  readonly nodeId: string;
  readonly x: number;
  readonly y: number;
  readonly values: readonly number[];
}


interface PreviewSurfaceProps {
  readonly canvasRef: React.RefObject<HTMLCanvasElement | null>;
  readonly previews: readonly PreviewDescriptor[];
  readonly nativePreview: NativePreviewDescriptor | null;
  readonly fileName: string | null;
  readonly frameCount: number;
  readonly sample: PreviewSampleValue | null;
  readonly mode: 'final' | 'selected' | 'compare';
  readonly selectedNode: string | null;
  readonly nodeOptions: readonly PreviewNodeOption[];
  readonly previewCapabilities: Readonly<Record<string, true>>;
  readonly compareA: string | null;
  readonly compareB: string | null;
  readonly focused: boolean;
  readonly onModeChange: (mode: 'final' | 'selected' | 'compare') => void;
  readonly onCompareAChange: (nodeId: string) => void;
  readonly onCompareBChange: (nodeId: string) => void;
  readonly onFocusedChange: (focused: boolean) => void;
  readonly onPresentationChange: (nodeA: string, nodeB: string | null, curtain: number) => void;
  readonly onSampleRequest: (nodeId: string, x: number, y: number) => void;
}

interface PanState {
  readonly x: number;
  readonly y: number;
}

export function PreviewSurface({ canvasRef, previews, nativePreview, fileName, frameCount, sample, mode, selectedNode, nodeOptions, previewCapabilities, compareA, compareB, focused, onModeChange, onCompareAChange, onCompareBChange, onFocusedChange, onPresentationChange, onSampleRequest }: PreviewSurfaceProps) {
  const surfaceRef = useRef<HTMLElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const imageViewportRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<PanState>({ x: 0, y: 0 });
  const [curtain, setCurtain] = useState(0.5);
  const [showSample, setShowSample] = useState(false);
  const [samplePoint, setSamplePoint] = useState<{ readonly x: number; readonly y: number; readonly nodeId: string } | null>(null);
  const dragRef = useRef<{ pointerId: number; origin: PanState; point: PanState } | null>(null);
  const curtainRef = useRef<number | null>(null);
  const isFinal = mode === 'final';
  const finalPreview = previews[0] ?? null;
  const selectedPreview = selectedNode === null ? null : previews.find((candidate) => candidate.nodeId === selectedNode) ?? null;
  const comparePreviewA = compareA === null ? null : previews.find((candidate) => candidate.nodeId === compareA) ?? null;
  const comparePreviewB = compareB === null ? null : previews.find((candidate) => candidate.nodeId === compareB) ?? null;
  const compareMismatch = comparePreviewA === null || comparePreviewB === null || compareA === compareB
    ? 'A and B must reference two independently selectable committed outputs.'
    : comparePreviewA.width !== comparePreviewB.width || comparePreviewA.height !== comparePreviewB.height
      ? `extent mismatch: ${comparePreviewA.width} × ${comparePreviewA.height} vs ${comparePreviewB.width} × ${comparePreviewB.height}`
      : previewIdentityMismatch(comparePreviewA, comparePreviewB);
  const preview = mode === 'selected' ? selectedPreview : mode === 'compare' ? comparePreviewA : finalPreview;
  const nativeSelected = mode === 'selected' && selectedNode === nativePreview?.nodeId;
  const extent = preview ?? (isFinal || nativeSelected ? nativePreview : null);
  const hasFrame = extent !== null;
  const displayPreview = isFinal ? hasFrame : mode === 'selected' ? selectedPreview !== null || nativeSelected : compareMismatch === null;
  const unavailableReason = mode === 'selected'
    ? selectedNode !== null && previewCapabilities[selectedNode] === true
      ? `No committed preview output is available for selected node ${selectedNode}.`
      : 'The selected node has no declared preview output for this frame.'
    : mode === 'compare'
      ? compareMismatch
      : null;
  useEffect(() => {
    if (previews.length === 0) return;
    if (mode === 'final' && finalPreview !== null) onPresentationChange(finalPreview.nodeId, null, curtain);
    else if (mode === 'selected' && selectedPreview !== null) onPresentationChange(selectedPreview.nodeId, null, curtain);
    else if (mode === 'compare' && compareMismatch === null && compareA !== null && compareB !== null) onPresentationChange(compareA, compareB, curtain);
  }, [compareA, compareB, compareMismatch, curtain, finalPreview, mode, onPresentationChange, previews.length, selectedPreview]);
  useEffect(() => {
    if (!showSample || samplePoint === null) return undefined;
    const timer = window.setTimeout(() => onSampleRequest(samplePoint.nodeId, samplePoint.x, samplePoint.y), 30);
    return () => window.clearTimeout(timer);
  }, [onSampleRequest, samplePoint, showSample]);

  const updatePan = (next: PanState): void => {
    const viewport = viewportRef.current;
    if (viewport === null || extent === null) return;
    setPan(clampPan(next, { width: extent.width, height: extent.height }, { width: viewport.clientWidth, height: viewport.clientHeight }, zoom));
  };
  const setToolbarZoom = (next: number): void => {
    const viewport = viewportRef.current;
    if (viewport !== null && extent !== null) {
      const anchored = zoomAroundPoint(pan, zoom, next, { x: 0, y: 0 });
      setPan(clampPan(anchored, { width: extent.width, height: extent.height }, { width: viewport.clientWidth, height: viewport.clientHeight }, next));
    }
    setZoom(next);
  };
  const fitPreview = (): void => {
    const viewport = viewportRef.current;
    if (viewport !== null && extent !== null) {
      setZoom(fitZoom({ width: extent.width, height: extent.height }, { width: viewport.clientWidth, height: viewport.clientHeight }));
    }
    setPan({ x: 0, y: 0 });
  };
  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport === null) return undefined;
    const handleWheel = (event: WheelEvent): void => {
      event.preventDefault();
      const nextZoom = wheelZoom(zoom, event.deltaY);
      if (nextZoom === zoom) return;
      const bounds = viewport.getBoundingClientRect();
      const point = { x: event.clientX - bounds.left - bounds.width / 2, y: event.clientY - bounds.top - bounds.height / 2 };
      const imageSize = extent ?? { width: bounds.width, height: bounds.height };
      setPan((current) => clampPan(zoomAroundPoint(current, zoom, nextZoom, point), imageSize, { width: bounds.width, height: bounds.height }, nextZoom));
      setZoom(nextZoom);
    };
    viewport.addEventListener('wheel', handleWheel, { passive: false });
    return () => viewport.removeEventListener('wheel', handleWheel);
  }, [extent, zoom]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport === null || extent === null || typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(([entry]) => {
      if (entry === undefined) return;
      setPan((current) => clampPan(current, { width: extent.width, height: extent.height }, { width: entry.contentRect.width, height: entry.contentRect.height }, zoom));
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [extent, zoom]);

  useEffect(() => {
    if (focused) {
      previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      surfaceRef.current?.focus();
      return undefined;
    }
    previousFocusRef.current?.focus();
    previousFocusRef.current = null;
    return undefined;
  }, [focused]);
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (mode === 'compare' && curtainRef.current !== null) return;
    if (event.button !== 0 || !displayPreview) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = { pointerId: event.pointerId, origin: pan, point: { x: event.clientX, y: event.clientY } };
  };
  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (showSample && extent !== null && preview !== null) {
      const bounds = event.currentTarget.getBoundingClientRect();
      const point = imagePointAt({ x: event.clientX - bounds.left, y: event.clientY - bounds.top }, { width: bounds.width, height: bounds.height }, { width: extent.width, height: extent.height }, pan, zoom);
      const curtainBounds = imageViewportRef.current?.getBoundingClientRect();
      const onB = mode === 'compare' && curtainBounds !== undefined && event.clientX > curtainBounds.left + curtainBounds.width * curtain;
      const nodeId = onB ? compareB : preview.nodeId;
      setSamplePoint(point === null || nodeId === null ? null : { ...point, nodeId });
    }
    const drag = dragRef.current;
    if (drag !== null && drag.pointerId === event.pointerId) {
      updatePan({ x: drag.origin.x + event.clientX - drag.point.x, y: drag.origin.y + event.clientY - drag.point.y });
    }
  };
  const finishPan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  };
  const handleCurtainPointerDown = (event: ReactPointerEvent<HTMLButtonElement>): void => {
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    curtainRef.current = event.pointerId;
  };
  const handleCurtainMove = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (curtainRef.current !== event.pointerId) return;
    const viewport = imageViewportRef.current;
    if (viewport === null) return;
    const bounds = viewport.getBoundingClientRect();
    setCurtain(clampCurtain((event.clientX - bounds.left) / Math.max(bounds.width, 1)));
  };
  const finishCurtain = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (curtainRef.current === event.pointerId) curtainRef.current = null;
  };
  const moveCurtainByKey = (event: ReactKeyboardEvent<HTMLButtonElement>): void => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    setCurtain((current) => clampCurtain(current + (event.key === 'ArrowRight' ? 0.01 : -0.01)));
  };
  const handlePreviewKey = (event: ReactKeyboardEvent<HTMLElement>): void => {
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      setToolbarZoom(zoomIn(zoom));
    } else if (event.key === '-') {
      event.preventDefault();
      setToolbarZoom(zoomOut(zoom));
    } else if (event.key === '1') {
      event.preventDefault();
      setToolbarZoom(1);
    }
  };
  const movePointer = (event: ReactPointerEvent<HTMLDivElement>): void => {
    handleCurtainMove(event);
    handlePointerMove(event);
  };
  const finishPointer = (event: ReactPointerEvent<HTMLDivElement>): void => {
    finishCurtain(event);
    finishPan(event);
  };

  const imageTransform = `translate(calc(-50% + ${pan.x}px), calc(-50% + ${pan.y}px)) scale(${zoom})`;
  const overlayTransform = `translate(calc(-50% + ${pan.x}px), calc(-50% + ${pan.y}px))`;
  const overlaySize = extent === null ? undefined : { width: extent.width * zoom, height: extent.height * zoom };
  const label = preview !== null ? `${preview.nodeId}.${preview.portId}` : nativePreview !== null ? `${nativePreview.nodeId}.${nativePreview.portId}` : 'NO FRAME';
  const metadataPreview = displayPreview ? preview : null;
  const visibleFrame = metadataPreview?.frameIndex ?? (displayPreview ? nativePreview?.frameIndex ?? null : null);
  const metadataNode = mode === 'selected' ? selectedNode : metadataPreview?.nodeId ?? (displayPreview ? nativePreview?.nodeId : null) ?? '—';

  return (
    <section ref={surfaceRef} className={`panel preview-panel ${focused ? 'is-focused' : ''}`} aria-labelledby="preview-heading" onKeyDown={handlePreviewKey} tabIndex={0}>
      <div className="panel-heading compact preview-heading">
        <div><span className="eyebrow">GPU surface / native-domain output</span><h2 id="preview-heading">Preview</h2></div>
        <span className="preview-label">{displayPreview ? label : mode.toUpperCase()}</span>
      </div>
      <div className="preview-toolbar" aria-label="Preview toolbar">
        <div className="preview-mode" aria-label="Preview mode" role="group">
          {([['final', 'Final'], ['selected', 'Selected'], ['compare', 'Compare']] as const).map(([candidate, text]) => <button key={candidate} className={mode === candidate ? 'is-active' : ''} aria-pressed={mode === candidate} onClick={() => onModeChange(candidate)} type="button">{text}</button>)}
        </div>
        {mode === 'compare' && <div className="preview-compare-selectors"><label>A <select aria-label="Compare node A" value={compareA ?? ''} onChange={(event) => onCompareAChange(event.target.value)}>{nodeOptions.map((node) => <option key={node.id} value={node.id}>{node.label}</option>)}</select></label><label>B <select aria-label="Compare node B" value={compareB ?? ''} onChange={(event) => onCompareBChange(event.target.value)}>{nodeOptions.map((node) => <option key={node.id} value={node.id}>{node.label}</option>)}</select></label></div>}
        <div className="preview-zoom-controls"><button aria-label="Zoom out" onClick={() => setToolbarZoom(zoomOut(zoom))} type="button">−</button><output>{Math.round(zoom * 100)}%</output><button aria-label="Zoom in" onClick={() => setToolbarZoom(zoomIn(zoom))} type="button">+</button><button onClick={fitPreview} type="button">Fit</button><button onClick={() => setToolbarZoom(1)} type="button">1:1</button></div>
        <label className="preview-sample-toggle"><input checked={showSample} onChange={(event) => { setShowSample(event.target.checked); if (!event.target.checked) setSamplePoint(null); }} type="checkbox" /> Pixel sample</label>
        <button className="preview-focus-button" onClick={() => onFocusedChange(!focused)} type="button">{focused ? 'Restore Workspace' : 'Focus Preview'}</button>
      </div>
      {mode === 'compare' && <div className="preview-warning">Native-domain visual comparison; numeric values are not directly equivalent.</div>}
      <div ref={viewportRef} className={`preview-stage ${displayPreview ? 'is-pannable' : ''}`} onPointerDown={handlePointerDown} onPointerMove={movePointer} onPointerUp={finishPointer} onPointerCancel={finishPointer}>
        <div className={`preview-viewport ${displayPreview ? '' : 'is-concealed'}`} style={{ ...((extent === null) ? {} : { width: extent.width, height: extent.height }), transform: imageTransform }} data-preview-visible={displayPreview ? 'true' : 'false'}>
          <div className="preview-media">
            <canvas ref={canvasRef} aria-label="Normal Graph GPU preview" />
            {nativePreview !== null && <img className="preview-native-image" src={nativePreview.dataUrl} alt="Native Normal Graph preview" />}
          </div>
        </div>
        {mode === 'compare' && <div ref={imageViewportRef} className="preview-curtain-overlay" style={{ ...overlaySize, transform: overlayTransform }}><button className="preview-curtain" style={{ left: `${curtain * 100}%` }} onDoubleClick={() => setCurtain(0.5)} onKeyDown={moveCurtainByKey} onPointerDown={handleCurtainPointerDown} type="button" role="slider" aria-label="Compare curtain" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(curtain * 100)} aria-valuetext={`${Math.round(curtain * 100)}% A / ${Math.round((1 - curtain) * 100)}% B`}>A / B</button></div>}
        {!displayPreview && !(isFinal && !hasFrame) && <div className="preview-unavailable"><strong>Preview unavailable</strong><span>{unavailableReason}</span></div>}
        {showSample && samplePoint !== null && <output className="preview-sample-tooltip">x {samplePoint.x} · y {samplePoint.y} · {sample !== null && sample.nodeId === samplePoint.nodeId && sample.x === samplePoint.x && sample.y === samplePoint.y ? sample.values.map((value) => Number.isInteger(value) ? String(value) : value.toFixed(6)).join(' / ') : 'sampling…'}</output>}
      </div>
      <div className="preview-meta"><span>frame <b>{visibleFrame === null ? '—' : `${visibleFrame + 1}${frameCount > 1 ? ` / ${frameCount}` : ''}`}</b></span><span>file <b>{displayPreview ? fileName ?? '—' : '—'}</b></span><span>node <b>{metadataNode ?? '—'}</b></span><span>port <b>{metadataPreview?.portId ?? (displayPreview ? nativePreview?.portId : null) ?? '—'}</b></span><span>domain <b>{metadataPreview?.domain ?? (displayPreview && nativePreview !== null ? 'yuv' : '—')}</b></span><span>format <b>{metadataPreview?.format ?? (displayPreview && nativePreview !== null ? 'rgba8 PNG' : '—')}</b></span><span>range <b>{metadataPreview?.range ?? (displayPreview && nativePreview !== null ? 'normalized' : '—')}</b></span><span>channels <b>{metadataPreview?.channelLayout ?? (displayPreview && nativePreview !== null ? 'rgba' : '—')}</b></span><span>presentation <b>{metadataPreview?.presentation ?? (displayPreview && nativePreview !== null ? 'yuv' : '—')}</b></span><span>extent <b>{metadataPreview ? `${metadataPreview.width} × ${metadataPreview.height}` : displayPreview && nativePreview !== null ? `${nativePreview.outputWidth} × ${nativePreview.outputHeight}` : '—'}</b></span><span>revision <b>{metadataPreview ? `${metadataPreview.runRevision}/${metadataPreview.methodRevision}` : '—'}</b></span><span>generation <b>{metadataPreview?.gpuGeneration ?? '—'}</b></span></div>
    </section>
  );
}
