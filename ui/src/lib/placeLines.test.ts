import { describe, it, expect } from 'vitest';
import { placeLines } from './placeLines';

describe('placeLines', () => {
  it('shows a landmark with its locality underneath', () => {
    expect(placeLines({ landmark: 'Colosseum', locality: 'Roma, Lazio, Italia' })).toEqual({
      primary: 'Colosseum',
      secondary: 'Roma, Lazio, Italia',
    });
  });
  it('shows just the locality when there is no landmark', () => {
    expect(placeLines({ landmark: null, locality: 'Paris, France' })).toEqual({ primary: 'Paris, France', secondary: null });
  });
  it('shows a landmark alone when the locality is unknown', () => {
    expect(placeLines({ landmark: 'Bellagio', locality: null })).toEqual({ primary: 'Bellagio', secondary: null });
  });
  it('shows nothing for no place', () => {
    expect(placeLines(null)).toEqual({ primary: null, secondary: null });
  });
});
