import { describe, expect, it } from 'vitest';
import { missingFromMap } from './mapNote';

describe('missingFromMap', () => {
  it('says nothing when every item has a position', () => {
    expect(missingFromMap(0)).toBeNull();
  });
  it('agrees with one item', () => {
    expect(missingFromMap(1)).toBe('Reminder: 1 item has no map information, so it is missing from this display.');
  });
  it('agrees with several, with thousands separated', () => {
    expect(missingFromMap(24)).toBe('Reminder: 24 items have no map information, so they are missing from this display.');
    expect(missingFromMap(1312)).toBe('Reminder: 1,312 items have no map information, so they are missing from this display.');
  });
});
