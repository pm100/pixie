/** The pages the Print tool lays out. Sizes are in millimetres. */
export type Paper = 'a4' | 'letter' | '4x6' | '5x7';
export const PAPERS: Record<Paper, { label: string; w: number; h: number }> = {
  a4: { label: 'A4', w: 210, h: 297 },
  letter: { label: 'Letter', w: 215.9, h: 279.4 },
  '4x6': { label: '4 × 6 in', w: 101.6, h: 152.4 },
  '5x7': { label: '5 × 7 in', w: 127, h: 177.8 },
};

export const PER_PAGE = [1, 2, 4, 6] as const;
export type PerPage = (typeof PER_PAGE)[number];

export function pageSize(paper: Paper, landscape: boolean): { w: number; h: number } {
  const { w, h } = PAPERS[paper];
  return landscape ? { w: h, h: w } : { w, h };
}

/** The columns and rows photos are laid out in. */
export function grid(perPage: PerPage, landscape = false): { cols: number; rows: number } {
  const g = { 1: { cols: 1, rows: 1 }, 2: { cols: 1, rows: 2 }, 4: { cols: 2, rows: 2 }, 6: { cols: 2, rows: 3 } }[perPage];
  return landscape ? { cols: g.rows, rows: g.cols } : g;
}

/** The photos split into pages, in order. */
export function sheets(ids: number[], perPage: number): number[][] {
  const pages: number[][] = [];
  for (let i = 0; i < ids.length; i += perPage) pages.push(ids.slice(i, i + perPage));
  return pages;
}
