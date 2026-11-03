<script lang="ts">
  import type { FolderInfo } from './api';
  import { folderNote, type FolderStates } from './folderState';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { plural } from './format';
  import { NETWORK_MINUTES, type NetworkRescan } from './networkRescan';

  let { folders, folderStates = {}, scanning, scanFolder, onstop, onadd, onremove, master = null, networkRescan, onnetworkrescan }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void;
    /** Which folders cannot be reached right now (their drive is unplugged, say). */
    folderStates?: FolderStates;
    /** The folder the running scan is on. */
    scanFolder: number | null; onstop: () => void;
    onremove: (id: number) => Promise<void>;
    /** The sync folder, or null while there is none (chosen in Import/Export, Sync): its row says so. */
    master?: string | null;
    /** Whether, and how often, folders on the network are scanned again. */
    networkRescan: NetworkRescan; onnetworkrescan: (r: NetworkRescan) => void;
  } = $props();

  // The folder being asked about in the popup (to remove), if any.
  let ask = $state<{ kind: 'remove'; folder: FolderInfo } | null>(null);
  let error = $state('');

  async function confirmAsk() {
    const a = ask;
    ask = null;
    if (!a) return;
    error = '';
    try { await onremove(a.folder.id); } catch (e) { error = String(e); }
  }
</script>

<h2>Photo folders</h2>
<p class="hint">Pixie looks here for photos and videos. Removing a folder never deletes files; it only stops Pixie showing them.</p>
{#if folders.length === 0}
  <p class="none">No folders yet.</p>
{/if}
<ul>
  {#each folders as f (f.id)}
    {@const busy = scanning && scanFolder === f.id}
    {@const note = folderNote(folderStates[f.id])}
    <li class:offline={note !== null}>
      <span class="path" title={f.path}>{f.path}</span>
      {#if note}<span class="offnote" role="status">{note}</span>{/if}
      <span class="n">{plural(f.count, 'item')}</span>
      {#if master !== null && f.path === master}
        <span class="badge" title="Photos from your other folders are synced into this folder">Sync folder</span>
      {/if}
      {#if busy}
        <button onclick={onstop} title="Pause this scan; what was read so far is kept. Resume it from the Processing button at the top right">Pause</button>
      {/if}
      <button disabled={scanning} title={scanning ? 'Wait for the scan to finish' : ''} onclick={() => (ask = { kind: 'remove', folder: f })}>Remove</button>
    </li>
  {/each}
</ul>
{#if error}<p class="err">{error}</p>{/if}
<button onclick={onadd} title="Choose another folder of photos and videos to add to the library">Add folder…</button>

<h3>Keeping up to date</h3>
<p class="hint">Pixie automatically detects new or changed local files. How often should Pixie check network files?</p>
<label class="opt">
  <input type="checkbox" checked={networkRescan.on} onchange={(e) => onnetworkrescan({ ...networkRescan, on: e.currentTarget.checked })} />
  Check folders on the network
</label>
<label class="opt">
  Time between checks:
  <select value={networkRescan.minutes} disabled={!networkRescan.on}
          onchange={(e) => onnetworkrescan({ ...networkRescan, minutes: Number(e.currentTarget.value) })}>
    {#each NETWORK_MINUTES as m (m)}<option value={m}>{m < 60 ? `${m} minutes` : plural(m / 60, 'hour')}</option>{/each}
  </select>
</label>
{#if ask}
  <ConfirmDialog title="Remove this folder?" confirmLabel="Remove"
    message={`Pixie will stop showing the photos and videos in ${ask.folder.path}. Your files are not deleted.`}
    onconfirm={confirmAsk} oncancel={() => (ask = null)} />
{/if}

<style>
  h2 { margin: 0 0 8px; font-size: 18px; }
  .hint { color: #666; margin: 0 0 16px; max-width: 60ch; }
  ul { list-style: none; margin: 0 0 16px; padding: 0; display: grid; gap: 6px; }
  li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid #e4e4e8; border-radius: 6px; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #777; font-size: 12px; }
  li.offline .path, li.offline .n { opacity: .55; }
  .offnote { font-size: 12px; color: #8a5a00; background: #fff1d6; border-radius: 8px; padding: 1px 8px; white-space: nowrap; }
  .none { color: #777; }
  .badge { font-size: 12px; color: #1a3f9a; background: #e3ecff; border-radius: 8px; padding: 1px 8px; white-space: nowrap; }
  .err { color: #b00020; margin: 0 0 4px; }
</style>
