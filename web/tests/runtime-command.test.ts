import { describe, expect, it } from 'vitest';

import type { RuntimeCommand } from '../src/contracts.js';

describe('runtime bypass command contract', () => {
  it('represents graph bypass updates as a JSON config command', () => {
    const command: RuntimeCommand = { type: 'set_bypass_config', config: '{"graph_id":"normal","modules":[]}' };
    expect(command).toEqual({ type: 'set_bypass_config', config: '{"graph_id":"normal","modules":[]}' });
  });
  it('represents DRC IQ updates as a dedicated typed config command', () => {
    const command: RuntimeCommand = { type: 'set_drc_iq_parameters', config: '{"drc_gain_offset_ev":0.5,"knee":1.2,"amplifier":0.8}' };
    expect(command.type).toBe('set_drc_iq_parameters');
  });
});
