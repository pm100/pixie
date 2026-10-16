import { describe, expect, it } from 'vitest';
import { neighbors } from './neighbors';

describe('neighbors', () => {
  it('lists nearby indices nearest first, ahead before behind', () => {
    expect(neighbors(5, 100, 2)).toEqual([6, 4, 7, 3]);
  });
  it('stops at the start of the library', () => {
    expect(neighbors(0, 100, 2)).toEqual([1, 2]);
    expect(neighbors(1, 100, 3)).toEqual([2, 0, 3, 4]);
  });
  it('stops at the end of the library', () => {
    expect(neighbors(99, 100, 2)).toEqual([98, 97]);
  });
  it('is empty for a single photo or zero radius', () => {
    expect(neighbors(0, 1, 3)).toEqual([]);
    expect(neighbors(5, 100, 0)).toEqual([]);
  });
});
