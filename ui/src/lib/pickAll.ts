/** Whether "Pick all" is available for what the filters show now, and the tooltip that says what it does or why not. There is no limit on how many. */
export function pickAllState(shown: number, onlyPicked: boolean, pickedNow = 0): { enabled: boolean; title: string } {
  if (onlyPicked) return { enabled: false, title: 'The photos shown are already the picked ones' };
  if (shown === 0) return { enabled: false, title: 'No photos are shown' };
  const all = shown === 1 ? 'Pick the 1 photo shown' : `Pick all ${shown.toLocaleString()} photos shown`;
  // The photos are added to the picks already made.
  return { enabled: true, title: pickedNow > 0 ? `${all}, adding them to the ${pickedNow.toLocaleString()} picked now` : all };
}
