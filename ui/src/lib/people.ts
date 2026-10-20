import type { AnalysisStatus } from './analysis';

/** A person in the nav: `count` photos, `face_id` the face to show as their picture (mirrors the Rust `PersonInfo`). */
export type PersonInfo = { id: number; name: string | null; count: number; face_id: number | null };
/** A face in one photo; the box is a fraction (0..1) of the upright image (mirrors the Rust `FaceInfo`). */
export type FaceInfo = { id: number; person_id: number | null; person_name: string | null; x: number; y: number; w: number; h: number };
/** The face scan reports progress the same way the search analysis does. */
export type PeopleStatus = AnalysisStatus;

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
  if (s.error) return `People search is having trouble: ${s.error}`;
  if (s.total === 0) return 'No photos to check for faces yet';
  if (s.paused && s.done < s.total) return `Paused at ${n(s.done)} of ${n(s.total)}`;
  if (s.done >= s.total) return `Faces checked in all ${n(s.total)} photos`;
  return `Looking for people… ${n(s.done)} of ${n(s.total)} photos`;
}

/** A person for the Faces tab (mirrors the Rust `FacePerson`). `listed`: also in the nav, so Browse can filter on them. */
export type FacePerson = { id: number; name: string | null; hidden: boolean; listed: boolean; photos: number; faces: number; samples: number[] };
/** One face to look at: the photo it is in (with that photo's version, to show it) and the face's box as fractions
 *  (0..1) of the upright photo (mirrors the Rust `FaceTile`). */
export type FaceTile = { id: number; photo_id: number; mtime: number; size: number; x: number; y: number; w: number; h: number };

const plural = (count: number, one: string) => `${n(count)} ${one}${count === 1 ? '' : 's'}`;
/** "12 photos · 15 faces". */
export const faceCardLine = (p: { photos: number; faces: number }): string => `${plural(p.photos, 'photo')} · ${plural(p.faces, 'face')}`;

/** Another person already called this (ignoring case and surrounding spaces), if any: naming someone that offers a merge. */
export function sameNamePerson<T extends { id: number; name: string | null }>(people: T[], name: string, exceptId: number): T | null {
  const wanted = name.trim().toLowerCase();
  if (wanted === '') return null;
  return people.find((p) => p.id !== exceptId && p.name?.trim().toLowerCase() === wanted) ?? null;
}

/** What typing `name` for a face means: nothing (blank, or the name it already has), move it to the person with that
 *  name, or start a new person. Moving one face is easy to undo, so it needs no confirmation. */
export function resolveFaceName(
  people: { id: number; name: string | null }[],
  name: string,
  currentPersonId: number | null,
): { kind: 'none' } | { kind: 'move'; personId: number } | { kind: 'new'; name: string } {
  const text = name.trim();
  if (text === '') return { kind: 'none' };
  const current = currentPersonId === null ? null : people.find((p) => p.id === currentPersonId);
  if (current?.name?.trim().toLowerCase() === text.toLowerCase()) return { kind: 'none' };
  const same = sameNamePerson(people, text, currentPersonId ?? -1);
  return same ? { kind: 'move', personId: same.id } : { kind: 'new', name: text };
}

/** The rows the People list shows: the first `limit` (or all when expanded), plus the chosen person even if further down. */
export function personRows(people: PersonInfo[], chosen: number | null, expanded: boolean, limit: number): { rows: PersonInfo[]; hidden: number } {
  if (expanded || people.length <= limit) return { rows: people, hidden: 0 };
  let rows = people.slice(0, limit);
  const pick = chosen === null ? undefined : people.find((p) => p.id === chosen);
  if (pick && !rows.includes(pick)) rows = [...rows.slice(0, limit - 1), pick];
  return { rows, hidden: people.length - limit };
}
