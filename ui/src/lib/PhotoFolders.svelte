<script lang="ts">
  import type { FolderInfo } from './api';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { readFraction, scanText, type ScanState } from './scanProgress';

  let { folders, scanning, scanFolder, scan, onstop, onadd, onremove, onrescan }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void;
    /** The folder the running scan is on, and what it has reported so far. */
    scanFolder: number | null; scan: ScanState; onstop: () => void;
    onremove: (id: number) => Promise<void>; onrescan: (id: number, full: boolean) => void;
  } = $props();

  // The question being asked in the popup, if any.
  let ask = $state<{ kind: 'reload' | 'remove'; folder: FolderInfo } | null>(null);
  let error = $state('');

  async function confirmAsk() {
    const a = ask;
    ask = null;
    if (!a) return;
    if (a.kind === 'reload') { onrescan(a.folder.id, true); return; }
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
    <li>
      <span class="path" title={f.path}>{f.path}</span>
      {#if busy}
        {@const frac = readFraction(scan)}
        <span class="prog" role="status">
          <span class="txt">{scanText(scan)}</span>
          <progress value={frac ?? undefined} max="1" aria-label="Scan progress"></progress>
        </span>
      {:else}
        <span class="n">{f.count.toLocaleString()} items</span>
      {/if}
      {#if busy}
        <button onclick={onstop} title="Stop this scan; what was read so far is kept">Stop</button>
      {:else}
        <button disabled={scanning} title="Look for new and changed files only" onclick={() => onrescan(f.id, false)}>Rescan</button>
        <button disabled={scanning} title="Read every file in this folder again, not just new and changed ones" onclick={() => (ask = { kind: 'reload', folder: f })}>Reload</button>
      {/if}
      <button disabled={scanning} title={scanning ? 'Wait for the scan to finish' : ''} onclick={() => (ask = { kind: 'remove', folder: f })}>Remove</button>
    </li>
  {/each}
</ul>
{#if error}<p class="err">{error}</p>{/if}
<button class="add" onclick={onadd}>Add folder…</button>
{#if ask}
  {#if ask.kind === 'reload'}
    <ConfirmDialog title="Reload this folder?" confirmLabel="Reload"
      message={`Pixie will read all ${ask.folder.count.toLocaleString()} items in ${ask.folder.path} again. This can take a long time for a large folder or a network drive. You can keep using Pixie while it runs, and stop it at any time.`}
      onconfirm={confirmAsk} oncancel={() => (ask = null)} />
  {:else}
    <ConfirmDialog title="Remove this folder?" confirmLabel="Remove" danger
      message={`Pixie will stop showing the photos and videos in ${ask.folder.path}. Your files are not deleted.`}
      onconfirm={confirmAsk} oncancel={() => (ask = null)} />
  {/if}
{/if}

<style>
  h2 { margin: 0 0 8px; font-size: 18px; }
  .hint { color: #666; margin: 0 0 16px; max-width: 60ch; }
  ul { list-style: none; margin: 0 0 16px; padding: 0; display: grid; gap: 6px; }
  li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid #e4e4e8; border-radius: 6px; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #777; font-size: 12px; }
  .prog { display: flex; flex-direction: column; gap: 3px; width: 190px; flex: none; }
  .prog .txt { color: #555; font-size: 12px; }
  .prog progress { width: 100%; height: 6px; }
  .none { color: #777; }
  .err { color: #b00020; }
  .add { background: #2f6fed; color: #fff; border-color: #2f6fed; }
</style>
