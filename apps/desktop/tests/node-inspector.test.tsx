import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { NodeInspector } from '../src/components/NodeInspector.js';
import { TuningProfilePanel, resolveTuningParameterValue } from '../src/components/iq/TuningProfilePanel.js';
import type { CurvePoint } from '../src/components/iq/curve-model.js';
import type { RuntimeEnvelope } from '../../web/src/contracts.js';
import { normalGraphQuantization } from '../../../web/src/generated/normal_quantization.generated.js';
import type { DngFrameDescriptor } from '../src/runtime/worker-bridge.js';

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
  it('renders Overall Rime.Q as a visual switch control', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} />);
    expect(html).toContain('role="switch"');
    expect(html).toContain('class="inspector-switch"');
    expect(html).not.toContain('aria-label="Overall Rime.Q" type="checkbox"');
    expect(html).not.toContain('type="range"');
  });

  it('hides output parameters when module output Rime.Q is disabled', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="blc" quantization={{ ...normalGraphQuantization, modules: normalGraphQuantization.modules.map((module) => module.module_id === 'blc' ? { ...module, output_enabled: false } : module) }} />);
    expect(html).toContain('BLC output Rime.Q');
    expect(html).not.toContain('BLC output profile');
    expect(html).not.toContain('BLC dither');
    expect(html).not.toContain('Dither');
    expect(html).not.toContain('BLC ClipType');
  });

  it('shows signed profiles and the dither-gpu ClipType option', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="blc" />);
    expect(html).toContain('s0.14');
    expect(html).toContain('<option value="dither_gpu">dither-gpu</option>');
  });

  it('uses generated signed quantization defaults without exposing input profile', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} />);
    expect(html).toContain('s0.14');
    expect(html).toContain('s0.12');
    expect(html).toContain('s0.10');
    expect(html).not.toContain('Input profile');
  });

  it('disables Rime.Q and hides quantization parameters for bypass nodes', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="sbpc_horizontal" />);
    expect(html).toContain('disabled=""');
    expect(html).toContain('bypass');
    expect(html).toContain('SBPC-H output Rime.Q');
    expect(html).not.toContain('SBPC-H output profile');
    expect(html).not.toContain('SBPC-H ClipType');
  });
  it('uses the current DNG frame gains over stale default values', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        {...inspectorProps}
        nodeId="wbc"
        parameterValues={{ red_gain: 2, green_gain: 1, blue_gain: 1.5 }}
        dngFrame={{ cfa: 'rggb', whiteBalanceGains: [2.125, 1, 1.625] } as DngFrameDescriptor}
      />,
    );

    expect(html).toContain('red_gain');
    expect(html).toContain('2.125');
    expect(html).toContain('green_gain');
    expect(html).toContain('1');
    expect(html).toContain('blue_gain');
    expect(html).toContain('1.625');
    expect(html).not.toContain('>2<');
    expect(html).not.toContain('>1.5<');
  });

  it('keeps parameter values as the WBC fallback before a DNG is loaded', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        {...inspectorProps}
        nodeId="wbc"
        parameterValues={{ red_gain: 2, green_gain: 1, blue_gain: 1.5 }}
      />,
    );

    expect(html).toContain('red_gain');
    expect(html).toContain('>2<');
    expect(html).toContain('green_gain');
    expect(html).toContain('>1<');
    expect(html).toContain('blue_gain');
    expect(html).toContain('>1.5<');
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

  it('shows per-parameter IQ tuning actions without mounting a curve page', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        nodeId="dem"
        envelope={envelope}
        dngFrame={null}
        frameCount={0}
        activeMethod="04"
        parameterValues={{ cfa_pattern: 'rggb', ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }}
        onMethodChange={() => undefined}
        onParameterChange={() => undefined}
      />,
    );

    expect(html).toContain('Tune ahd_l_threshold');
    expect(html).toContain('Tune ahd_c_threshold_sq');
    expect(html).not.toContain('Tune cfa_pattern');
    expect(html).not.toContain('ahd_l_threshold curve');
    expect(html).not.toContain('<svg');
  });
  it('renders only the selected parameter control on the tuning page', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }} onApply={() => undefined} />);
    expect(html).toContain('ahd_l_threshold');
    expect(html).not.toContain('ahd_c_threshold_sq curve');
    expect(html.match(/<svg/g)?.length).toBe(1);
  });
  it('separates IQ title, parameter, curve kind, and profile metadata', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2 }} onApply={() => undefined} />);
    expect(html).toContain('class="iq-tuning-heading"');
    expect(html).toContain('<span class="section-label">IQ Tuning</span>');
    expect(html).toContain('<strong>ahd_l_threshold</strong>');
    expect(html).toContain('<small>curve · factory-default</small>');
  });
  it('exposes Apply as a real parameter submission control', () => {
    const applied: Array<[string, number]> = [];
    const html = renderToStaticMarkup(
      <TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2 }} onApply={(parameter, value) => applied.push([parameter, value])} />,
    );
    expect(html).toContain('Apply');
    expect(applied).toHaveLength(0);
  });

  it('renders a per-parameter factory reset action beside tuning controls', () => {
    const html = renderToStaticMarkup(
      <NodeInspector {...inspectorProps} nodeId="dem" activeMethod="04" parameterValues={{ cfa_pattern: 'rggb', ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }} />,
    );
    expect(html).toContain('Reset ahd_l_threshold to factory');
    expect(html).toContain('Reset ahd_c_threshold_sq to factory');
  });
  it('resolves the displayed direct LUT value from the current axis coordinate', () => {
    const curve: readonly CurvePoint[] = [{ x: -4, y: 1 }, { x: 0, y: 1.05 }, { x: 4, y: 1.1 }];
    expect(resolveTuningParameterValue(2, curve, 4)).toBeCloseTo(1.1);
  });
  it('renders base, current LUT, and effect values', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2 }} onApply={() => undefined} />);
    expect(html).toContain('Base LUT value');
    expect(html).toContain('Current LUT value');
    expect(html).toContain('Effect value');
    expect(html).toContain('2.0000');
  });
  it('shows the applied AHD value when returning to the parameter page', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="dem" activeMethod="04" parameterValues={{ cfa_pattern: 'rggb', ahd_l_threshold: 3.25, ahd_c_threshold_sq: 4 }} />);
    expect(html).toContain('value="3.25"');
    expect(html).toContain('data-current-parameter-value="ahd_l_threshold:3.25"');
  });
  it('marks parameters as unapplied when draft differs from applied value', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="dem" activeMethod="04" parameterValues={{ cfa_pattern: 'rggb', ahd_l_threshold: 3.25, ahd_c_threshold_sq: 4 }} appliedParameterValues={{ ahd_l_threshold: 2, ahd_c_threshold_sq: 4 }} />);
    expect(html).toContain('data-parameter-dirty="ahd_l_threshold"');
    expect(html).toContain('class="inspector-parameter-editor is-dirty"');
  });
  it('renders indexed grid labels and current LUT coordinate', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2 }} onApply={() => undefined} />);
    expect(html).toContain('index / EV knot');
    expect(html).toContain('parameter value');
    expect(html).toContain('aria-label="ahd_l_threshold curve point 1"');
    expect(html).toContain('class="iq-current-marker"');
  });
  it('renders persisted knot values supplied by the parent draft', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="ahd_l_threshold" controlKind="curve" baseValues={{ ahd_l_threshold: 2 }} curves={{ lCurve: [{ x: -4, y: 9.25 }, { x: 4, y: 9.5 }], cCurve: [{ x: -4, y: 3 }, { x: 4, y: 3.3 }] }} onApply={() => undefined} />);
    expect(html).toContain('[0] -4: 9.2500');
  });
  it('renders the adjustable Gamma exponent and luminance-only LUT action', () => {
    const html = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="gamma" parameterValues={{ gamma: 2.2, gamma_lut: '9-point Y LUT' }} appliedParameterValues={{ gamma: 2.2 }} />);
    expect(html).toContain('aria-label="gamma"');
    expect(html).toContain('min="1.8"');
    expect(html).toContain('max="2.4"');
    expect(html).toContain('step="0.1"');
    expect(html).toContain('Tune gamma_lut');
    expect(html).toContain('9-point Y LUT');
    expect(html).not.toContain('red_lut');
    expect(html).not.toContain('green_lut');
    expect(html).not.toContain('blue_lut');
  });
  it('renders Gamma as a nine-knot monotone Bézier luminance curve', () => {
    const html = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="gamma_lut" controlKind="lut_1d" baseValues={{ gamma: 2.2, gamma_lut: '9-point Y LUT' }} onApply={() => undefined} />);
    expect(html).toContain('linear_luminance_y · normalized');
    expect(html).toContain('<strong>monotone Bézier</strong>');
    expect(html.match(/aria-label="gamma_lut curve point/g)?.length).toBe(9);
    expect(html).toContain('[0] 0: 0.0000');
    expect(html).toContain('[8] 1: 1.0000');
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
