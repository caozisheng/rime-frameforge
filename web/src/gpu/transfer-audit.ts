import type { TransferAuditSnapshot } from '../contracts.js';

export class ZeroCopyViolation extends Error {
  public constructor(message: string) {
    super(message);
    this.name = 'ZeroCopyViolation';
  }
}

export class TransferAudit {
  readonly #snapshot = {
    hostReadBytes: 0,
    hostWriteBytes: 0,
    gpuCopyBytes: 0,
  };
  #rawUploaded = false;

  public recordRawUpload(bytes: number): void {
    if (this.#rawUploaded) {
      throw new ZeroCopyViolation('RAW may be uploaded exactly once per run');
    }
    if (!Number.isSafeInteger(bytes) || bytes <= 0) {
      throw new ZeroCopyViolation('RAW upload size must be a positive integer');
    }
    this.#rawUploaded = true;
    this.#snapshot.hostWriteBytes += bytes;
  }

  public recordHostRead(bytes: number): never {
    throw new ZeroCopyViolation(`host readback is forbidden (${bytes} bytes requested)`);
  }

  public recordGpuCopy(bytes: number, reason?: string): void {
    if (reason === undefined || reason.length === 0) {
      throw new ZeroCopyViolation('every GPU copy requires a declared reason');
    }
    this.#snapshot.gpuCopyBytes += bytes;
  }

  public snapshot(): TransferAuditSnapshot {
    return { ...this.#snapshot };
  }
}
