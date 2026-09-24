import { describe, expect, it } from 'vitest';
import { chromiumTimeNow } from '../src/chromium-time.js';

describe('chromium time', () => {
  it('timestamp is reasonable', () => {
    const ts = BigInt(chromiumTimeNow());
    // Post-2024 in Chromium epoch: 2024-01-01 ~= 13_348_876_800_000_000
    expect(ts > 13_348_876_800_000_000n).toBe(true);
    // Pre-2100 in Chromium epoch: 2100-01-01 ~= 15_745_036_800_000_000
    expect(ts < 15_745_036_800_000_000n).toBe(true);
  });

  it('timestamp format', () => {
    const ts = chromiumTimeNow();
    expect(ts.length).toBeGreaterThan(0);
    expect(ts).toMatch(/^\d+$/);
  });
});