// Generic module-shader shell — the ONLY web-side WGSL runtime glue.
//
// Contract (mirrors rime-native-gpu's dispatch): each ISP module owns its
// algorithm WGSL in crates/rime-isp (single source, exported via the
// generated `*_pipeline.generated.ts` assets). This shell compiles a
// module entry verbatim and binds module-declared resources. It performs
// NO algorithm transformations — the module's own binding declarations
// are used as-is; only the backing GPU resources are supplied here.
//
// Port quantization (Rime.Q) is a property of the module output port and
// is applied by the owning module's entry (or its wrapper) — never by
// this shell.

export interface ModulePassResource {
  readonly texture?: GPUTexture;
  readonly buffer?: GPUBuffer;
}

export interface ModulePass {
  readonly label: string;
  readonly source: string;
  readonly entryPoint: string;
  readonly bindings: ReadonlyArray<{ binding: number; resource: ModulePassResource }>;
  readonly workgroups: readonly [number, number];
}

export class ModuleShaderRuntime {
  readonly #device: GPUDevice;
  #pipelines: Record<string, GPUComputePipeline> = {};

  public constructor(device: GPUDevice) {
    this.#device = device;
  }

  public encode(encoder: GPUCommandEncoder, pass: ModulePass): void {
    const pipeline = this.#pipelines[pass.label] ?? this.#compile(pass);
    const bindGroup = this.#device.createBindGroup({
      label: pass.label,
      layout: pipeline.getBindGroupLayout(0),
      entries: pass.bindings.map(({ binding, resource }): GPUBindGroupEntry => {
        const view = resource.texture?.createView();
        return view === undefined
          ? { binding, resource: { buffer: resource.buffer! } }
          : { binding, resource: view };
      }),
    });
    const compute = encoder.beginComputePass({ label: pass.label });
    compute.setPipeline(pipeline);
    compute.setBindGroup(0, bindGroup, []);
    compute.dispatchWorkgroups(pass.workgroups[0], pass.workgroups[1], 1);
    compute.end();
  }

  #compile(pass: ModulePass): GPUComputePipeline {
    const pipeline = this.#device.createComputePipeline({
      label: pass.label,
      layout: 'auto',
      compute: {
        module: this.#device.createShaderModule({ label: pass.label, code: pass.source }),
        entryPoint: pass.entryPoint,
      },
    });
    this.#pipelines[pass.label] = pipeline;
    return pipeline;
  }

  public dispose(): void {
    this.#pipelines = {};
  }
}
