import { describe, expect, it } from 'vitest';
import { DEFAULT_DATE_RULES, ODD_YEARS, parseBeforeYear, withBeforeYear, withOddYear } from './dateRules';

describe('the defaults', () => {
  it('count dates before 1980 as missing, and use the file date', () => {
    expect(DEFAULT_DATE_RULES).toEqual({ odd_years: [1904, 1970, 1980, 2000], before_year: 1980, file_when_none: true, file_when_odd: true });
  });
});

describe('withOddYear', () => {
  it('ticks and unticks a year, keeping the others in order', () => {
    expect(withOddYear(DEFAULT_DATE_RULES, 1980, false).odd_years).toEqual([1904, 1970, 2000]);
    expect(withOddYear({ ...DEFAULT_DATE_RULES, odd_years: [2000] }, 1970, true).odd_years).toEqual([1970, 2000]);
  });
  it('lists only years that can be chosen, once each', () => {
    expect(withOddYear(DEFAULT_DATE_RULES, 1980, true).odd_years).toEqual([...ODD_YEARS]);
    expect(withOddYear(DEFAULT_DATE_RULES, 1999, true).odd_years).toEqual([...ODD_YEARS]);
  });
});

describe('parseBeforeYear', () => {
  it('reads a year, and blank or nonsense as none', () => {
    expect(parseBeforeYear('1990')).toBe(1990);
    expect(parseBeforeYear(' 1985 ')).toBe(1985);
    expect(parseBeforeYear('')).toBeNull();
    expect(parseBeforeYear('soon')).toBeNull();
  });
  it('keeps to years a photo can have', () => {
    expect(parseBeforeYear('1200')).toBe(1826);
    expect(parseBeforeYear('9999')).toBe(new Date().getFullYear());
  });
  it('is set on the rules as typed', () => {
    expect(withBeforeYear(DEFAULT_DATE_RULES, '1990').before_year).toBe(1990);
    expect(withBeforeYear({ ...DEFAULT_DATE_RULES, before_year: 1990 }, '').before_year).toBeNull();
  });
});
