<script lang="ts">
  import type { FolderInfo } from './api';
  import { folderNote, type FolderStates } from './folderState';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { plural } from './format';
  import { NETWORK_MINUTES, type NetworkRescan } from './networkRescan';
  import { scanText, type ScanState } from './scanProgress';

  let { folders, folderStates = {}, analysing = null, scanning, scanFolder, scan, onstop, onadd, onremove, master = null, onmaster = () => {}, onfeeds = () => {}, networkRescan, onnetworkrescan }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void;
    /** Which folders cannot be reached right now (their drive is unplugged, say). */
    folderStates?: FolderStates;
    /** The folders just added and how far their analysis has got: those rows say so. */
    analysing?: { ids: number[]; fraction: number | null } | null;
    /** The folder the running scan is on, and what it has reported so far. */
    scanFolder: number | null; scan: ScanState; onstop: () => void;
    onremove: (id: number) => Promise<void>;
    /** The master folder, or null while there is none: then nothing of the master feature is shown but the choice to set one. */
    master?: string | null; onmaster?: (path: string | null) => void;
    /** Whether new items found in a folder wait for review before they are copied into the master. */
    onfeeds?: (id: number, feeds: boolean) => void;
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
      {#if busy}
        <span class="n" role="status">{scanText(scan)}</span>
      {:else}
        <span class="n">{plural(f.count, 'item')}{#if analysing && analysing.fraction !== null && analysing.ids.includes(f.id)} · {Math.floor(analysing.fraction * 100)}% analysed{/if}</span>
      {/if}
      {#if master !== null && f.path === master}
        <span class="badge" title="Approved photos are copied into this folder">Master folder</span>
      {:else if master !== null}
        <label class="feeds" title="New photos found in this folder wait for your approval before they are copied into the master folder">
          <input type="checkbox" checked={f.feeds_master} onchange={(e) => onfeeds(f.id, e.currentTarget.checked)} /> Feeds the master
        </label>
      {/if}
      {#if busy}
        <button onclick={onstop} title="Stop this scan; what was read so far is kept">Stop</button>
      {/if}
      <button disabled={scanning} title={scanning ? 'Wait for the scan to finish' : ''} onclick={() => (ask = { kind: 'remove', folder: f })}>Remove</button>
    </li>
  {/each}
</ul>
{#if error}<p class="err">{error}</p>{/if}
<button onclick={onadd} title="Choose another folder of photos and videos to add to the library">Add folder…</button>

<h3>Master folder</h3>
<p class="hint">A folder that holds the photos you share, such as a family drive. Pixie copies new photos into it once you have approved them; nothing is copied before that. Until you choose one, none of this is used.</p>
<label class="opt">
  Master folder:
  <select value={master ?? ''} disabled={folders.length === 0} title="Choose which of your folders holds the photos you share, or None to go without one. No files are moved or deleted"
          aria-label="Master folder" onchange={(e) => onmaster(e.currentTarget.value === '' ? null : e.currentTarget.value)}>
    <option value="">None</option>
    {#each folders as f (f.id)}<option value={f.path}>{f.path}</option>{/each}
  </select>
</label>

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
  .feeds { font-size: 12px; color: #555; white-space: nowrap; display: inline-flex; align-items: center; gap: 4px; }
  .err { color: #b00020; }
</style>
