import { describe, expect, it } from 'vitest';
import { buildLine, flatten, matches, type NoticeGroup } from './licences';

const groups: NoticeGroup[] = [
  { license: 'MIT', text: 'MIT text', packages: [{ name: 'serde', version: '1.0.0', url: 'u' }, { name: 'anyhow', version: '1.0.1', url: 'u' }] },
  { license: 'LGPL-3.0-or-later', text: 'LGPL text', packages: [{ name: 'libheif', version: '', url: 'u', note: 'replaceable' }] },
];

describe('licences', () => {
  it('lists every piece of software once, in name order, with its own licence and text', () => {
    const l = flatten(groups);
    expect(l.map((n) => n.name)).toEqual(['anyhow', 'libheif', 'serde']);
    expect(l[1]).toMatchObject({ license: 'LGPL-3.0-or-later', text: 'LGPL text', note: 'replaceable' });
  });
  it('gives every row its own number, even when one package has several licence files', () => {
    const two: NoticeGroup[] = [
      { license: 'MIT', text: 'a', packages: [{ name: 'ring', version: '1', url: 'u' }] },
      { license: 'MIT', text: 'b', packages: [{ name: 'ring', version: '1', url: 'u' }] },
    ];
    const ids = flatten(two).map((n) => n.id);
    expect(new Set(ids).size).toBe(2);
  });
  it('finds software by part of its name or its licence, ignoring case', () => {
    const l = flatten(groups);
    expect(l.filter((n) => matches(n, 'SER')).map((n) => n.name)).toEqual(['serde']);
    expect(l.filter((n) => matches(n, 'lgpl')).map((n) => n.name)).toEqual(['libheif']);
    expect(l.filter((n) => matches(n, '  ')).length).toBe(3);
  });
  it('says which commit it was built from, and when that build had changes not yet committed', () => {
    expect(buildLine({ commit: '2dc2e6b', modified: false })).toBe('Build 2dc2e6b');
    expect(buildLine({ commit: '2dc2e6b', modified: true })).toBe('Build 2dc2e6b (with changes not yet committed)');
    expect(buildLine({ commit: '', modified: false })).toBe('');
    expect(buildLine(null)).toBe('');
  });
});
