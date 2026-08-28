import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { NodeInspector } from '../src/components/NodeInspector.js';
import type { RuntimeEnvelope } from '../../web/src/contracts.js';

const envelope: RuntimeEnvelope = {
  graphInstanceId: 1,
  runRevision: 0,
  methodRevision: 1,
  gpuGeneration: 0,
  frameIndex: null,
  framePhase: null,
  visibleFrameCommitted: false,
  lifecycleState: 'stop',
};

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
