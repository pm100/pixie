const DELAYS_MS = [400, 1500, 4000];

/** Delay before retrying a thumbnail that failed to load, or null to give up. */
export function retryDelay(attemptsSoFar: number): number | null {
  return attemptsSoFar < DELAYS_MS.length ? DELAYS_MS[attemptsSoFar] : null;
}
