import { createRef } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { PreviewDescriptor } from '../../../web/src/contracts.js';
import { PreviewSurface } from '../src/components/PreviewSurface.js';

const preview: PreviewDescriptor = {
  nodeId: 'rgb2yuv', portId: 'out', frameIndex: 2, runRevision: 4, methodRevision: 1,
  gpuGeneration: 3, width: 1920, height: 1080, format: 'rgba32_float', domain: 'yuv',
  range: 'normalized', channelLayout: 'rgba', presentation: 'yuv',
};
const demPreview: PreviewDescriptor = { ...preview, nodeId: 'dem', domain: 'linear_rgb', presentation: 'rgb' };
const nodes = [
  { id: 'blc', label: 'BLC' },
  { id: 'dem', label: 'DEM' },
  { id: 'rgb2yuv', label: 'RGB2YUV' },
];
const handlers = {
  onModeChange: () => undefined,
  onCompareAChange: () => undefined,
  onCompareBChange: () => undefined,
  onFocusedChange: () => undefined,
  onPresentationChange: () => undefined,
  onSampleRequest: () => undefined,
};

function render(mode: 'final' | 'selected' | 'compare', selectedNode: string | null = 'blc'): string {
  return renderToStaticMarkup(
    <PreviewSurface canvasRef={createRef<HTMLCanvasElement>()} previews={[preview]} nativePreview={null} fileName="frame.dng" frameCount={3} sample={null} mode={mode} selectedNode={selectedNode} previewCapabilities={{ rgb2yuv: true }} nodeOptions={nodes} compareA="blc" compareB="dem" focused={false} {...handlers} />,
  );
}

describe('PreviewSurface section 9 contract', () => {
  it('renders mode, discrete zoom, fit, one-to-one, sample, and focus controls', () => {
    const html = render('final');
    expect(html).toContain('aria-label="Preview mode"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain('Final');
    expect(html).toContain('Selected');
    expect(html).toContain('Compare');
    expect(html).toContain('100%');
    expect(html).toContain('Fit');
    expect(html).toContain('1:1');
    expect(html).toContain('Pixel sample');
    expect(html).toContain('Focus Preview');
  });

  it('does not substitute final output when the selected node has no published surface', () => {
    const html = render('selected', 'blc');
    expect(html).toContain('Preview unavailable');
    expect(html).toContain('blc');
    expect(html).not.toContain('data-preview-visible="true"');
  });

  it('shows explicit A/B selectors, warning, and an accessible curtain', () => {
    const html = render('compare');
    expect(html).toContain('aria-label="Compare node A"');
    expect(html).toContain('aria-label="Compare node B"');
    expect(html).toContain('Native-domain visual comparison; numeric values are not directly equivalent.');
    expect(html).toContain('A and B must reference two independently selectable committed outputs.');
    expect(html).toContain('role="slider"');
    expect(html).toContain('aria-valuetext="50% A / 50% B"');
  });

  it('rejects Compare when both selectors point at one committed output', () => {
    const html = renderToStaticMarkup(
      <PreviewSurface canvasRef={createRef<HTMLCanvasElement>()} previews={[preview]} nativePreview={null} fileName="frame.dng" frameCount={3} sample={null} mode="compare" selectedNode={null} previewCapabilities={{ rgb2yuv: true }} nodeOptions={nodes} compareA="rgb2yuv" compareB="rgb2yuv" focused={false} {...handlers} />,
    );
    expect(html).toContain('A and B must reference two independently selectable committed outputs.');
    expect(html).toContain('data-preview-visible="false"');
  });

  it('keeps the Compare curtain in an unscaled overlay aligned to the transformed image', () => {
    const html = renderToStaticMarkup(
      <PreviewSurface canvasRef={createRef<HTMLCanvasElement>()} previews={[preview, demPreview]} nativePreview={null} fileName="frame.dng" frameCount={3} sample={null} mode="compare" selectedNode={null} previewCapabilities={{ rgb2yuv: true, dem: true }} nodeOptions={nodes} compareA="rgb2yuv" compareB="dem" focused={false} {...handlers} />,
    );
    const viewportStart = html.indexOf('class="preview-viewport ');
    const viewportEnd = html.indexOf('</div>', viewportStart);
    const overlayStart = html.indexOf('class="preview-curtain-overlay"');
    const curtainStart = html.indexOf('class="preview-curtain"');
    expect(viewportStart).toBeGreaterThan(-1);
    expect(overlayStart).toBeGreaterThan(viewportEnd);
    expect(curtainStart).toBeGreaterThan(overlayStart);
    expect(html.slice(overlayStart, curtainStart)).not.toContain('scale(');
  });

  it('centers the image element before applying Fit or 1:1 scale', () => {
    const html = render('final');
    expect(html).toContain('transform:translate(calc(-50% + 0px), calc(-50% + 0px)) scale(1)');
  });

  it('shows Compare for two identity-matched committed outputs', () => {
    const html = renderToStaticMarkup(
      <PreviewSurface canvasRef={createRef<HTMLCanvasElement>()} previews={[preview, demPreview]} nativePreview={null} fileName="frame.dng" frameCount={3} sample={null} mode="compare" selectedNode={null} previewCapabilities={{ rgb2yuv: true, dem: true }} nodeOptions={nodes} compareA="rgb2yuv" compareB="dem" focused={false} {...handlers} />,
    );
    expect(html).toContain('data-preview-visible="true"');
    expect(html).not.toContain('Preview unavailable');
  });

  it('uses fallback PNG pixels for 1:1 while reporting native output extent', () => {
    const html = renderToStaticMarkup(
      <PreviewSurface canvasRef={createRef<HTMLCanvasElement>()} previews={[]} nativePreview={{ dataUrl: 'data:image/png;base64,native', width: 624, height: 467, outputWidth: 3744, outputHeight: 2800, nodeId: 'rgb2yuv', portId: 'out', frameIndex: 0 }} fileName="native.dng" frameCount={1} sample={null} mode="final" selectedNode={null} previewCapabilities={{ rgb2yuv: true }} nodeOptions={nodes} compareA="rgb2yuv" compareB="dem" focused={false} {...handlers} />,
    );
    expect(html).toContain('width:624px;height:467px');
    expect(html).toContain('3744 × 2800');
    expect(html).toContain('native.dng');
  });
});
