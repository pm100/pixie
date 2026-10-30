import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(__dirname, '..');
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? files(join(dir, e.name)) : e.name.endsWith('.svelte') ? [join(dir, e.name)] : []));

const named = /\b(title|aria-label|aria-labelledby)\s*=/;
const tag = /<(button|select|textarea|input)\b((?:[^>"'{]|"[^"]*"|'[^']*'|\{(?:[^{}]|\{[^{}]*\})*\})*)>(?:([\s\S]*?)<\/button>)?/g;

/** Controls a person cannot tell the purpose of: a button with no words in it (a symbol, an icon), and a field, select or
 *  text area with neither a label around it, a `for` label, nor a name of its own. They need a title or an aria-label. */
function unnamed(src: string): string[] {
  const out: string[] = [];
  for (const m of src.matchAll(tag)) {
    const [, kind, attrs, inner] = m;
    if (named.test(attrs)) continue;
    const line = src.slice(0, m.index).split('\n').length;
    if (kind === 'button') {
      if (!/[A-Za-z{]/.test((inner ?? '').replace(/<[^>]*>/g, ''))) out.push(`line ${line}: <button> with no words`);
    } else if (!/type="(checkbox|radio|hidden)"/.test(attrs) && !/\bid=/.test(attrs)) {
      const before = src.slice(0, m.index);
      if (before.lastIndexOf('<label') <= before.lastIndexOf('</label>')) out.push(`line ${line}: <${kind}> with no label`);
    }
  }
  return out;
}

describe('controls have a name', () => {
  it('spots a symbol-only button and a bare field', () => {
    expect(unnamed('<button onclick={x}>✎</button>')).toHaveLength(1);
    expect(unnamed('<input type="text" bind:value={v} />')).toHaveLength(1);
    expect(unnamed('<button title="Rename" onclick={x}>✎</button><label>Name <input /></label>')).toEqual([]);
  });

  for (const f of files(SRC)) {
    it(f.slice(SRC.length + 1), () => expect(unnamed(readFileSync(f, 'utf8'))).toEqual([]));
  }
});
