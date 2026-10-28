import { describe, expect, it } from 'vitest';
import { badAddress } from './emailTo';

describe('badAddress', () => {
  it('is null when the box is empty or every address looks right', () => {
    expect(badAddress('')).toBeNull();
    expect(badAddress('  ')).toBeNull();
    expect(badAddress('a@b.com; c@d.org,  e@f.net')).toBeNull();
  });
  it('names the first address that does not look like name@host', () => {
    expect(badAddress('a@b.com nobody c@d.org')).toBe('nobody');
    for (const bad of ['a@b', '@b.com', 'a@@b.com', 'a"b@c.com', '<a@b.com>', 'zoë@b.com']) expect(badAddress(bad)).toBe(bad);
  });
});
