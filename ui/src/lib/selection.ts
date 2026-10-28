/** The photo chosen in the grid is one position in the current view (its details show on the right), or none; it is cleared when
 *  the view changes. Keeping several photos together is what Picked is for. */

/** Every position from `from` (the photo clicked before; none: just `to`) to `to`, in order: what a Shift-click picks. */
export function run(from: number | null, to: number): number[] {
  const [a, b] = from === null || from > to ? [to, from ?? to] : [from, to];
  return Array.from({ length: b - a + 1 }, (_, k) => a + k);
}
