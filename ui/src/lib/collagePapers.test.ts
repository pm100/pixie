import { describe, expect, it } from 'vitest';
import { defaultPaperId, findPaper, paperGroups, usesInches } from './collagePapers';

describe('usesInches', () => {
  it('is true for the countries that print in inches', () => {
    for (const l of ['en-US', 'en-CA', 'es-MX', 'fil-PH', 'fr-CA', 'en_US']) expect(usesInches(l), l).toBe(true);
  });
  it('is false for everywhere else, for a language with no country, and for nothing', () => {
    for (const l of ['en', 'en-GB', 'de-DE', 'fr', '', 'xx_??']) expect(usesInches(l), l).toBe(false);
    expect(usesInches(undefined)).toBe(false);
  });
});

describe('paperGroups', () => {
  it('puts the prints of the region first', () => {
    expect(paperGroups('en-US')[0].label).toBe('Prints (inches)');
    expect(paperGroups('de-DE')[0].label).toBe('Prints (cm)');
    expect(paperGroups(undefined)[0].label).toBe('Prints (cm)');
  });
  it('lists every size once, with unique ids', () => {
    const all = paperGroups('en-US').flatMap((g) => g.sizes);
    expect(new Set(all.map((p) => p.id)).size).toBe(all.length);
    expect(paperGroups('en-US').flatMap((g) => g.sizes.map((s) => s.id)).sort()).toEqual(paperGroups('de-DE').flatMap((g) => g.sizes.map((s) => s.id)).sort());
  });
  it('has the sizes the shops sell', () => {
    const ids = paperGroups().flatMap((g) => g.sizes.map((s) => s.id));
    for (const id of ['4x6in', '8x10in', '24x36in', 'letter', '10x15cm', '20x30cm', '70x100cm', 'a4', 'a0', '61x91cm', '12x12in', '30x30cm']) expect(ids, id).toContain(id);
  });
  it('keeps every size portrait', () => {
    for (const p of paperGroups().flatMap((g) => g.sizes)) expect(p.h, p.id).toBeGreaterThanOrEqual(p.w);
  });
});

describe('findPaper and defaultPaperId', () => {
  it('finds a size by id', () => {
    expect(findPaper('8x10in')).toMatchObject({ w: 203.2, h: 254 });
    expect(findPaper('20x30cm')).toMatchObject({ w: 200, h: 300 });
    expect(findPaper('nope')).toBeUndefined();
  });
  it('starts on 8 × 10 in or 20 × 30 cm', () => {
    expect(defaultPaperId('en-US')).toBe('8x10in');
    expect(defaultPaperId('en-GB')).toBe('20x30cm');
    expect(findPaper(defaultPaperId('en-US'))).toBeDefined();
    expect(findPaper(defaultPaperId())).toBeDefined();
  });
});
