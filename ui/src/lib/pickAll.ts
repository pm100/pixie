/** The most photos "Pick all shown" will pick at once. Picking reads the photos in batches and the Tools lists draw a tile for each, so a limit keeps it quick (see #79). */
export const PICK_ALL_LIMIT = 2000;

/** Whether "Pick all shown" is available for what the filters show now, and the tooltip that says what it does or why not. */
export function pickAllState(shown: number, onlyPicked: boolean, limit: number = PICK_ALL_LIMIT): { enabled: boolean; title: string } {
  if (onlyPicked) return { enabled: false, title: 'The photos shown are already the picked ones' };
  if (shown === 0) return { enabled: false, title: 'No photos are shown' };
  if (shown > limit) return { enabled: false, title: `${shown.toLocaleString()} photos are shown; at most ${limit.toLocaleString()} can be picked at once. Narrow the filters first.` };
  return { enabled: true, title: `Pick all ${shown.toLocaleString()} photos shown` };
}
