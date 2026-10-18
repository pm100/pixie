import { describe, expect, it } from 'vitest';
import { periodRange } from './periods';

const utc = (y: number, m: number, d: number) => Date.UTC(y, m - 1, d) / 1000;

describe('periodRange', () => {
  it('a year is January 1st to the next January 1st, UTC', () => {
    expect(periodRange('y:2025')).toEqual({ since: utc(2025, 1, 1), until: utc(2026, 1, 1) });
  });
  it('a month runs to the first of the next month, including December into January', () => {
    expect(periodRange('m:2025-06')).toEqual({ since: utc(2025, 6, 1), until: utc(2025, 7, 1) });
    expect(periodRange('m:2025-12')).toEqual({ since: utc(2025, 12, 1), until: utc(2026, 1, 1) });
    expect(periodRange('m:2024-02')).toEqual({ since: utc(2024, 2, 1), until: utc(2024, 3, 1) }); // leap year
  });
  it('a day is 24 hours', () => {
    expect(periodRange('d:2025-06-14')).toEqual({ since: utc(2025, 6, 14), until: utc(2025, 6, 15) });
    expect(periodRange('d:2025-12-31')).toEqual({ since: utc(2025, 12, 31), until: utc(2026, 1, 1) });
  });
  it('anything else is not a period', () => {
    for (const k of ['undated', 'decade:2010', 'y:abcd', 'm:2025-13', 'm:2025-00', 'd:2025-02-30', '', 'x:1']) expect(periodRange(k)).toBeNull();
  });
});
