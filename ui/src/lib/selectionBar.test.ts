import { describe, expect, it } from 'vitest';
import { selectionLabel } from './selectionBar';

describe('selectionLabel', () => {
  it('is empty with nothing selected', () => {
    expect(selectionLabel(0)).toBe('');
    expect(selectionLabel(-3)).toBe('');
  });
  it('counts what is selected', () => {
    expect(selectionLabel(1)).toBe('1 selected');
    expect(selectionLabel(12)).toBe('12 selected');
    expect(selectionLabel(1234)).toBe('1,234 selected');
  });
});
