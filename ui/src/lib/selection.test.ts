import { describe, expect, it } from 'vitest';
import { run } from './selection';

describe('run', () => {
  it('is every position from the photo clicked before to this one, in either direction', () => {
    expect(run(4, 7)).toEqual([4, 5, 6, 7]);
    expect(run(4, 2)).toEqual([2, 3, 4]);
  });
  it('is just the photo itself when none was clicked before, or the same one is clicked', () => {
    expect(run(null, 5)).toEqual([5]);
    expect(run(5, 5)).toEqual([5]);
  });
});
