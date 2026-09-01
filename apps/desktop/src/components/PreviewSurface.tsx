import type { PreviewDescriptor } from '../../../../web/src/contracts.js';

interface PreviewSurfaceProps {
  readonly canvasRef: React.RefObject<HTMLCanvasElement | null>;
  readonly preview: PreviewDescriptor | null;
  readonly nativePreviewDataUrl: string | undefined;
  readonly fileName: string | null;
  readonly frameCount: number;
}

export function PreviewSurface({ canvasRef, preview, nativePreviewDataUrl, fileName, frameCount }: PreviewSurfaceProps) {
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
          {nativePreviewDataUrl === undefined ? <canvas ref={canvasRef} width={640} height={480} aria-label="Normal Graph GPU preview" /> : <img src={nativePreviewDataUrl} alt="Native Normal Graph preview" />}
          {preview === null && nativePreviewDataUrl === undefined && <div className="no-frame">Run or Step to commit the loaded frame</div>}
        </div>
      </div>
      <div className="preview-meta">
        <span>frame <b>{preview === null ? '—' : `${preview.frameIndex + 1}${frameCount > 1 ? ` / ${frameCount}` : ''}`}</b></span>
        <span>file <b>{preview === null ? '—' : fileName ?? '—'}</b></span>
        <span>domain <b>{preview?.domain ?? '—'}</b></span>
        <span>format <b>{preview?.format ?? '—'}</b></span>
        <span>extent <b>{preview ? `${preview.width} × ${preview.height}` : '—'}</b></span>
        <span>generation <b>{preview?.gpuGeneration ?? '—'}</b></span>
      </div>
    </section>
  );
}
