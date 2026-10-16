/** Guards async results: only the response for the most recent request may be applied. */
export function createLatest() {
  let n = 0;
  return { next: () => ++n, isCurrent: (token: number) => token === n };
}
