import { describe, expect, it } from 'vitest';
import { folderNote, toStates } from './folderState';

describe('folderNote', () => {
  it('names what is wrong, and says nothing for a folder that is fine or not yet known', () => {
    expect(folderNote('disconnected')).toBe('Not connected');
    expect(folderNote('missing')).toBe('Folder not found');
    expect(folderNote('ok')).toBeNull();
    expect(folderNote(undefined)).toBeNull();
  });
});

describe('toStates', () => {
  it('keys the answers by folder', () => {
    expect(toStates([{ id: 2, state: 'ok' }, { id: 5, state: 'disconnected' }])).toEqual({ 2: 'ok', 5: 'disconnected' });
    expect(toStates([])).toEqual({});
  });
});
