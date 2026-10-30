import { describe, expect, it } from 'vitest';
import { shouldFit } from './mapFit';

describe('shouldFit', () => {
  it('never moves a map that is filtering the list', () => {
    expect(shouldFit('filter', true, false)).toBe(false);
    expect(shouldFit('selection', true, false)).toBe(false);
    expect(shouldFit('filter', true, true)).toBe(false);
  });

  it('shows everything chosen in the tree when the map is only showing where things are', () => {
    expect(shouldFit('selection', false, false)).toBe(true);
    expect(shouldFit('selection', false, true)).toBe(true);
  });

  it('fits after a filter chip while the map is still where the app put it', () => {
    expect(shouldFit('filter', false, false)).toBe(true);
  });

  it('keeps a map the user has moved by hand when a filter chip changes', () => {
    expect(shouldFit('filter', false, true)).toBe(false);
  });
});
