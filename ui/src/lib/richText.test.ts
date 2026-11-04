import { describe, expect, it } from 'vitest';
import { parseRich, spans, splitInlineSteps } from './richText';

describe('steps run together on one line', () => {
  it('are cut into lines, with the lead-in kept', () => {
    expect(splitInlineSteps('To email a video: 1. Choose it. 2. Open **Send**. 3. Press Email.')).toEqual(['To email a video:', '1. Choose it.', '2. Open **Send**.', '3. Press Email.']);
    expect(parseRich('To email: 1. Pick. 2. Open.').map((b) => b.kind)).toEqual(['p', 'steps']);
  });
  it('leave other numbers alone', () => {
    for (const line of ['Set photos per page (1, 2, 4 or 6).', 'It took 2. times', 'Version 2. Done', '1. only one step', 'Step 2. Then 3. Then 4.']) expect(splitInlineSteps(line), line).toEqual([line]);
  });
  it('need the numbers to count up from 1', () => {
    expect(splitInlineSteps('a 2. b 3. c')).toEqual(['a 2. b 3. c']);
    expect(splitInlineSteps('1. a 1. b')).toEqual(['1. a 1. b']);
  });
});

describe('spans', () => {
  it('cuts out the bold names', () => {
    expect(spans('Open **Send**, then **Email**.')).toEqual([{ text: 'Open ', bold: false }, { text: 'Send', bold: true }, { text: ', then ', bold: false }, { text: 'Email', bold: true }, { text: '.', bold: false }]);
  });
  it('shows an unclosed pair of asterisks as plain text, and never loses words', () => {
    expect(spans('Open **Send')).toEqual([{ text: 'Open Send', bold: false }]);
    expect(spans('a **b** c **d').map((s) => s.text).join('')).toBe('a b c d');
    expect(spans('')).toEqual([]);
  });
});

describe('parseRich', () => {
  it('lays out a sentence, numbered steps and a closing line', () => {
    const blocks = parseRich('To email:\n1. Pick the **photos**.\n2) Open **Send**.\n\nVideos go as they are.');
    expect(blocks.map((b) => b.kind)).toEqual(['p', 'steps', 'p']);
    expect(blocks[1].kind === 'steps' && blocks[1].items).toHaveLength(2);
  });
  it('keeps dashed lines together as points', () => {
    const blocks = parseRich('- one\n- two\nthen text');
    expect(blocks.map((b) => b.kind)).toEqual(['points', 'p']);
  });
  it('treats a plain answer as paragraphs, line by line', () => {
    expect(parseRich('One.\nTwo.').map((b) => b.kind)).toEqual(['p', 'p']);
    expect(parseRich('')).toEqual([]);
  });
  it('reads nothing else as markup: no headings, links or HTML', () => {
    const [b] = parseRich('# Not a heading [x](http://e) <b>y</b>');
    expect(b.kind).toBe('p');
    expect(b.kind === 'p' && b.spans.map((s) => s.text).join('')).toBe('# Not a heading [x](http://e) <b>y</b>');
  });
});
