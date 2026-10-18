<script lang="ts">
  import type { FolderInfo } from './api';

  let { folders, scanning, onadd, onremove, onrescan }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void;
    onremove: (id: number) => Promise<void>; onrescan: (id: number) => void;
  } = $props();

  let confirming = $state<number | null>(null);
  let error = $state('');

  async function remove(id: number) {
    error = '';
    try { await onremove(id); } catch (e) { error = String(e); }
    confirming = null;
  }
</script>

<h2>Photo folders</h2>
<p class="hint">Pixie looks here for photos and videos. Removing a folder never deletes files; it only stops Pixie showing them.</p>
{#if folders.length === 0}
  <p class="none">No folders yet.</p>
{/if}
<ul>
  {#each folders as f (f.id)}
    <li>
      <span class="path" title={f.path}>{f.path}</span>
      <span class="n">{f.count.toLocaleString()} items</span>
      <button disabled={scanning} onclick={() => onrescan(f.id)}>Rescan</button>
      {#if confirming === f.id}
        <span class="ask">Stop showing this folder?</span>
        <button class="danger" onclick={() => remove(f.id)}>Yes, remove</button>
        <button onclick={() => (confirming = null)}>Cancel</button>
      {:else}
        <button disabled={scanning} title={scanning ? 'Wait for the scan to finish' : ''} onclick={() => (confirming = f.id)}>Remove</button>
      {/if}
    </li>
  {/each}
</ul>
{#if error}<p class="err">{error}</p>{/if}
<button class="add" onclick={onadd}>Add folder…</button>

<style>
  h2 { margin: 0 0 4px; font-size: 20px; }
  .hint { color: #666; margin: 0 0 16px; max-width: 60ch; }
  ul { list-style: none; margin: 0 0 16px; padding: 0; display: grid; gap: 6px; }
  li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid #e4e4e8; border-radius: 6px; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #777; font-size: 13px; }
  .ask { color: #8a4b00; font-size: 13px; }
  .danger { background: #b00020; color: #fff; border-color: #b00020; }
  .none { color: #777; }
  .err { color: #b00020; }
  .add { background: #2f6fed; color: #fff; border-color: #2f6fed; }
</style>
