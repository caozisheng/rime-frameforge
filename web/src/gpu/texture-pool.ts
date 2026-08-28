export interface TextureKey {
  readonly domain: string;
  readonly format: GPUTextureFormat;
  readonly width: number;
  readonly height: number;
  readonly usage: GPUTextureUsageFlags;
}

export interface TextureLease {
  readonly leaseId: number;
  readonly resourceId: number;
  readonly generation: number;
  readonly key: TextureKey;
  readonly texture: GPUTexture;
}

interface PoolEntry {
  readonly resourceId: number;
  generation: number;
  readonly key: TextureKey;
  readonly texture: GPUTexture;
  free: boolean;
}

export class StaleTextureLease extends Error {
  public constructor(message: string) {
    super(message);
    this.name = 'StaleTextureLease';
  }
}

export class GpuTexturePool {
  readonly #entries = new Map<number, PoolEntry>();
  #generation: number;
  #nextResourceId = 1;
  #nextLeaseId = 1;

  public constructor(generation: number) {
    this.#generation = generation;
  }

  public acquire(key: TextureKey, factory: () => GPUTexture): TextureLease {
    const reusable = [...this.#entries.values()].find(
      (entry) => entry.free && sameKey(entry.key, key),
    );
    const entry = reusable ?? this.createEntry(key, factory);
    entry.generation = this.#generation;
    entry.free = false;
    return {
      leaseId: this.#nextLeaseId++,
      resourceId: entry.resourceId,
      generation: this.#generation,
      key,
      texture: entry.texture,
    };
  }

  public resolve(lease: TextureLease): GPUTexture {
    const entry = this.#entries.get(lease.resourceId);
    if (entry === undefined || lease.generation !== this.#generation || entry.generation !== this.#generation || entry.free) {
      throw new StaleTextureLease(`texture lease ${lease.leaseId} is no longer live`);
    }
    return entry.texture;
  }

  public release(lease: TextureLease): void {
    const entry = this.#entries.get(lease.resourceId);
    if (entry === undefined || lease.generation !== this.#generation || entry.generation !== this.#generation || entry.free) {
      throw new StaleTextureLease(`texture lease ${lease.leaseId} cannot be released`);
    }
    entry.free = true;
  }

  public invalidateGeneration(nextGeneration: number): void {
    for (const entry of this.#entries.values()) {
      entry.free = true;
      entry.generation = nextGeneration;
    }
    this.#generation = nextGeneration;
  }

  private createEntry(key: TextureKey, factory: () => GPUTexture): PoolEntry {
    const entry: PoolEntry = {
      resourceId: this.#nextResourceId++,
      generation: this.#generation,
      key,
      texture: factory(),
      free: false,
    };
    this.#entries.set(entry.resourceId, entry);
    return entry;
  }
}

function sameKey(left: TextureKey, right: TextureKey): boolean {
  return left.domain === right.domain && left.format === right.format && left.width === right.width && left.height === right.height && left.usage === right.usage;
}
