import { plural } from './format';

/** The note under the map: how many items have no position and so are not on it. Nothing when none are missing. */
export function missingFromMap(n: number): string | null {
  if (n <= 0) return null;
  return n === 1 ? `Note: ${plural(n, 'item')} has no map information, so it is missing from this display.` : `Note: ${plural(n, 'item')} have no map information, so they are missing from this display.`;
}
