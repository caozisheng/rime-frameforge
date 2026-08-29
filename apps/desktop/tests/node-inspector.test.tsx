import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { NodeInspector } from '../src/components/NodeInspector.js';
import type { RuntimeEnvelope } from '../../web/src/contracts.js';

const envelope: RuntimeEnvelope = {
  graphInstanceId: 1,
  runRevision: 0,
  methodRevision: 1,
  configRevision: 0,
  gpuGeneration: 0,
  frameIndex: null,
  framePhase: null,
  visibleFrameCommitted: false,
  lifecycleState: 'stop',
};

const inspectorProps = {
  envelope,
  dngFrame: null,
  frameCount: 0,
  activeMethod: '00',
  parameterValues: {},
  onMethodChange: () => undefined,
  onParameterChange: () => undefined,
  onGraphQuantizationChange: () => undefined,
  onModuleQuantizationChange: () => undefined,
};

describe('graph-level NodeInspector', () => {
  it('renders the graph tree when selection is null', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} />);
    expect(html).toContain('Normal Graph');
    expect(html).toContain('Overall');
    expect(html).toContain('VFE');
    expect(html).toContain('VBE');
    expect(html).toContain('VPE');
    expect(html).toContain('pass-1');
    expect(html).toContain('pass-2');
    expect(html).toContain('pass-3');
    expect(html).toContain('Rime.Q');
  });

  it('uses generated quantization defaults without exposing input profile', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} />);
    expect(html).toContain('u0.14');
    expect(html).toContain('u0.12');
    expect(html).toContain('u0.10');
    expect(html).not.toContain('Input profile');
  });

  it('disables effective module controls for bypass nodes', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="sbpc_horizontal" />);
    expect(html).toContain('disabled=""');
    expect(html).toContain('bypass');
    expect(html).toContain('ClipType');
  });
});

describe('NodeInspector DEM controls', () => {
  it('renders the method selector and active method parameters', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        nodeId="dem"
        envelope={envelope}
        dngFrame={null}
        frameCount={0}
        activeMethod="03"
        parameterValues={{ cfa_pattern: 'rggb', vng_threshold: 1.5 }}
        onMethodChange={() => undefined}
        onParameterChange={() => undefined}
      />,
    );

    expect(html).toContain('<select');
    expect(html).toContain('value="04"');
    expect(html).toContain('vng_threshold');
    expect(html).toContain('value="1.5"');
  });

  it('renders every operator inspector as a tree with compact groups', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        nodeId="rgb2yuv"
        envelope={envelope}
        dngFrame={null}
        frameCount={0}
        activeMethod="00"
        parameterValues={{}}
        onMethodChange={() => undefined}
        onParameterChange={() => undefined}
      />,
    );

    expect(html).toContain('role="tree"');
    expect(html).toContain('General');
    expect(html).toContain('Parameters');
    expect(html).not.toContain('<dl');
  });

  it('keeps Raw Source on the same tree renderer', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        nodeId="raw_source"
        envelope={envelope}
        dngFrame={null}
        frameCount={0}
        activeMethod="00"
        parameterValues={{}}
        onMethodChange={() => undefined}
        onParameterChange={() => undefined}
      />,
    );

    expect(html).not.toContain('<dl');
    expect(html).toContain('No DNG frame loaded');
  });
});
