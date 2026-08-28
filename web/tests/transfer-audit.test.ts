import { describe, expect, it } from 'vitest';

import { TransferAudit, ZeroCopyViolation } from '../src/gpu/transfer-audit.js';

describe('TransferAudit', () => {
  it('allows exactly one declared RAW upload', () => {
    const audit = new TransferAudit();

    audit.recordRawUpload(18_432);

    expect(audit.snapshot()).toEqual({ hostReadBytes: 0, hostWriteBytes: 18_432, gpuCopyBytes: 0 });
  });

  it('rejects a second RAW upload in one run', () => {
    const audit = new TransferAudit();
    audit.recordRawUpload(18_432);

    expect(() => audit.recordRawUpload(18_432)).toThrow(ZeroCopyViolation);
  });

  it('rejects every host readback', () => {
    const audit = new TransferAudit();

    expect(() => audit.recordHostRead(4)).toThrow(ZeroCopyViolation);
  });
});
