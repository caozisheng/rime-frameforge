export interface GpuResourceRef {
  readonly id: number;
  readonly label: string;
  readonly generation: number;
}

export class StaleGpuResource extends Error {
  public constructor(reference: GpuResourceRef, generation: number) {
    super(`resource ${reference.label} belongs to generation ${reference.generation}, current ${generation}`);
    this.name = 'StaleGpuResource';
  }
}

interface RegisteredTexture {
  readonly reference: GpuResourceRef;
  readonly texture: GPUTexture;
  readonly destroyOnInvalidate: boolean;
}

export class GpuResourceRegistry {
  readonly #textures = new Map<number, RegisteredTexture>();
  #nextId = 1;
  #generation: number;

  public constructor(generation: number) {
    this.#generation = generation;
  }

  public register(
    label: string,
    texture: GPUTexture,
    options: { readonly destroyOnInvalidate?: boolean } = {},
  ): GpuResourceRef {
    const reference = { id: this.#nextId++, label, generation: this.#generation };
    this.#textures.set(reference.id, {
      reference,
      texture,
      destroyOnInvalidate: options.destroyOnInvalidate ?? true,
    });
    return reference;
  }

  public resolve(reference: GpuResourceRef): GPUTexture {
    const registered = this.#textures.get(reference.id);
    if (
      reference.generation !== this.#generation ||
      registered?.reference.generation !== this.#generation
    ) {
      throw new StaleGpuResource(reference, this.#generation);
    }
    return registered.texture;
  }
  public release(reference: GpuResourceRef): void {
    const registered = this.#textures.get(reference.id);
    if (registered === undefined) {
      throw new StaleGpuResource(reference, this.#generation);
    }
    if (registered.destroyOnInvalidate) registered.texture.destroy();
    this.#textures.delete(reference.id);
  }


  public invalidate(nextGeneration: number): void {
    for (const registered of this.#textures.values()) {
      if (registered.destroyOnInvalidate) registered.texture.destroy();
    }
    this.#textures.clear();
    this.#generation = nextGeneration;
  }
}
