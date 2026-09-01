import { useEffect, useRef, useState } from 'react';
import { Group, Panel, Separator, usePanelRef, type Layout } from 'react-resizable-panels';

import type { PreviewDescriptor, RuntimeEnvelope, RuntimeEvent, RuntimeLogEntry } from '../../../web/src/contracts.js';
import { readPaneLayout, writePaneLayout } from '../../../web/src/pane-layout.js';
import { acceptsEnvelope } from '../../../web/src/revision-guard.js';
import { normalGraphQuantization } from '../../../web/src/generated/normal_quantization.generated.js';
import { LogConsole } from './components/LogConsole.js';
import { NormalGraphCanvas } from './components/NormalGraphCanvas.js';
import { NodeInspector, type GraphQuantizationConfig, type ModuleQuantizationPreference } from './components/NodeInspector.js';
import { PreviewSurface } from './components/PreviewSurface.js';
import { loadDngIntoWorker, loadDngPathIntoWorker, loadDngSequenceIntoWorker, loadDngSequencePathIntoWorker, loadDecodedDngIntoWorker, decodeDngPath } from './runtime/dng-loader.js';
import { isCurrentDngPrefetch, nextDngRunFrame } from './runtime/dng-sequence.js';
import { listenNativePipeline, renderDngNative, type NativeRenderDescriptor } from './runtime/native-pipeline.js';
import type { DecodedDngFramePayload } from './runtime/dng-frame-payload.js';
import type { DngFrameDescriptor, DngSequenceDescriptor, WorkerBridge } from './runtime/worker-bridge.js';
import { createWorkerBridge } from './runtime/worker-bridge.js';

const INITIAL_ENVELOPE: RuntimeEnvelope = {
  graphInstanceId: 1,
  runRevision: 0,
  methodRevision: 0,
  configRevision: 0,
  frameIndex: null,
  framePhase: null,
  visibleFrameCommitted: false,
  lifecycleState: 'loading',
  gpuGeneration: 0,
};

const WORKSPACE_LAYOUT_KEY = 'rime:pane-layout:workspace:v2';
const LEFT_LAYOUT_KEY = 'rime:pane-layout:left:v2';
const RIGHT_LAYOUT_KEY = 'rime:pane-layout:right:v2';

const WORKSPACE_LAYOUT = { left: 62, right: 38 };
const LEFT_LAYOUT = { graph: 78, logs: 22 };
const RIGHT_LAYOUT = { inspector: 34, preview: 66 };

function persistedLayout(key: string, fallback: Layout): Layout {
  return readPaneLayout(window.localStorage, key, fallback);
}
export function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const bridgeRef = useRef<WorkerBridge | null>(null);
  const envelopeRef = useRef<RuntimeEnvelope>(INITIAL_ENVELOPE);
  const logsPanelRef = usePanelRef();
  const [envelope, setEnvelope] = useState(INITIAL_ENVELOPE);
  const [preview, setPreview] = useState<PreviewDescriptor | null>(null);
  const [nativePreview, setNativePreview] = useState<NativeRenderDescriptor | null>(null);
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [quantization, setQuantization] = useState<GraphQuantizationConfig>(normalGraphQuantization);
  const [activeMethods, setActiveMethods] = useState<Record<string, string>>({ dem: '00' });
  const [parameterValues, setParameterValues] = useState<Record<string, string | number>>({
    cfa_pattern: 'rggb',
    vng_threshold: 1.5,
    ahd_l_threshold: 2.0,
    ahd_c_threshold_sq: 4.0,
  });
  const [logs, setLogs] = useState<RuntimeLogEntry[]>([]);
  const [loadedDng, setLoadedDng] = useState<DngFrameDescriptor | null>(null);
  const [dngSequence, setDngSequence] = useState<DngSequenceDescriptor | null>(null);
  const [dngPaths, setDngPaths] = useState<readonly string[]>([]);
  const [dngFrameIndex, setDngFrameIndex] = useState(0);
  const [sequencePlaying, setSequencePlaying] = useState(false);
  const sequencePlayingRef = useRef(false);
  const dngFrameIndexRef = useRef(0);
  const pendingDngRef = useRef<{ readonly index: number; readonly generation: number; readonly decoded: DecodedDngFramePayload } | null>(null);
  const prefetchSlotRef = useRef<{ readonly index: number; readonly generation: number; readonly promise: Promise<DecodedDngFramePayload> } | null>(null);
  const sequenceGenerationRef = useRef(0);
  const nativeGenerationRef = useRef(0);
  const dngPathsRef = useRef<readonly string[]>([]);
  const [commandPending, setCommandPending] = useState(false);
  const [logsCollapsed, setLogsCollapsed] = useState(false);
  const [fitGraphRequest, setFitGraphRequest] = useState(0);
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const [workspaceLayout] = useState(() => persistedLayout(WORKSPACE_LAYOUT_KEY, WORKSPACE_LAYOUT));
  const [leftLayout] = useState(() => persistedLayout(LEFT_LAYOUT_KEY, LEFT_LAYOUT));
  const [rightLayout] = useState(() => persistedLayout(RIGHT_LAYOUT_KEY, RIGHT_LAYOUT));
  const beginDngPrefetch = (index: number): void => {
    const path = dngPathsRef.current[index];
    const generation = sequenceGenerationRef.current;
    if (path === undefined || prefetchSlotRef.current !== null) return;
    const promise = decodeDngPath(path, index);
    prefetchSlotRef.current = { index, generation, promise };
    void promise.then((decoded) => {
      if (generation === sequenceGenerationRef.current) pendingDngRef.current = { index, generation, decoded };
    }).finally(() => { if (prefetchSlotRef.current?.promise === promise) prefetchSlotRef.current = null; });
  };

  const consumeDngFrame = async (index: number, mode: 'run' | 'step'): Promise<void> => {
    const path = dngPathsRef.current[index];
    if (path === undefined || bridgeRef.current === null) return;
    const generation = sequenceGenerationRef.current;
    const pending = pendingDngRef.current?.index === index && pendingDngRef.current.generation === generation ? pendingDngRef.current : null;
    const decoded = pending?.decoded ?? await decodeDngPath(path, index);
    if (generation !== sequenceGenerationRef.current) return;
    pendingDngRef.current = null;
    dngFrameIndexRef.current = index;
    setDngFrameIndex(index);
    setLoadedDng(decoded.descriptor);
    loadDecodedDngIntoWorker(bridgeRef.current, decoded);
    if (mode === 'run') beginDngPrefetch(index + 1);
    bridgeRef.current[mode](index);
  };


  const renderNativeFrame = async (index: number): Promise<void> => {
    const generation = nativeGenerationRef.current;
    const path = dngPathsRef.current[index];
    if (path === undefined) throw new Error('NATIVE_PIPELINE_INPUT_MISSING: DNG path is unavailable');
    const descriptor = await renderDngNative(path, index);
    if (generation !== nativeGenerationRef.current) return;
    dngFrameIndexRef.current = descriptor.frameIndex;
    setDngFrameIndex(descriptor.frameIndex);
    setNativePreview(descriptor);
    setCommandPending(false);
    setLogs((current) => [...current.slice(-39), { level: 'info', message: `native ${descriptor.nodeId}.${descriptor.portId} completed for frame ${descriptor.frameIndex} (${descriptor.encoderBackend})`, framePhase: 'output' }]);
  };

  const renderNativeSequence = async (startIndex: number, play: boolean): Promise<void> => {
    let index = startIndex;
    while (true) {
      await renderNativeFrame(index);
      if (!play || !sequencePlayingRef.current || index + 1 >= dngPathsRef.current.length) break;
      index += 1;
    }
    sequencePlayingRef.current = false;
    setSequencePlaying(false);
  };

  const invalidateDngPrefetch = (): void => {
    nativeGenerationRef.current += 1;
    sequenceGenerationRef.current += 1;
    pendingDngRef.current = null;
    prefetchSlotRef.current = null;
    setNativePreview(null);
  };

  useEffect(() => {
    let unlistenNative: (() => void) | undefined;
    void listenNativePipeline((event) => {
      if (event.event === 'frame_started') {
        appendLog({ level: 'info', message: `native frame ${event.frame_index} started`, framePhase: 'output' });
      }
    }).then((unlisten) => { unlistenNative = unlisten; });
    const canvas = canvasRef.current;
    if (canvas === null) return undefined;
    const offscreen = canvas.transferControlToOffscreen();
    const bridge = createWorkerBridge(handleEvent);
    bridgeRef.current = bridge;
    void bridge.initialize(offscreen).then(async () => {
      const smokeSequencePath = import.meta.env.VITE_RIME_DNG_SEQUENCE_SMOKE_PATH;
      if (smokeSequencePath !== undefined && smokeSequencePath.length > 0) {
        try {
          const { descriptor, sequence } = await loadDngSequencePathIntoWorker(bridge, smokeSequencePath);
          setLoadedDng(descriptor);
          setDngSequence(sequence);
          setDngPaths(sequence.paths);
          dngPathsRef.current = sequence.paths;
          appendLog({ level: 'info', message: `DNG sequence smoke loaded: ${sequence.frameCount} frames` });
        } catch (error) {
          appendLog({ level: 'error', message: String(error), diagnosticCode: 'DNG_SEQUENCE_SMOKE_FAILED' });
        }
        return;
      }
      const smokePath = import.meta.env.VITE_RIME_DNG_SMOKE_PATH;
      if (smokePath === undefined || smokePath.length === 0) return;
      try {
        const descriptor = await loadDngPathIntoWorker(bridge, smokePath, 0);
        setLoadedDng(descriptor);
        setDngPaths([smokePath]);
        dngPathsRef.current = [smokePath];
        appendLog({ level: 'info', message: `DNG smoke loaded: ${descriptor.cameraModel} ${descriptor.width}x${descriptor.height}` });
      } catch (error) {
        appendLog({ level: 'error', message: String(error), diagnosticCode: 'DNG_SMOKE_FAILED' });
      }
    });
    return () => { unlistenNative?.(); bridge.dispose(); };

    function handleEvent(event: RuntimeEvent): void {
      if (!acceptsEnvelope(envelopeRef.current, event.envelope)) return;
      envelopeRef.current = event.envelope;
      if (event.type === 'ready' || event.type === 'snapshot') {
        setEnvelope(event.envelope);
        setCommandPending(false);
        if (!event.envelope.visibleFrameCommitted && dngPathsRef.current.length <= 1) setPreview(null);
      }
      if (event.type === 'preview') {
        setEnvelope(event.envelope);
        setPreview(event.preview);
        setCommandPending(false);
        appendLog({ level: 'info', message: `frame ${event.preview.frameIndex} committed to GPU preview`, framePhase: 'output' });
        const nextIndex = event.preview.frameIndex + 1;
        if (sequencePlayingRef.current && nextIndex < dngPathsRef.current.length) {
          setCommandPending(true);
          void consumeDngFrame(nextIndex, 'run').catch((error: unknown) => {
            sequencePlayingRef.current = false;
            setSequencePlaying(false);
            setCommandPending(false);
            appendLog({ level: 'error', message: String(error), diagnosticCode: 'DNG_SEQUENCE_FAILED' });
          });
        } else if (sequencePlayingRef.current) {
          sequencePlayingRef.current = false;
          setSequencePlaying(false);
        }
      }
      if (event.type === 'log') {
        setEnvelope(event.envelope);
        if (event.entry.level === 'error') setCommandPending(false);
        appendLog(event.entry);
      }
    }

    function appendLog(entry: RuntimeLogEntry): void {
      setLogs((current) => [...current.slice(-39), entry]);
    }
  }, []);

  const canStep = !commandPending && envelope.lifecycleState === 'stop';
  const canRun = !commandPending && (envelope.lifecycleState === 'stop' || envelope.lifecycleState === 'completed');
  const canReset = !commandPending && envelope.lifecycleState !== 'loading' && envelope.lifecycleState !== 'unloaded';
  const canLoad = !commandPending && (envelope.lifecycleState === 'stop' || envelope.lifecycleState === 'completed');

  const loadDng = (): void => {
    if (bridgeRef.current === null) return;
    setOpenMenu(null);
    invalidateDngPrefetch();
    setCommandPending(true);
    void loadDngIntoWorker(bridgeRef.current).then(({ descriptor, paths }) => {
      setLoadedDng(descriptor);
      setDngSequence(null);
      setDngPaths(paths);
      dngPathsRef.current = paths;
      setDngFrameIndex(0);
      dngFrameIndexRef.current = 0;
      setNativePreview(null);
      setCommandPending(false);
    }).catch((error: unknown) => {
      setCommandPending(false);
      setLogs((current) => [...current.slice(-39), { level: 'error', message: String(error), diagnosticCode: 'DNG_LOAD_FAILED' }]);
    });
  };

  const loadDngSequence = (): void => {
    if (bridgeRef.current === null) return;
    invalidateDngPrefetch();
    setOpenMenu(null);
    setCommandPending(true);
    sequencePlayingRef.current = false;
    setSequencePlaying(false);
    void loadDngSequenceIntoWorker(bridgeRef.current).then(({ descriptor, sequence }) => {
      setLoadedDng(descriptor);
      setDngSequence(sequence);
      setDngPaths(sequence.paths);
      dngPathsRef.current = sequence.paths;
      setDngFrameIndex(0);
      dngFrameIndexRef.current = 0;
      setNativePreview(null);
      setCommandPending(false);
    }).catch((error: unknown) => {
      setCommandPending(false);
      setLogs((current) => [...current.slice(-39), { level: 'error', message: String(error), diagnosticCode: 'DNG_SEQUENCE_LOAD_FAILED' }]);
    });
  };

  const toggleMenu = (menu: string): void => {
    setOpenMenu((current) => current === menu ? null : menu);
  };

  const changeMethod = (nodeId: string, method: string): void => {
    if (bridgeRef.current === null) return;
    setCommandPending(true);
    setActiveMethods((current) => ({ ...current, [nodeId]: method }));
    bridgeRef.current.setMethod(nodeId, method);
  };

  const changeParameter = (nodeId: string, parameter: string, value: number): void => {
    if (bridgeRef.current === null || !Number.isFinite(value)) return;
    setCommandPending(true);
    setParameterValues((current) => ({ ...current, [parameter]: value }));
    bridgeRef.current.setParameter(nodeId, parameter, value);
  };
  const changeGraphQuantization = (next: GraphQuantizationConfig): void => {
    if (bridgeRef.current === null) return;
    setCommandPending(true);
    setQuantization(next);
    bridgeRef.current.setQuantizationConfig(JSON.stringify(next));
  };

  const changeModuleQuantization = (moduleId: string, preference: ModuleQuantizationPreference): void => {
    const next = { ...quantization, modules: quantization.modules.map((module) => module.module_id === moduleId ? preference : module) };
    changeGraphQuantization(next);
  };
  const runGraph = (): void => {
    if (bridgeRef.current === null || dngPathsRef.current.length === 0) return;
    const pending = pendingDngRef.current;
    const runIndex = nextDngRunFrame(dngFrameIndexRef.current, dngPathsRef.current.length, envelope.visibleFrameCommitted, pending?.index ?? null);
    sequencePlayingRef.current = dngPathsRef.current.length > 1 && runIndex + 1 < dngPathsRef.current.length;
    setSequencePlaying(sequencePlayingRef.current);
    setCommandPending(true);
    if (runIndex !== dngFrameIndexRef.current || pending?.index === runIndex) {
      void consumeDngFrame(runIndex, 'run').catch((error: unknown) => {
        sequencePlayingRef.current = false;
        setSequencePlaying(false);
        setCommandPending(false);
        setLogs((current) => [...current.slice(-39), { level: 'error', message: String(error), diagnosticCode: 'DNG_SEQUENCE_FAILED' }]);
      });
      return;
    }
    bridgeRef.current.run(runIndex);
  };

  const stepGraph = (): void => {
    if (bridgeRef.current === null || dngPathsRef.current.length === 0) return;
    sequencePlayingRef.current = false;
    setSequencePlaying(false);
    setCommandPending(true);
    void consumeDngFrame(dngFrameIndexRef.current, 'step').catch((error: unknown) => {
      setCommandPending(false);
      setLogs((current) => [...current.slice(-39), { level: 'error', message: String(error), diagnosticCode: 'DNG_SEQUENCE_FAILED' }]);
    });
  };

  const resetGraph = (): void => {
    sequencePlayingRef.current = false;
    setSequencePlaying(false);
    invalidateDngPrefetch();
    setCommandPending(true);
    bridgeRef.current?.reset();
  };


  const transportControls = (
    <div className="transport-toolbar" aria-label="Transport controls">
      <button disabled={!canRun} onClick={runGraph} type="button">Run</button>
      <button disabled={!canStep} onClick={stepGraph} type="button">Step <span className="shortcut">▷|</span></button>
      <button disabled={!sequencePlaying} onClick={() => { sequencePlayingRef.current = false; setSequencePlaying(false); }} type="button">Pause</button>
      <button disabled={!canReset} onClick={resetGraph} type="button">Reset</button>
    </div>
  );

  return (
    <main className="app-shell">
      <header className="command-bar">
        <div className="brand-lockup"><span className="brand-mark">R</span><h1>Normal Graph</h1></div>
        <nav className="menu-bar" aria-label="Application menu">
          <div className="menu-item">
            <button className="menu-trigger" onClick={() => toggleMenu('file')} type="button" aria-expanded={openMenu === 'file'}>File</button>
            {openMenu === 'file' && <div className="menu-popover">
              <button disabled={!canLoad} onClick={loadDng} type="button">Load DNG</button>
              <button disabled={!canLoad} onClick={loadDngSequence} type="button">Load DNG sequence</button>
              <button disabled={!canReset} onClick={() => { setOpenMenu(null); resetGraph(); }} type="button">Reset</button>
            </div>}
          </div>
          <div className="menu-item">
            <button className="menu-trigger" onClick={() => toggleMenu('view')} type="button" aria-expanded={openMenu === 'view'}>View</button>
            {openMenu === 'view' && <div className="menu-popover"><button type="button" onClick={() => { setOpenMenu(null); setFitGraphRequest((request) => request + 1); }}>Fit Graph</button></div>}
          </div>
          <div className="menu-item">
            <button className="menu-trigger" onClick={() => toggleMenu('help')} type="button" aria-expanded={openMenu === 'help'}>Help</button>
            {openMenu === 'help' && <div className="menu-popover"><span className="menu-note">Rime FrameForge</span></div>}
          </div>
        </nav>
      </header>
      <Group
        className="shell-split"
        id="workspace-split"
        defaultLayout={workspaceLayout}
        onLayoutChanged={(layout) => writePaneLayout(window.localStorage, WORKSPACE_LAYOUT_KEY, layout)}
      >
        <Panel id="left" minSize="45%">
          <Group
            className="workspace-split"
            id="left-split"
            orientation="vertical"
            defaultLayout={leftLayout}
            onLayoutChanged={(layout) => writePaneLayout(window.localStorage, LEFT_LAYOUT_KEY, layout)}
          >
            <Panel id="graph" minSize="45%">
              <div className="pane-content">
                <NormalGraphCanvas envelope={envelope} onSelect={setSelectedNode} selectedNode={selectedNode} fitRequest={fitGraphRequest} headingActions={transportControls} />
              </div>
            </Panel>
            <Separator className="pane-separator pane-separator-horizontal" id="graph-logs-separator" />
            <Panel id="logs" panelRef={logsPanelRef} collapsible collapsedSize="30px" minSize="80px" maxSize="40%" onResize={(size) => setLogsCollapsed(size.inPixels <= 31)}>
              <div className="pane-content">
                <LogConsole entries={logs} collapsed={logsCollapsed} onToggle={() => { if (logsPanelRef.current?.isCollapsed()) logsPanelRef.current.expand(); else logsPanelRef.current?.collapse(); }} />
              </div>
            </Panel>
          </Group>
        </Panel>
        <Separator className="pane-separator pane-separator-vertical" id="workspace-columns-separator" />
        <Panel id="right" minSize="35%">
          <Group
            className="diagnostic-split"
            id="right-split"
            orientation="vertical"
            defaultLayout={rightLayout}
            onLayoutChanged={(layout) => writePaneLayout(window.localStorage, RIGHT_LAYOUT_KEY, layout)}
          >
            <Panel id="inspector" minSize="28%"><div className="pane-content"><NodeInspector nodeId={selectedNode} envelope={sequencePlaying ? { ...envelope, lifecycleState: 'running', frameIndex: dngFrameIndex } : { ...envelope, lifecycleState: dngPaths.length > 1 && dngFrameIndex + 1 < dngPaths.length && envelope.lifecycleState === 'completed' ? 'paused' : envelope.lifecycleState, frameIndex: loadedDng?.frameIndex ?? envelope.frameIndex }} dngFrame={loadedDng} dngSequence={dngSequence} frameCount={dngPaths.length} activeMethod={activeMethods[selectedNode ?? ''] ?? '00'} parameterValues={{ ...parameterValues, cfa_pattern: loadedDng?.cfa ?? String(parameterValues.cfa_pattern ?? 'rggb') }} quantization={quantization} onGraphQuantizationChange={changeGraphQuantization} onModuleQuantizationChange={changeModuleQuantization} onMethodChange={changeMethod} onParameterChange={changeParameter} /></div></Panel>
            <Separator className="pane-separator pane-separator-horizontal" id="inspector-preview-separator" />
            <Panel id="preview" minSize="28%"><div className="pane-content"><PreviewSurface canvasRef={canvasRef} preview={preview} nativePreviewDataUrl={nativePreview?.previewDataUrl} fileName={nativePreview === null ? null : dngSequence?.fileNames[nativePreview.frameIndex] ?? loadedDng?.fileName ?? null} frameCount={dngPaths.length} /></div></Panel>
          </Group>
        </Panel>
      </Group>
      <footer className="status-bar" aria-label="Runtime status">
        <span><b>graph</b> normal · {envelope.graphInstanceId}</span>
        <span><i className="live-dot" /> GPU / FP32</span>
        <span><b>frame</b> {envelope.visibleFrameCommitted ? envelope.frameIndex ?? '—' : '—'}</span>
        <span className={`status-state state-${envelope.lifecycleState}`}>{envelope.lifecycleState}</span>
        <span className="status-input">{loadedDng ? `${loadedDng.cameraModel} · ${loadedDng.width} × ${loadedDng.height}` : 'fixed RAW asset'}</span>
      </footer>
    </main>
  );
}
