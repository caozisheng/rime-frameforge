import presentShader from '../../../crates/rime-isp/src/shaders/present.wgsl?raw';
import type { PreviewDescriptor } from '../contracts.js';

export interface PreviewView {
  readonly texture: GPUTexture;
  readonly descriptor: PreviewDescriptor;
}

export class GpuPreviewPresenter {
  readonly #context: GPUCanvasContext;
  readonly #device: GPUDevice;
  readonly #pipelines: Readonly<Record<'raw' | 'gray' | 'rgb' | 'yuv', GPURenderPipeline>>;

  public constructor(gpuContext: GPUCanvasContext, device: GPUDevice, canvasFormat: GPUTextureFormat) {
    this.#context = gpuContext;
    this.#device = device;
    const module = device.createShaderModule({ code: presentShader });
    const pipeline = (entryPoint: string): GPURenderPipeline => device.createRenderPipeline({
      layout: 'auto', vertex: { module, entryPoint: 'present_vertex' },
      fragment: { module, entryPoint, targets: [{ format: canvasFormat }] }, primitive: { topology: 'triangle-list' },
    });
    this.#pipelines = { raw: pipeline('present_raw'), gray: pipeline('present_gray'), rgb: pipeline('present_rgb'), yuv: pipeline('present_yuv') };
  }

  public encode(encoder: GPUCommandEncoder, a: PreviewView, b: PreviewView | null = null, curtain = 0.5): void {
    const output = this.#context.getCurrentTexture();
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: output.createView(), clearValue: { r: 0, g: 0, b: 0, a: 1 }, loadOp: 'clear', storeOp: 'store' }] });
    this.draw(pass, a);
    if (b !== null) {
      const width = output.width;
      const height = output.height;
      const x = Math.max(0, Math.min(width, Math.round(width * curtain)));
      if (x < width) {
        pass.setScissorRect(x, 0, width - x, height);
        this.draw(pass, b);
      }
    }
    pass.end();
  }

  public async render(a: PreviewView, b: PreviewView | null = null, curtain = 0.5): Promise<void> {
    const encoder = this.#device.createCommandEncoder({ label: 'normal-preview' });
    this.encode(encoder, a, b, curtain);
    this.#device.queue.submit([encoder.finish()]);
    await this.#device.queue.onSubmittedWorkDone();
  }

  public clear(): void {
    const encoder = this.#device.createCommandEncoder({ label: 'normal-preview-clear' });
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: this.#context.getCurrentTexture().createView(), clearValue: { r: 0.02, g: 0.03, b: 0.04, a: 1 }, loadOp: 'clear', storeOp: 'store' }] });
    pass.end();
    this.#device.queue.submit([encoder.finish()]);
  }

  private draw(pass: GPURenderPassEncoder, view: PreviewView): void {
    const pipeline = view.descriptor.format === 'r16_uint' ? this.#pipelines.raw
      : view.descriptor.presentation === 'raw_gray' ? this.#pipelines.gray
        : view.descriptor.presentation === 'yuv' ? this.#pipelines.yuv : this.#pipelines.rgb;
    const group = view.descriptor.format === 'r16_uint' ? 1 : 0;
    pass.setPipeline(pipeline);
    pass.setBindGroup(group, this.#device.createBindGroup({ layout: pipeline.getBindGroupLayout(group), entries: [{ binding: 0, resource: view.texture.createView() }] }));
    pass.draw(3);
  }
}
