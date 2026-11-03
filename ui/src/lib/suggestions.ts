import { plural } from './format';
import type { Suggestion } from './api';

/** A suggested folder on the welcome screen: what the app found, whether it is ticked, and its count once known. */
export type Row = Suggestion & { count: { n: number; capped: boolean } | null };

/** Case-insensitive, either slash, no trailing slash: how paths are compared here. */
const norm = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
const inside = (path: string, folder: string) => norm(path).startsWith(norm(folder) + '/');

export const toRows = (list: Suggestion[]): Row[] => list.map((s) => ({ ...s, count: null }));

/** A count arrived for one folder. */
export const applyCount = (rows: Row[], c: { path: string; n: number; capped: boolean }): Row[] =>
  rows.map((r) => (norm(r.path) === norm(c.path) ? { ...r, count: { n: c.n, capped: c.capped } } : r));

/** A further candidate was found: add it unless the folder is already listed, lies inside a listed one, or holds one. */
export function addFound(rows: Row[], s: Suggestion): Row[] {
  const clash = rows.some((r) => norm(r.path) === norm(s.path) || inside(s.path, r.path) || inside(r.path, s.path));
  return clash ? rows : [...rows, { ...s, count: null }];
}

/** A folder the person chose by hand always ends up listed and ticked, even when it is empty. Rows that overlap it (the same
 *  folder, one holding it, one inside it) give way to it. If the library already has it (or a folder holding it), nothing
 *  changes and `note` says so. */
export function chooseByHand(rows: Row[], library: string[], s: Suggestion): { rows: Row[]; note: string | null } {
  const held = library.find((f) => norm(f) === norm(s.path) || inside(s.path, f));
  if (held !== undefined) return { rows, note: norm(held) === norm(s.path) ? `${s.path} is already in your library.` : `${s.path} is already in your library, inside ${held}.` };
  const overlaps = (p: string) => norm(p) === norm(s.path) || inside(s.path, p) || inside(p, s.path);
  return { rows: [...rows.filter((r) => !overlaps(r.path)), { ...s, count: null }], note: null };
}

export const toggle = (rows: Row[], path: string): Row[] => rows.map((r) => (r.path === path ? { ...r, ticked: !r.ticked } : r));

/** The rows worth showing: not inside another suggestion, and not known to be empty (a count of 0; a folder still being
 *  counted, or only counted up to a cap, stays). Order is kept. */
export const listedRows = (rows: Row[]): Row[] =>
  rows.filter((r) => r.included_in === null && !(r.count !== null && r.count.n === 0 && !r.count.capped));

/** Leave out what the library already has: a folder it holds, one inside a folder it holds, or one that holds one of its
 *  folders (adding either would list the same photos twice). */
export const notInLibrary = (rows: Row[], library: string[]): Row[] =>
  rows.filter((r) => !library.some((f) => norm(f) === norm(r.path) || inside(r.path, f) || inside(f, r.path)));

/** The folders to add: the ticked ones among the rows shown. */
export const tickedPaths = (rows: Row[]): string[] => listedRows(rows).filter((r) => r.ticked).map((r) => r.path);

/** The line under the spinner: what is being sniffed now (`suggestion-looking`); nothing before the first answer. */
export const lookingText = (what: string | null): string => (what ? `Looking in ${what}…` : '');

/** The count shown beside a folder. */
export function countText(c: Row['count'], kind: Suggestion['kind']): string {
  if (c === null) return kind === 'further' ? '' : 'counting…';
  if (c.capped) return `${c.n.toLocaleString()}+ items`; // (a lower bound: always more than one)
  return c.n === 0 ? 'no photos or videos' : `about ${plural(c.n, 'item')}`;
}
