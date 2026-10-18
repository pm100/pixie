/** The positions the filmstrip shows around `index`: [lo, hi), clamped to the list. */
export function filmRange(index: number, total: number, radius: number): { lo: number; hi: number } {
  if (total <= 0) return { lo: 0, hi: 0 };
  return { lo: Math.max(0, index - radius), hi: Math.min(total, index + radius + 1) };
}
