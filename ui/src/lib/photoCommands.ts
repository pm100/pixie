import { registerCommand, type Command } from './commands';

/** What was right-clicked in Browse: one photo, or (when it is one of the Picked) all the Picked photos. */
export interface PhotoTarget {
  ids: number[];
  /** The position in the listing: set for one photo only. */
  index: number | null;
  /** 'video' only for one video. */
  kind: 'photo' | 'video' | 'mixed';
  /** Every one of them is Picked. */
  picked: boolean;
  /** One photo: whether it is private, or null when there is no sync folder (private means nothing then) or it is several photos. */
  private: boolean | null;
  /** Every one of them is hidden. */
  hidden: boolean;
}

export interface PhotoCommandContext {
  /** The system can put photos on the clipboard (Windows now; a Mac later). */
  canCopy: boolean;
  open: (index: number) => void;
  setPicked: (ids: number[], on: boolean) => void;
  openPlayer: (id: number) => void;
  showInFolder: (id: number) => void;
  /** Open a tool on the photo right-clicked ('current') or on the Picked photos ('picked'). */
  openTool: (toolId: string, source: 'current' | 'picked') => void;
  setPrivate: (id: number, on: boolean) => void;
  setHidden: (ids: number[], on: boolean) => void;
  copy: (ids: number[], form: 'photo' | 'image' | 'paths') => void;
}

const one = (t: PhotoTarget) => t.ids.length === 1;

/** The right-click menu of a photo in Browse: group 1 open and pick, group 2 the tools that take the photos, group 3 copying, the file and its visibility. */
export function registerPhotoCommands(ctx: PhotoCommandContext): () => void {
  const photo = (group: number) => [{ context: 'photo', group }];
  const tool = (id: string, label: string, toolId: string): Command<PhotoTarget> => ({
    id, label, where: photo(2), run: (t) => ctx.openTool(toolId, one(t) ? 'current' : 'picked'),
  });
  const cmds: Command<PhotoTarget>[] = [
    { id: 'photo.open', label: 'Open', where: photo(1), visible: (t) => one(t) && t.index !== null, run: (t) => ctx.open(t.index!) },
    { id: 'photo.openPlayer', label: 'Open in the default player', where: photo(1), visible: (t) => one(t) && t.kind === 'video', run: (t) => ctx.openPlayer(t.ids[0]) },
    { id: 'photo.pick', label: (t) => (t.picked ? 'Take off Picked' : 'Pick'), where: photo(1), run: (t) => ctx.setPicked(t.ids, !t.picked) },
    tool('photo.email', 'Email…', 'email'),
    tool('photo.print', 'Print…', 'print'),
    { id: 'photo.copy', label: 'Copy', where: photo(3), visible: () => ctx.canCopy, run: (t) => ctx.copy(t.ids, 'photo') },
    { id: 'photo.copyImage', label: 'Copy image', where: photo(3), visible: (t) => ctx.canCopy && one(t) && t.kind === 'photo', run: (t) => ctx.copy(t.ids, 'image') },
    { id: 'photo.copyPath', label: 'Copy path', where: photo(3), visible: () => ctx.canCopy, run: (t) => ctx.copy(t.ids, 'paths') },
    { id: 'photo.showInFolder', label: 'Show in folder', where: photo(3), visible: one, run: (t) => ctx.showInFolder(t.ids[0]) },
    { id: 'photo.private', label: (t) => (t.private ? 'Make public' : 'Make private'), where: photo(3), visible: (t) => one(t) && t.private !== null, run: (t) => ctx.setPrivate(t.ids[0], !t.private) },
    { id: 'photo.hide', label: (t) => (t.hidden ? 'Unhide' : 'Hide'), where: photo(3), run: (t) => ctx.setHidden(t.ids, !t.hidden) },
  ];
  const offs = cmds.map((c) => registerCommand(c));
  return () => offs.forEach((off) => off());
}
