import { describe, expect, it } from 'vitest';
import { searchActive, tagLabel, tagRows } from './tagNav';

const tags = (n: number) => Array.from({ length: n }, (_, i) => ({ tag: `t${i}`, count: n - i }));

describe('tagLabel', () => {
  it('capitalises the first letter only', () => {
    expect(tagLabel('beach')).toBe('Beach');
    expect(tagLabel('city street')).toBe('City street');
    expect(tagLabel('black and white photo')).toBe('Black and white photo');
    expect(tagLabel('')).toBe('');
  });
});

describe('tagRows', () => {
  it('shows the most common few and says how many are hidden', () => {
    const r = tagRows(tags(30), null, false, 12);
    expect(r.rows).toHaveLength(12);
    expect(r.hidden).toBe(18);
  });
  it('shows everything when expanded', () => {
    expect(tagRows(tags(30), null, true, 12)).toEqual({ rows: tags(30), hidden: 0 });
  });
  it('never hides the chosen tag, even if it is rare', () => {
    const r = tagRows(tags(30), 't25', false, 12);
    expect(r.rows).toHaveLength(12);
    expect(r.rows.map((x) => x.tag)).toContain('t25');
    expect(r.hidden).toBe(18);
  });
  it('has nothing hidden for a short list', () => {
    expect(tagRows(tags(5), null, false, 12)).toEqual({ rows: tags(5), hidden: 0 });
  });
});

describe('searchActive', () => {
  it('is on when either a text search or a tag is applied', () => {
    expect(searchActive(null, null)).toBe(false);
    expect(searchActive('dog', null)).toBe(true);
    expect(searchActive(null, 'beach')).toBe(true);
    expect(searchActive('dog', 'beach')).toBe(true);
  });
});
