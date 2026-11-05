import { describe, expect, it } from 'vitest';
import { firstTool, GROUPS, isToolGroup, toolById, TOOLS, toolsIn, worksOnWholeLibrary } from './tools';

describe('tools', () => {
  it('lists each tab\'s tools in order', () => {
    expect(toolsIn('send').map((t) => t.id)).toEqual(['email', 'print', 'share', 'clipboard', 'open-with', 'slideshow']);
    expect(toolsIn('create').map((t) => t.id)).toEqual(['collage', 'mosaic', 'gif', 'edit']);
    expect(toolsIn('organize').map((t) => t.id)).toEqual(['tags', 'dates', 'hide', 'move']);
    expect(toolsIn('cleanup').map((t) => t.id)).toEqual(['duplicates', 'missing-dates', 'bursts', 'thumbnails']);
    expect(toolsIn('transfer').map((t) => t.id)).toEqual(['sync']);
  });

  it('has the Collage built, as the first tool of Create', () => {
    expect(toolById('collage')?.built).toBe(true);
    expect(firstTool('create').id).toBe('collage');
  });

  it('always has Duplicates in Clean up, which opens on it (the duplicate check always runs)', () => {
    expect(toolsIn('cleanup').map((t) => t.id)[0]).toBe('duplicates');
    expect(firstTool('cleanup').id).toBe('duplicates');
  });

  it('opens each tab at its first built tool, or at its first tool when none is built', () => {
    expect(firstTool('send').id).toBe('email');
    expect(firstTool('organize').id).toBe('tags');
    expect(firstTool('create').id).toBe('collage');
  });

  it('gives every unbuilt tool something to show as a placeholder', () => {
    for (const t of TOOLS.filter((x) => !x.built)) {
      expect(t.blurb.length, t.id).toBeGreaterThan(0);
      expect(t.mock?.length, t.id).toBeGreaterThan(0);
      expect(t.action, t.id).toBeTruthy();
    }
  });

  it('has unique ids, each in a known tab', () => {
    expect(new Set(TOOLS.map((t) => t.id)).size).toBe(TOOLS.length);
    for (const t of TOOLS) expect(GROUPS.some((g) => g.id === t.group)).toBe(true);
  });

  it('knows which tab works on the whole library', () => {
    expect(worksOnWholeLibrary('cleanup')).toBe(true);
    expect(worksOnWholeLibrary('transfer')).toBe(true);
    expect(['send', 'create', 'organize'].some((g) => worksOnWholeLibrary(g as never))).toBe(false);
  });

  it('finds a tool by id and recognises a tab id', () => {
    expect(toolById('gif')?.label).toBe('Animated GIF');
    expect(toolById('nope')).toBeNull();
    expect(isToolGroup('send')).toBe(true);
    expect(isToolGroup('browse')).toBe(false);
  });
});
