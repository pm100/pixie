/** Which of the background analyses are on (they all are until the person says otherwise); kept in the library. */
export type AnalysisOptions = { tags: boolean; faces: boolean; dupes: boolean };

export const ALL_ON: AnalysisOptions = { tags: true, faces: true, dupes: true };

/** The options as the backend sent them (any piece missing counts as on). */
export function parseOptions(raw: unknown): AnalysisOptions {
  const o = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {};
  const on = (v: unknown) => v !== false;
  return { tags: on(o.tags), faces: on(o.faces), dupes: on(o.dupes) };
}

/** What "turned off" means on the screen: the people and tag choices that were made in the tree no longer apply. */
export function clearsFilters(next: AnalysisOptions): { tag: boolean; person: boolean } {
  return { tag: !next.tags, person: !next.faces };
}
