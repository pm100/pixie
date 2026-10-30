/** The most photos "Pick all" will pick at once. Picking reads the photos in batches and the Tools lists draw a tile for each, so a limit keeps it quick (see #79). */
export const PICK_ALL_LIMIT = 2000;

/** Whether "Pick all" is available for what the filters show now, and the tooltip that says what it does or why not. */
export function pickAllState(shown: number, onlyPicked: boolean, limit: number = PICK_ALL_LIMIT, pickedNow = 0): { enabled: boolean; title: string } {
  if (onlyPicked) return { enabled: false, title: 'The photos shown are already the picked ones' };
  if (shown === 0) return { enabled: false, title: 'No photos are shown' };
  if (shown > limit) return { enabled: false, title: `${shown.toLocaleString()} photos are shown; at most ${limit.toLocaleString()} can be picked at once. Narrow the filters first.` };
  const all = shown === 1 ? 'Pick the 1 photo shown' : `Pick all ${shown.toLocaleString()} photos shown`;
  // The photos are added to the picks already made.
  return { enabled: true, title: pickedNow > 0 ? `${all}, adding them to the ${pickedNow.toLocaleString()} picked now` : all };
}
