import { beforeEach, describe, expect, it, vi } from 'vitest';
import { allCommands, contextModel, resetCommands } from './commands';
import { registerPhotoCommands, type PhotoCommandContext, type PhotoTarget } from './photoCommands';

const target = (over: Partial<PhotoTarget> = {}): PhotoTarget => ({ ids: [7], index: 3, kind: 'photo', picked: false, private: null, hidden: false, ...over });
function fake(): PhotoCommandContext {
  return { open: vi.fn(), setPicked: vi.fn(), openPlayer: vi.fn(), showInFolder: vi.fn(), openTool: vi.fn(), setPrivate: vi.fn(), setHidden: vi.fn(), copy: vi.fn() };
}
const labels = (t: PhotoTarget) => contextModel(allCommands(), 'photo', t).map((g) => g.map((i) => i.label));
const find = (id: string) => allCommands().find((c) => c.id === id)!;

describe('photoCommands', () => {
  beforeEach(() => resetCommands());

  it('offers one photo its whole menu, in groups', () => {
    registerPhotoCommands(fake());
    expect(labels(target())).toEqual([
      ['Open', 'Pick'],
      ['Email…', 'Print…'],
      ['Copy', 'Copy image', 'Copy path', 'Show in folder', 'Hide'],
    ]);
  });

  it('words Pick and Hide by the state, and offers Make private only when a sync folder is set', () => {
    registerPhotoCommands(fake());
    expect(labels(target({ picked: true, hidden: true }))).toEqual([
      ['Open', 'Take off Picked'],
      ['Email…', 'Print…'],
      ['Copy', 'Copy image', 'Copy path', 'Show in folder', 'Unhide'],
    ]);
    expect(labels(target({ private: false }))[2]).toContain('Make private');
    expect(labels(target({ private: true }))[2]).toContain('Make public');
    expect(labels(target({ private: null }))[2]).not.toContain('Make private');
  });

  it('offers the default player only for one video', () => {
    registerPhotoCommands(fake());
    expect(labels(target({ kind: 'video' }))[0]).toContain('Open in the default player');
    expect(labels(target({ kind: 'photo' }))[0]).not.toContain('Open in the default player');
  });

  it('offers several photos only what works on all of them', () => {
    registerPhotoCommands(fake());
    expect(labels(target({ ids: [1, 2, 3], index: null, picked: true, private: null }))).toEqual([
      ['Take off Picked'],
      ['Email…', 'Print…'],
      ['Copy', 'Copy path', 'Hide'],
    ]);
  });

  it('hands the right photos to the tools: the current photo for one, Picked for several', () => {
    const ctx = fake();
    registerPhotoCommands(ctx);
    find('photo.email').run(target());
    expect(ctx.openTool).toHaveBeenCalledWith('email', 'current');
    find('photo.print').run(target({ ids: [1, 2], index: null, picked: true }));
    expect(ctx.openTool).toHaveBeenCalledWith('print', 'picked');
    find('photo.print').run(target());
    expect(ctx.openTool).toHaveBeenCalledWith('print', 'current');
  });

  it('runs the single-photo and the group actions with the target', () => {
    const ctx = fake();
    registerPhotoCommands(ctx);
    const t = target();
    find('photo.open').run(t);
    expect(ctx.open).toHaveBeenCalledWith(3);
    find('photo.pick').run(t);
    expect(ctx.setPicked).toHaveBeenCalledWith([7], true);
    find('photo.pick').run(target({ picked: true }));
    expect(ctx.setPicked).toHaveBeenCalledWith([7], false);
    find('photo.showInFolder').run(t);
    expect(ctx.showInFolder).toHaveBeenCalledWith(7);
    find('photo.hide').run(target({ hidden: true }));
    expect(ctx.setHidden).toHaveBeenCalledWith([7], false);
    find('photo.private').run(target({ private: false }));
    expect(ctx.setPrivate).toHaveBeenCalledWith(7, true);
  });

  it('copies the files, the picture (one photo only) and the paths of what was clicked', () => {
    const ctx = fake();
    registerPhotoCommands(ctx);
    find('photo.copy').run(target({ ids: [1, 2], index: null }));
    expect(ctx.copy).toHaveBeenCalledWith([1, 2], 'photo');
    find('photo.copyPath').run(target({ ids: [1, 2], index: null }));
    expect(ctx.copy).toHaveBeenCalledWith([1, 2], 'paths');
    find('photo.copyImage').run(target());
    expect(ctx.copy).toHaveBeenCalledWith([7], 'image');
  });

  it('offers a video the file and the path but no picture copy', () => {
    registerPhotoCommands(fake());
    expect(labels(target({ kind: 'video' })).flat()).not.toContain('Copy image');
    expect(labels(target({ kind: 'video' })).flat()).toContain('Copy');
    expect(labels(target({ kind: 'video' })).flat()).toContain('Copy path');
  });

  it('removes its commands when unregistered', () => {
    const off = registerPhotoCommands(fake());
    off();
    expect(allCommands()).toHaveLength(0);
  });
});
