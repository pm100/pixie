/** Whether a watched folder can be reached right now: its drive may be unplugged or a share disconnected. */
export type FolderStateName = 'ok' | 'disconnected' | 'missing';
export type FolderStates = Record<number, FolderStateName>;

/** The short note shown beside a folder that cannot be reached, or null when it is fine (or not yet known). */
export function folderNote(state: FolderStateName | undefined): string | null {
  if (state === 'disconnected') return 'Not connected';
  if (state === 'missing') return 'Folder not found';
  return null;
}

export function toStates(list: { id: number; state: FolderStateName }[]): FolderStates {
  return Object.fromEntries(list.map((s) => [s.id, s.state]));
}
