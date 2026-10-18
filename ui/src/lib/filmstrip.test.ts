import { describe, expect, it } from 'vitest';
import { filmRange } from './filmstrip';

describe('filmRange', () => {
  it('is centred on the item', () => expect(filmRange(10, 100, 4)).toEqual({ lo: 6, hi: 15 }));
  it('is clamped at the start and the end', () => {
    expect(filmRange(1, 100, 4)).toEqual({ lo: 0, hi: 6 });
    expect(filmRange(98, 100, 4)).toEqual({ lo: 94, hi: 100 });
  });
  it('is empty with nothing to show and covers a short list', () => {
    expect(filmRange(0, 0, 4)).toEqual({ lo: 0, hi: 0 });
    expect(filmRange(1, 3, 4)).toEqual({ lo: 0, hi: 3 });
  });
});
