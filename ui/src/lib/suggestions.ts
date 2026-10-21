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

export const toggle = (rows: Row[], path: string): Row[] => rows.map((r) => (r.path === path ? { ...r, ticked: !r.ticked } : r));

/** The folders to add: ticked ones, except those already included in another suggestion. */
export const tickedPaths = (rows: Row[]): string[] => rows.filter((r) => r.ticked && r.included_in === null).map((r) => r.path);

/** The count shown beside a folder. */
export function countText(c: Row['count'], kind: Suggestion['kind']): string {
  if (c === null) return kind === 'further' ? '' : 'counting…';
  if (c.capped) return `${c.n.toLocaleString()}+ items`;
  return c.n === 0 ? 'no photos or videos' : `about ${c.n.toLocaleString()} items`;
}
