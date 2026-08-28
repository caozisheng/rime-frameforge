import presentShader from '../../../crates/rime-isp/src/shaders/present.wgsl?raw';

export class GpuPreviewPresenter {
  readonly #context: GPUCanvasContext;
  readonly #device: GPUDevice;
  readonly #pipeline: GPURenderPipeline;
  readonly #bindGroupLayout: GPUBindGroupLayout;

  public constructor(gpuContext: GPUCanvasContext, device: GPUDevice, canvasFormat: GPUTextureFormat) {
    this.#context = gpuContext;
    this.#device = device;
    const module = device.createShaderModule({ code: presentShader });
    this.#pipeline = device.createRenderPipeline({
      layout: 'auto',
      vertex: { module, entryPoint: 'present_vertex' },
      fragment: { module, entryPoint: 'present_fragment', targets: [{ format: canvasFormat }] },
      primitive: { topology: 'triangle-list' },
    });
    this.#bindGroupLayout = this.#pipeline.getBindGroupLayout(0);
  }

  public async render(texture: GPUTexture): Promise<void> {
    const output = this.#context.getCurrentTexture().createView();
    const bindGroup = this.#device.createBindGroup({
      layout: this.#bindGroupLayout,
      entries: [{ binding: 0, resource: texture.createView() }],
    });
    const encoder = this.#device.createCommandEncoder({ label: 'normal-preview' });
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: output,
        clearValue: { r: 0, g: 0, b: 0, a: 1 },
        loadOp: 'clear',
        storeOp: 'store',
      }],
    });
    pass.setPipeline(this.#pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.draw(3);
    pass.end();
    this.#device.queue.submit([encoder.finish()]);
    await this.#device.queue.onSubmittedWorkDone();
  }

  public clear(): void {
    const encoder = this.#device.createCommandEncoder({ label: 'normal-preview-clear' });
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.#context.getCurrentTexture().createView(),
        clearValue: { r: 0.02, g: 0.03, b: 0.04, a: 1 },
        loadOp: 'clear',
        storeOp: 'store',
      }],
    });
    pass.end();
    this.#device.queue.submit([encoder.finish()]);
  }
}
