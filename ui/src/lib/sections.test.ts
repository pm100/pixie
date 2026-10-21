import { describe, expect, it } from 'vitest';
import { cleanCollapsed, toggleSection } from './sections';

describe('toggleSection', () => {
  it('collapses an open section', () => expect(toggleSection([], 'people')).toEqual(['people']));
  it('opens a collapsed section and leaves the others alone', () => expect(toggleSection(['people', 'tags'], 'people')).toEqual(['tags']));
});

describe('cleanCollapsed', () => {
  it('starts with everything open', () => {
    expect(cleanCollapsed(undefined)).toEqual([]);
    expect(cleanCollapsed('people')).toEqual([]);
  });
  it('drops sections that do not exist and repeats', () => expect(cleanCollapsed(['tags', 'nope', 'tags', 7, 'people'])).toEqual(['people', 'tags']));
});
