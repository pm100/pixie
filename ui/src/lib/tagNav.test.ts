import { describe, expect, it } from 'vitest';
import { categoryOf, hitsActive, tagLabel, tagTree } from './tagNav';

const t = (tag: string, count: number, category: string, category_photos: number) => ({ tag, count, category, category_photos });
const list = [t('beach', 9, 'Settings', 12), t('lake', 4, 'Settings', 12), t('dog', 7, 'Animals', 8), t('museum', 3, 'Places', 3), t('cat', 2, 'Animals', 8)];

describe('tagLabel', () => {
  it('capitalises the first letter only', () => {
    expect(tagLabel('beach')).toBe('Beach');
    expect(tagLabel('city street')).toBe('City street');
    expect(tagLabel('black and white photo')).toBe('Black and white photo');
    expect(tagLabel('')).toBe('');
  });
});

describe('tagTree', () => {
  it('groups tags by category, categories in order of first appearance and tags in list order', () => {
    expect(tagTree(list)).toEqual([
      { category: 'Settings', photos: 12, tags: [list[0], list[1]] },
      { category: 'Animals', photos: 8, tags: [list[2], list[4]] },
      { category: 'Places', photos: 3, tags: [list[3]] },
    ]);
  });
  it('is empty for no tags', () => {
    expect(tagTree([])).toEqual([]);
  });
});

describe('categoryOf', () => {
  it('finds the category of a listed tag, and nothing for none or an unlisted one', () => {
    expect(categoryOf(list, 'museum')).toBe('Places');
    expect(categoryOf(list, null)).toBeNull();
    expect(categoryOf(list, 'unicorn')).toBeNull();
  });
});

describe('hitsActive', () => {
  it('is on when a tag or a person is applied', () => {
    expect(hitsActive(null, null)).toBe(false);
    expect(hitsActive('beach', null)).toBe(true);
    expect(hitsActive(null, 3)).toBe(true);
    expect(hitsActive('beach', 3)).toBe(true);
  });
  it('is on when a camera is applied', () => {
    expect(hitsActive(null, null, 'Pixel 7')).toBe(true);
    expect(hitsActive(null, null, null)).toBe(false);
  });
});
