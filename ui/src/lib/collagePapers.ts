/** The page sizes a collage can be made for: what print shops sell, with the sizes of the person's region first. Millimetres, portrait. */
export interface PaperSize { id: string; label: string; w: number; h: number }
export interface PaperGroup { label: string; sizes: PaperSize[] }

const IN = 25.4;
const inches = (w: number, h: number): PaperSize => ({ id: `${w}x${h}in`, label: `${w} × ${h} in`, w: w * IN, h: h * IN });
const cm = (w: number, h: number, label = `${w} × ${h} cm`): PaperSize => ({ id: `${w}x${h}cm`, label, w: w * 10, h: h * 10 });

const GROUPS: Record<'inches' | 'cm' | 'posters' | 'square', PaperGroup> = {
  inches: {
    label: 'Prints (inches)',
    sizes: [inches(4, 6), inches(5, 7), inches(8, 10), inches(8, 12), inches(11, 14), inches(12, 18), inches(16, 20), inches(16, 24), inches(20, 24), inches(20, 30), inches(24, 36), inches(30, 40),
      { id: 'letter', label: 'Letter', w: 215.9, h: 279.4 }],
  },
  cm: { label: 'Prints (cm)', sizes: [cm(10, 15), cm(13, 18), cm(15, 20), cm(20, 30), cm(30, 40), cm(40, 50), cm(50, 70), cm(60, 90), cm(70, 100)] },
  posters: {
    label: 'Posters and frames',
    sizes: [{ id: 'a4', label: 'A4', w: 210, h: 297 }, { id: 'a3', label: 'A3', w: 297, h: 420 }, { id: 'a2', label: 'A2', w: 420, h: 594 }, { id: 'a1', label: 'A1', w: 594, h: 841 }, { id: 'a0', label: 'A0', w: 841, h: 1189 },
      cm(61, 91, '61 × 91 cm (frame)')],
  },
  square: { label: 'Square (books, calendars)', sizes: [inches(8, 8), inches(12, 12), cm(20, 20), cm(30, 30)] },
};

/** Countries that print in inches. */
const INCH_REGIONS = new Set(['US', 'CA', 'MX', 'PH']);

/** True when the language names one of those countries (for example `en-US`). A language with no country, such as plain `en`, is metric: a
 *  country is not guessed from it. */
export function usesInches(locale: string | undefined): boolean {
  if (!locale) return false;
  let region: string | undefined;
  try {
    region = new Intl.Locale(locale).region?.toUpperCase();
  } catch {
    region = /[-_]([A-Za-z]{2})\b/.exec(locale)?.[1]?.toUpperCase();
  }
  return region !== undefined && INCH_REGIONS.has(region);
}

/** The groups for the paper list, the region's prints first. */
export function paperGroups(locale?: string): PaperGroup[] {
  const { inches: i, cm: c, posters, square } = GROUPS;
  return usesInches(locale) ? [i, c, posters, square] : [c, i, posters, square];
}

export const defaultPaperId = (locale?: string): string => (usesInches(locale) ? '8x10in' : '20x30cm');

const ALL: PaperSize[] = Object.values(GROUPS).flatMap((g) => g.sizes);
export const findPaper = (id: string): PaperSize | undefined => ALL.find((p) => p.id === id);
