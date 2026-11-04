import { describe, expect, it } from 'vitest';
import { hexToHsv, hsvToHex, normaliseHex } from './colour';

describe('normaliseHex', () => {
  it('accepts 3 and 6 digits, with or without #, in any case', () => {
    expect(normaliseHex('#ABC')).toBe('#aabbcc');
    expect(normaliseHex(' 1a2B3c ')).toBe('#1a2b3c');
  });
  it('refuses anything else', () => {
    for (const t of ['', '#12', '#12345', 'ggg', '#1234567']) expect(normaliseHex(t), t).toBeNull();
  });
});

describe('hsv', () => {
  it('knows the primaries', () => {
    expect(hexToHsv('#ff0000')).toEqual({ h: 0, s: 1, v: 1 });
    expect(hexToHsv('#00ff00').h).toBe(120);
    expect(hexToHsv('#0000ff').h).toBe(240);
    expect(hexToHsv('#ffffff')).toEqual({ h: 0, s: 0, v: 1 });
    expect(hexToHsv('#000000').v).toBe(0);
  });
  it('goes there and back to the same colour', () => {
    for (const hex of ['#d93025', '#2f6fed', '#f3ead8', '#808080', '#123456', '#ffffff', '#000000']) expect(hsvToHex(hexToHsv(hex)), hex).toBe(hex);
  });
});
