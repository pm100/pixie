/** What was changed in Browse: a filter chip (a tag, a person, Picked, a camera, the recent days) or a choice in the tree (a
 *  folder, a trip, a date, photos or videos, recent, everything). */
export type MapChange = 'filter' | 'selection';

/** Whether the map zooms to fit the photos after a change.
 *  - Filtering by the map: never, since the map is how the user chose the area.
 *  - A choice in the tree: always, so the map shows where the choice is.
 *  - A filter chip: only while the map is where the app put it, so a map the user moved by hand stays there. */
export const shouldFit = (change: MapChange, areaOnly: boolean, movedByHand: boolean): boolean =>
  !areaOnly && (change === 'selection' || !movedByHand);
