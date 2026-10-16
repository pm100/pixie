/** Indices within `radius` of `index`, nearest first, ahead (higher) before behind at equal distance. */
export function neighbors(index: number, total: number, radius: number): number[] {
  const out: number[] = [];
  for (let d = 1; d <= radius; d++) {
    if (index + d < total) out.push(index + d);
    if (index - d >= 0) out.push(index - d);
  }
  return out;
}
