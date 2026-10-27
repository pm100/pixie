import type { AnalysisStatus } from './analysis';

/** A person in the nav: `count` photos, `face_id` the face to show as their picture (mirrors the Rust `PersonInfo`). */
export type PersonInfo = { id: number; name: string | null; count: number; face_id: number | null };
/** A face in one photo; the box is a fraction (0..1) of the upright image (mirrors the Rust `FaceInfo`). */
export type FaceInfo = { id: number; person_id: number | null; person_name: string | null; x: number; y: number; w: number; h: number };
/** The face scan reports progress the same way the search analysis does; `rev` changes whenever who-is-who changes. */
export type PeopleStatus = AnalysisStatus & { rev: number };

const n = (x: number) => x.toLocaleString();

export const personLabel = (p: { name: string | null }): string => (p.name ? p.name : 'Unnamed person');

/** One line for Settings: what the face scan is doing right now. */
export function peopleLine(s: PeopleStatus): string {
  if (s.models === 'error') return `Could not get the face models: ${s.error ?? 'unknown error'}`;
  if (s.models === 'missing') return 'Face models not downloaded yet';
  if (s.models === 'downloading') {
    const pct = s.model_total > 0 ? Math.floor((s.model_done * 100) / s.model_total) : 0;
    return `Downloading face models… ${pct}%`;
  }
  if (s.error) return `Face finding is having trouble: ${s.error}`;
  if (s.total === 0) return 'No photos to check for faces yet';
  if (s.paused && s.done < s.total) return `Paused at ${n(s.done)} of ${n(s.total)}`;
  if (s.done >= s.total) return `Faces checked in ${s.total === 1 ? 'the' : 'all'} ${plural(s.total, 'photo')}`;
  return `Looking for people… ${n(s.done)} of ${plural(s.total, 'photo')}`;
}

/** Is it in the middle of looking for faces (models ready, not paused, not stuck, photos still to do)? */
export const lookingForPeople = (s: PeopleStatus): boolean =>
  s.models === 'ready' && !s.error && !s.paused && s.total > 0 && s.done < s.total;

/** One face to look at: the photo it is in (with that photo's version, to show it) and the face's box as fractions
 *  (0..1) of the upright photo (mirrors the Rust `FaceTile`). */
export type FaceTile = { id: number; photo_id: number; mtime: number; size: number; x: number; y: number; w: number; h: number };
/** "This looks like <person>", with how alike (0 to 1) (mirrors the Rust `Suggestion`). */
export type Suggestion = { person_id: number; name: string | null; score: number };
/** A person or a group for the People tab (mirrors the Rust `FacePerson`). A group is what the computer suggested; a
 *  `confirmed` person is one the user named, accepted or set apart. */
export type FacePerson = {
  id: number; name: string | null; hidden: boolean; confirmed: boolean; photos: number; faces: number;
  /** Faces that joined after the person was confirmed and have not been looked at. */
  unchecked: number;
  mixed: number | null; looks_mixed: boolean;
  /** Faces that show how the person looks (not just the best ones). */
  samples: number[];
  suggestion: Suggestion | null;
};
export type UnsortedFace = { tile: FaceTile; suggestion: Suggestion | null };
export type FaceOverview = { groups: number; people: number; unsorted: number; hidden: number; unchecked: number; rev: number };
/** Which cards the server lists; Unsorted is faces, not cards. */
export type FaceView = 'groups' | 'people' | 'hidden';
export type FaceOrder = 'best' | 'doubtful' | 'unchecked';
/** The sections of the People tab. */
export type Section = 'groups' | 'people' | 'hidden';

import { plural } from './format';
/** "12 photos". (A photo with the same person twice is rare, so faces are not counted separately.) */
export const faceCardLine = (p: { photos: number }): string => plural(p.photos, 'photo');

/** The rows the People list shows: the first `limit` (or all when expanded), plus the chosen person even if further down. */
export function personRows(people: PersonInfo[], chosen: number | null, expanded: boolean, limit: number): { rows: PersonInfo[]; hidden: number } {
  if (expanded || people.length <= limit) return { rows: people, hidden: 0 };
  let rows = people.slice(0, limit);
  const pick = chosen === null ? undefined : people.find((p) => p.id === chosen);
  if (pick && !rows.includes(pick)) rows = [...rows.slice(0, limit - 1), pick];
  return { rows, hidden: people.length - limit };
}

/** Which section to open the People tab on: Named, always, so it opens the same way every time. */
export const firstSection = (_o: FaceOverview): Section => 'people';

export const sectionTitle: Record<Section, string> = { groups: 'To name', people: 'Named', hidden: 'Ignored' };

/** The number on a section's pill: "To name" is the groups and the faces that fit no group. */
export const sectionCount = (o: FaceOverview, s: Section): number => (s === 'groups' ? o.groups + o.unsorted : o[s]);

/** What the computer thinks of a group's faces, in words. */
export function verdict(p: Pick<FacePerson, 'mixed' | 'looks_mixed'>): { text: string; mixed: boolean } | null {
  if (p.mixed === null) return null;
  return p.looks_mixed ? { text: 'Looks like several people', mixed: true } : { text: 'Looks like one person', mixed: false };
}

/** The people a suggestion names, ready for "This is…" (one with no name to show is left out). */
export const toCandidates = (s: Suggestion[]): { id: number; name: string }[] =>
  s.filter((x) => x.name !== null).map((x) => ({ id: x.person_id, name: x.name as string }));

/** "Looks like Gran", or null when the person it looks like has no name to show. */
export const suggestionLine = (s: Suggestion | null): string | null => (s?.name ? `Looks like ${s.name}` : null);

/** Who a group or face can be given to: named people (not hidden), not the one it already is. */
export const pickable = (people: Pick<FacePerson, 'id' | 'name' | 'hidden'>[], exceptId: number | null): { id: number; name: string }[] =>
  people
    .filter((p) => p.name !== null && !p.hidden && p.id !== exceptId)
    .map((p) => ({ id: p.id, name: p.name as string }))
    .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

/** Clicking a face in a grid: a plain click adds it to the picked faces or takes it out; shift+click picks everything
 *  between the last click and this one. `order` is the faces as shown. */
export function clickSelection(order: number[], picked: ReadonlySet<number>, anchor: number | null, id: number, shift: boolean): { picked: Set<number>; anchor: number } {
  const next = new Set(picked);
  const to = order.indexOf(id);
  const from = anchor === null ? -1 : order.indexOf(anchor);
  if (shift && from >= 0 && to >= 0) {
    for (let i = Math.min(from, to); i <= Math.max(from, to); i++) next.add(order[i]);
  } else if (next.has(id)) next.delete(id);
  else next.add(id);
  return { picked: next, anchor: id };
}

/** The list on screen is out of date because faces were regrouped since it was loaded (offered, never forced: cards must
 *  not move under the user). */
export const refreshOffered = (statusRev: number | undefined, loadedRev: number | null): boolean =>
  statusRev !== undefined && loadedRev !== null && statusRev > loadedRev;
