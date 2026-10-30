<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { duplicateGroups, duplicateStatus, removeDuplicates, setDuplicateKeeper } from './api';
  import { dupLine, extrasOf, totalSize, type DupGroup, type DupStatus } from './dupes';
  import { formatBytes, formatDateTime } from './format';
  import { thumbUrl } from './url';
  import VideoPoster from './VideoPoster.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import type { Photo } from './api';

  let { active, onchange }: {
    /** This tab is the one on screen. */
    active: boolean;
    /** Copies were moved to the Recycle Bin: the rest of the app should refresh its counts. */
    onchange: () => void;
  } = $props();

  let groups = $state<DupGroup[]>([]);
  let status = $state<DupStatus | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  /** What is waiting for a yes: the files that would go to the Recycle Bin. */
  let confirming = $state<Photo[] | null>(null);
  let busy = $state(false);
  /** How many sets were on screen when the list was loaded; a different count means there is more to see. */
  let loadedGroups = $state(0);

  async function load() {
    try {
      const [g, s] = await Promise.all([duplicateGroups(), duplicateStatus()]);
      groups = g;
      status = s;
      loadedGroups = s.groups;
      loaded = true;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    const sub = listen<DupStatus>('duplicate-status', (e) => { status = e.payload; });
    return () => { sub.then((off) => off()); };
  });
  // Load when first shown and each time the tab is shown again; the list does not change while it is being looked at
  // (new sets are offered with a button), so a set never moves under the user's mouse.
  let wasActive = false;
  $effect(() => {
    const now = active;
    if (now && !wasActive) untrack(load);
    wasActive = now;
  });

  const more = $derived(status !== null && loaded && status.groups !== loadedGroups);
  const checking = $derived(status !== null && status.done < status.total);
  const extras = $derived(extrasOf(groups));
  const dirOf = (path: string) => path.replace(/[\\/][^\\/]*$/, '');
  const nameOf = (path: string) => path.replace(/^.*[\\/]/, '');
  const coverOf = (g: DupGroup) => g.photos.find((p) => p.id === g.keeper) ?? g.photos[0];

  /** Show this copy in Browse for its set. The choice is saved, so it survives a rescan. */
  async function keep(id: number) {
    error = null;
    try {
      await setDuplicateKeeper(id);
      await load();
      onchange();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(photos: Photo[]) {
    confirming = null;
    busy = true;
    error = null;
    try {
      await removeDuplicates(photos.map((p) => p.id));
      await load();
      onchange();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="dupes">
  <div class="top">
    <header>
      <h2>Duplicates</h2>
      {#if error}<span class="err">{error}</span>{/if}
    </header>
    {#if status}
      <div class="status" role="status">
        <span>{dupLine(status)}</span>
        {#if checking}<progress value={status.total > 0 ? status.done / status.total : 0} max="1"></progress>{/if}
        {#if more}<button onclick={load} title="Add the duplicates found since this list was made">Show newly found duplicates</button>{/if}
      </div>
    {/if}
    {#if groups.length > 0}
      <div class="bar">
        <button class="danger" disabled={busy || extras.length === 0} onclick={() => (confirming = extras)}>
          Move all {extras.length.toLocaleString()} extra {extras.length === 1 ? 'file' : 'files'} to the Recycle Bin…
        </button>
        <span class="hint">In each set the one marked “Keep” stays, and it is the one Browse shows. Nothing is deleted for good.</span>
      </div>
    {/if}
  </div>

  <div class="scroll">
    {#if !loaded}
      <p class="hint">Loading…</p>
    {:else if groups.length === 0}
      <p class="hint">{checking ? 'No duplicates so far. Pixie is still looking, in the background.' : 'Every photo and video is a different file.'}</p>
    {:else}
      {#each groups as g (g.hash)}
        {@const others = g.photos.filter((p) => p.id !== g.keeper)}
        <section class="set">
          <div class="cover">
            {#if coverOf(g).kind === 'video'}
              {#key coverOf(g).id}<VideoPoster photo={coverOf(g)} />{/key}
            {:else}
              <img alt="" loading="lazy" decoding="async" src={thumbUrl('grid', coverOf(g).id, coverOf(g).mtime, coverOf(g).size)} />
            {/if}
          </div>
          <div class="copies">
            {#each g.photos as p (p.id)}
              <label class="copy" class:keep={p.id === g.keeper}>
                <input type="radio" name={g.hash} checked={p.id === g.keeper} disabled={busy} onchange={() => keep(p.id)} />
                <span class="mark">{p.id === g.keeper ? 'Keep' : 'Extra'}</span>
                <span class="where" title={p.path}><strong>{nameOf(p.path)}</strong> <span class="dir">{dirOf(p.path)}</span></span>
                <span class="meta">{formatDateTime(p.taken_at)} · {formatBytes(p.size)}</span>
              </label>
            {/each}
            <div class="row">
              <button disabled={busy} onclick={() => (confirming = others)}>
                Move the other {others.length === 1 ? 'duplicate' : `${others.length} duplicates`} to the Recycle Bin…
              </button>
            </div>
          </div>
        </section>
      {/each}
    {/if}
  </div>
</div>

{#if confirming}
  <ConfirmDialog
    title="Move to the Recycle Bin?"
    message={`${confirming.length.toLocaleString()} ${confirming.length === 1 ? 'file' : 'files'} (${formatBytes(totalSize(confirming))}) will be moved to the Recycle Bin. The one marked “Keep” in each set stays where it is. You can get them back from the Recycle Bin.`}
    confirmLabel="Move to Recycle Bin"
    danger
    onconfirm={() => remove(confirming ?? [])}
    oncancel={() => (confirming = null)} />
{/if}

<style>
  .dupes { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; font-size: 14px; }
  .top { flex: none; padding: 14px 18px 8px; border-bottom: 1px solid #e3e6ee; }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 14px 18px 24px; display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: baseline; gap: 16px; }
  h2 { margin: 0 0 4px; font-size: 18px; }
  .hint { color: #777; margin: 6px 0; font-size: 13px; }
  .err { color: #b3261e; }
  .status { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 6px 0; padding: 6px 12px; border-radius: 8px; background: #eef2fb; color: #2a3f7a; font-size: 13px; }
  .status progress { width: 160px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; margin-top: 8px; }
  button { font: inherit; font-size: 13px; }
  .set { display: flex; gap: 14px; padding: 10px; border: 1px solid #d3d8e4; border-radius: 10px; background: #fafbfe; }
  .cover { flex: none; width: 128px; height: 128px; }
  .cover img { width: 100%; height: 100%; object-fit: cover; border-radius: 8px; background: #dde2ee; display: block; }
  .copies { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .copy { display: grid; grid-template-columns: auto 48px 1fr auto; align-items: baseline; gap: 8px; padding: 3px 6px; border-radius: 6px; cursor: pointer; }
  .copy.keep { background: #e6f2e6; }
  .mark { font-size: 12px; font-weight: 600; color: #555; }
  .copy.keep .mark { color: #1d6b2a; }
  .where { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dir { color: #777; font-size: 12px; }
  .meta { color: #666; font-size: 12px; white-space: nowrap; }
  .row { margin-top: 4px; }
</style>
