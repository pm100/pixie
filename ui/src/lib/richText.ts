/**
 * The little markup Ask Pixie's answers may use: numbered lines ("1. Pick the photos"), dashed lines ("- a point") and **bold** for the
 * names of buttons, tabs and settings. Nothing else is read as markup, and nothing is ever put in as HTML: the pieces are shown as text.
 */
export interface Span { text: string; bold: boolean }
export type Block =
  | { kind: 'p'; spans: Span[] }
  | { kind: 'steps'; items: Span[][] }
  | { kind: 'points'; items: Span[][] };

/** A line cut into plain and bold pieces; a ** with nothing to close it is dropped and the words after it are plain. */
export function spans(line: string): Span[] {
  const parts = line.split('**');
  // With an odd number of markers the last one is unclosed, so the last piece is plain.
  const closedUpTo = parts.length % 2 === 0 ? parts.length - 1 : parts.length;
  const out: Span[] = [];
  parts.forEach((text, i) => {
    if (text === '') return;
    const bold = i % 2 === 1 && i < closedUpTo;
    const last = out[out.length - 1];
    if (last && last.bold === bold) last.text += text;
    else out.push({ text, bold });
  });
  return out;
}

const STEP = /^\s*\d{1,2}[.)]\s+(.*)$/;
const POINT = /^\s*[-•]\s+(.*)$/;

/**
 * Models sometimes run the steps together on one line ("To email: 1. Pick it. 2. Open Send."). A line that holds the numbers 1, 2, 3...
 * in order is cut before each of them, so it is laid out as a list like the rest. Any other numbers in a line are left alone.
 */
export function splitInlineSteps(line: string): string[] {
  const marks = [...line.matchAll(/(?:^|\s)(\d{1,2})[.)]\s+(?=\S)/g)];
  const counting: RegExpMatchArray[] = [];
  for (const m of marks) if (Number(m[1]) === counting.length + 1) counting.push(m);
  if (counting.length < 2) return [line];
  const cuts = counting.map((m) => m.index! + (m[0].length - m[0].trimStart().length));
  const pieces = [line.slice(0, cuts[0]), ...cuts.map((c, i) => line.slice(c, cuts[i + 1]))];
  return pieces.map((x) => x.trim()).filter((x) => x !== '');
}

export function parseRich(text: string): Block[] {
  const blocks: Block[] = [];
  for (const raw of text.split('\n').flatMap(splitInlineSteps)) {
    if (raw.trim() === '') continue;
    const step = STEP.exec(raw), point = POINT.exec(raw);
    const kind = step ? 'steps' : point ? 'points' : null;
    if (kind) {
      const item = spans((step ?? point)![1]);
      const last = blocks[blocks.length - 1];
      if (last && last.kind === kind) last.items.push(item);
      else blocks.push({ kind, items: [item] });
    } else {
      blocks.push({ kind: 'p', spans: spans(raw.trim()) });
    }
  }
  return blocks;
}
