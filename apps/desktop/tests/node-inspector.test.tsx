import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { NodeInspector } from '../src/components/NodeInspector.js';
import { applyHsLut, hsLutMaxDeltaCab, HsLutPanel, hsLutGrid, hsvToRgb8, sampleHsLut, srgb8ToCab, wheelPixelColor } from '../src/components/iq/HsLutPanel.js';
import { TuningProfilePanel, resolveTuningParameterValue } from '../src/components/iq/TuningProfilePanel.js';
import type { CurvePoint } from '../src/components/iq/curve-model.js';
import type { ColorReproduceAssets, RuntimeEnvelope } from '../../web/src/contracts.js';
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

  it('shows bypass controls only for non-excluded operator modules', () => {
    const bypassConfig = {
      graph_id: 'normal',
      modules: [
        { module_id: 'drc', bypass: false },
        { module_id: 'blc', bypass: false },
      ],
    };
    const graphHtml = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} bypassConfig={bypassConfig} />);
    expect(graphHtml).toContain('DRC bypass');

    const blcHtml = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId={null} bypassConfig={bypassConfig} />);
    expect(blcHtml).not.toContain('BLC bypass');

    const drcHtml = renderToStaticMarkup(<NodeInspector {...inspectorProps} nodeId="drc" bypassConfig={bypassConfig} />);
    expect(drcHtml).toContain('DRC bypass');
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
    expect(html).toContain('>✎</button>');
    expect(html).toContain('>↺</button>');
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

describe('NodeInspector DRC IQ controls', () => {
  it('shows tuning entry points only for DRC scalar inputs', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        {...inspectorProps}
        nodeId="drc"
        activeMethod="00"
        parameterValues={{ drc_gain_offset_ev: 0, knee: 1, amplifier: 1 }}
      />,
    );

    expect(html).toContain('Tune drc_gain_offset_ev');
    expect(html).toContain('Tune knee');
    expect(html).toContain('Tune amplifier');
    expect(html).not.toContain('aria-label="Tune drc_gain"');
    expect(html).not.toContain('aria-label="Tune global_tone_lut"');
  });

  it('renders DRC scalar units, legal ranges, values, and controls', () => {
    const offset = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="drc_gain_offset_ev" controlKind="scalar" baseValues={{ drc_gain_offset_ev: 0 }} onApply={() => undefined} />);
    const knee = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="knee" controlKind="scalar" baseValues={{ knee: 1 }} onApply={() => undefined} />);
    const amplifier = renderToStaticMarkup(<TuningProfilePanel canConfigure parameter="amplifier" controlKind="scalar" baseValues={{ amplifier: 1 }} onApply={() => undefined} />);

    expect(offset).toContain('aria-label="drc_gain_offset_ev current value"');
    expect(offset).toContain('<span>Unit</span><strong>EV</strong>');
    expect(offset).toContain('<span>Range</span><strong>-4 to 4 EV</strong>');
    expect(offset).toContain('Base value');
    expect(offset).toContain('Current value');
    expect(offset).toContain('aria-label="Reset tuning to factory"');
    expect(offset).toContain('aria-label="Apply tuning"');
    expect(knee).toContain('<span>Range</span><strong>&gt; 0 normalized</strong>');
    expect(amplifier).toContain('<span>Range</span><strong>≥ 0</strong>');
  });
});

describe('NodeInspector CR HS LUT IQ page', () => {
  const hsFrame = {
    ...inspectorProps.dngFrame,
    colorReproduce: {
      sensorToProphoto: new Array<number>(9).fill(0),
      prophotoToSrgb: new Array<number>(9).fill(0),
      hsDims: [6, 4] as const,
      hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, index) => (index % 7) - 3 + (index % 3 === 0 ? 0.5 : 0)),
    },
  } as unknown as DngFrameDescriptor;

  it('shows a visualize entry only on the hs_lut parameter row', () => {
    const html = renderToStaticMarkup(
      <NodeInspector
        {...inspectorProps}
        nodeId="color_reproduce"
        activeMethod="00"
        dngFrame={hsFrame}
      />,
    );
    expect(html).toContain('aria-label="Visualize hs_lut"');
    expect(html).not.toContain('aria-label="Tune hs_lut"');
    expect(html).not.toContain('aria-label="Visualize sensor_to_prophoto"');
  });

  it('disables the entry before a DNG frame is loaded', () => {
    const html = renderToStaticMarkup(
      <NodeInspector {...inspectorProps} nodeId="color_reproduce" activeMethod="00" />,
    );
    expect(html).toContain('aria-label="Visualize hs_lut"');
    expect(html).toMatch(/aria-label="Visualize hs_lut"[^>]*disabled/);
    expect(html).toContain('load a DNG frame first');
  });

  it('renders a single LUT-evaluated wheel canvas without component toggles', () => {
    const html = renderToStaticMarkup(<HsLutPanel assets={hsFrame.colorReproduce} />);
    expect(html).toContain('IQ LUT · read-only');
    expect(html).toContain('data-iq-parameter="hs_lut"');
    // Exactly one wheel canvas carrying the grid dims.
    expect(html.match(/<canvas/g)).toHaveLength(1);
    expect(html).toContain('data-hue-divs="6"');
    expect(html).toContain('data-sat-divs="4"');
    expect(html).toContain('<span>grid</span><strong>H 6 × S 4 · ValueDivs 1</strong>');
    // No per-component views: the page shows the LUT's colour effect only.
    expect(html).not.toContain('iq-hslut-component');
    expect(html).not.toContain('aria-label="hs_lut component"');
    // Source toggle: sampled (default) / corrected / delta, one pressed.
    expect(html).toContain('aria-label="hs_lut wheel source"');
    const pressed = html.match(/aria-pressed="true"[^>]*class="iq-hslut-source is-active"/g) ?? [];
    expect(html.match(/class="iq-hslut-source( is-active)?"/g)).toHaveLength(3);
    expect(pressed.length === 1 && pressed[0].length > 0).toBe(true);
    expect(html).toContain('>original</button>');
    expect(html).toContain('>corrected</button>');
    expect(html).toContain('>delta</button>');
    expect(html).toContain('data-testid="hs-lut-max-delta"');
  });

  it('toggles the wheel pixel source between original and corrected', () => {
    const rotated: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 120 : 1)),
    };
    const grid = hsLutGrid(rotated)!;
    const hs = { hue: 0, sat: 1 };
    // original: raw input colour, no lookup -> red.
    expect(wheelPixelColor(grid, hs, 'original')).toEqual({ r: 255, g: 0, b: 0 });
    // corrected: LUT pushes it 120deg -> green.
    expect(wheelPixelColor(grid, hs, 'corrected')).toEqual({ r: 0, g: 255, b: 0 });
    // Identity LUT: the two sources agree pixel-for-pixel.
    const identity: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 0 : 1)),
    };
    const identityGrid = hsLutGrid(identity)!;
    for (const hue of [0, 43, 137, 269, 355]) {
      for (const sat of [0.04, 0.5, 1]) {
        expect(wheelPixelColor(identityGrid, { hue, sat }, 'original')).toEqual(wheelPixelColor(identityGrid, { hue, sat }, 'corrected'));
      }
    }
  });

  it('delta view maps dCab onto a grayscale wheel: darker = more modified', () => {
    // Only hue bin 2 carries a modification: hueShift 90deg moves red
    // toward green; its chroma drop is the wheel's peak dCab.
    const lut: number[] = [];
    for (let h = 0; h < 6; h++) {
      for (let s = 0; s < 4; s++) {
        lut.push(h === 2 ? 90 : 0, 1, 1);
      }
    }
    const grid = hsLutGrid({ sensorToProphoto: [], prophotoToSrgb: [], hsDims: [6, 4], hsEnable: true, hsLut: lut })!;
    // sRGB anchors for the chroma pipeline: white carries no chroma
    // (within 8-bit quantization roundoff), pure red's C_ab is ~104.5
    // (classic CIELAB value).
    expect(srgb8ToCab(255, 255, 255)).toBeCloseTo(0, 1);
    expect(srgb8ToCab(255, 0, 0)).toBeCloseTo(104.55, 1);
    // Peak bin renders darkest (pure black), untouched bins pure white —
    // monochrome grayscale, r == g == b.
    expect(hsLutMaxDeltaCab(grid)).toBeGreaterThan(0);
    const peakHue = ((2 + 0.5) / 6) * 360;
    const peak = wheelPixelColor(grid, { hue: peakHue, sat: 0.875 }, 'delta');
    expect(peak.r).toBe(peak.g);
    expect(peak.g).toBe(peak.b);
    expect(peak.r).toBe(0);
    const untouched = wheelPixelColor(grid, { hue: 0, sat: 1 }, 'delta');
    expect(untouched).toEqual({ r: 255, g: 255, b: 255 });
    // A mid-strength bin lands strictly between white and black.
    const mid = wheelPixelColor(grid, { hue: peakHue, sat: 0.375 }, 'delta');
    expect(mid.r).toBeGreaterThan(0);
    expect(mid.r).toBeLessThan(255);
    // Identity LUT: dCab is identically 0 (incl. the divide-by-zero guard),
    // the whole wheel renders pure white.
    const identity: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 0 : 1)),
    };
    const identityGrid = hsLutGrid(identity)!;
    expect(hsLutMaxDeltaCab(identityGrid)).toBe(0);
    for (const hue of [0, 87, 233, 355]) {
      for (const sat of [0.2, 0.75, 1]) {
        expect(wheelPixelColor(identityGrid, { hue, sat }, 'delta')).toEqual({ r: 255, g: 255, b: 255 });
      }
    }
  });

  it('evaluates the LUT per pixel: identity table maps (h=0, s=1) to pure red', () => {
    const identity: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 0 : 1)),
    };
    const grid = hsLutGrid(identity)!;
    const sample = sampleHsLut(grid, 0, 1);
    expect(sample).toMatchObject({ hueBin: 0, satBin: 3, hueShift: 0, satScale: 1, valScale: 1 });
    const corrected = applyHsLut(0, 1, 1, sample);
    expect(corrected).toEqual({ hue: 0, sat: 1, val: 1 });
    expect(hsvToRgb8(corrected.hue, corrected.sat, corrected.val)).toEqual({ r: 255, g: 0, b: 0 });
    expect(wheelPixelColor(grid, { hue: 0, sat: 1 })).toEqual({ r: 255, g: 0, b: 0 });
  });

  it('evaluates the LUT per pixel: a 120deg hue rotation turns red into green', () => {
    const rotated: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 120 : 1)),
    };
    const grid = hsLutGrid(rotated)!;
    // h=0 (red) shifts by 120deg -> h=120 (green), full saturation.
    expect(wheelPixelColor(grid, { hue: 0, sat: 1 })).toEqual({ r: 0, g: 255, b: 0 });
  });

  it('evaluates the LUT per pixel: satScale 0 collapses every hue to white', () => {
    const desat: ColorReproduceAssets = {
      sensorToProphoto: [], prophotoToSrgb: [],
      hsDims: [6, 4], hsEnable: true,
      hsLut: Array.from({ length: 3 * 6 * 4 }, (_, i) => (i % 3 === 0 ? 0 : i % 3 === 1 ? 0 : 1)),
    };
    const grid = hsLutGrid(desat)!;
    for (const hue of [0, 60, 210, 359.9]) {
      expect(wheelPixelColor(grid, { hue, sat: 1 })).toEqual({ r: 255, g: 255, b: 255 });
    }
  });

  it('clamps the nearest-neighbour lookup to the (D-1) bin at the wheel rim', () => {
    const grid = hsLutGrid(hsFrame.colorReproduce as ColorReproduceAssets)!;
    expect(sampleHsLut(grid, 359.999, 1)).toMatchObject({ hueBin: 5, satBin: 3 });
    expect(sampleHsLut(grid, 360, 1)).toMatchObject({ hueBin: 0, satBin: 3 });
  });

  it('shows the bypass empty state when the frame has no usable HS map', () => {
    const html = renderToStaticMarkup(
      <HsLutPanel assets={{ hsDims: [1, 1], hsEnable: false } as ColorReproduceAssets} />,
    );
    expect(html).toContain('HS calibration unavailable');
    expect(html).toContain('disabled (no profile map)');
    expect(html).not.toContain('<canvas');
  });
});
