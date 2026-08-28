import type { PreviewDescriptor } from '../../../../web/src/contracts.js';

interface PreviewSurfaceProps {
  readonly canvasRef: React.RefObject<HTMLCanvasElement | null>;
  readonly preview: PreviewDescriptor | null;
}

export function PreviewSurface({ canvasRef, preview }: PreviewSurfaceProps) {
  return (
    <section className="panel preview-panel" aria-labelledby="preview-heading">
      <div className="panel-heading compact">
        <div>
          <span className="eyebrow">GPU surface / final output</span>
          <h2 id="preview-heading">Preview</h2>
        </div>
        <span className="preview-label">{preview === null ? 'NO FRAME' : 'rgb2yuv.out'}</span>
      </div>
      <div className="preview-stage">
        <div className="preview-viewport">
          <canvas ref={canvasRef} width={640} height={480} aria-label="Normal Graph GPU preview" />
          {preview === null && <div className="no-frame">Run Step to commit frame 0</div>}
        </div>
      </div>
      <div className="preview-meta">
        <span>domain <b>{preview?.domain ?? '—'}</b></span>
        <span>format <b>{preview?.format ?? '—'}</b></span>
        <span>extent <b>{preview ? `${preview.width} × ${preview.height}` : '—'}</b></span>
        <span>generation <b>{preview?.gpuGeneration ?? '—'}</b></span>
      </div>
    </section>
  );
}
