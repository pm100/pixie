import { describe, expect, it } from 'vitest';
import { BAR_ACTIONS, DEFAULT_BAR, directCount, parseBar, split, toggled } from './actionBar';

describe('actionBar', () => {
  it('lists Email, Print and Tag first, then the other tools of Send, Create and Organize', () => {
    expect(BAR_ACTIONS.slice(0, 4).map((a) => a.id)).toEqual(['email', 'print', 'tag', 'share']);
    expect(BAR_ACTIONS.some((a) => a.id === 'duplicates' || a.id === 'bursts' || a.id === 'tags')).toBe(false);
    expect(BAR_ACTIONS.find((a) => a.id === 'tag')).toEqual({ id: 'tag', label: 'Tag…', built: true });
  });

  it('starts with Email, Print and Tag', () => {
    expect(parseBar(undefined)).toEqual(DEFAULT_BAR);
    expect(parseBar('nonsense')).toEqual(DEFAULT_BAR);
  });

  it('keeps only known actions, once each, in the bar\'s order', () => {
    expect(parseBar(['tag', 'email', 'email', 'bogus', 7])).toEqual(['email', 'tag']);
  });

  it('treats an empty choice as a choice', () => {
    expect(parseBar([])).toEqual([]);
  });

  it('switches one action on or off', () => {
    expect(toggled(['email', 'print'], 'tag', true)).toEqual(['email', 'print', 'tag']);
    expect(toggled(['email', 'print', 'tag'], 'print', false)).toEqual(['email', 'tag']);
  });

  it('shows as many buttons as fit, and never more than were chosen', () => {
    expect(directCount(2000, 3)).toBe(3);
    expect(directCount(900, 5)).toBe(3);
    expect(directCount(700, 3)).toBe(0);
    expect(directCount(100, 3)).toBe(0);
  });

  it('puts the buttons that do not fit under More', () => {
    expect(split(['email', 'print', 'tag'], 2)).toEqual({ direct: ['email', 'print'], folded: ['tag'] });
    expect(split(['email', 'print', 'tag'], 0)).toEqual({ direct: [], folded: ['email', 'print', 'tag'] });
  });

  it('leaves out an action that is not built, or not offered on this bar', () => {
    expect(split(['email', 'share'], 5).direct).toEqual(['email']);
    expect(split(['email', 'tag'], 5, (id) => id !== 'tag').direct).toEqual(['email']);
  });
});
