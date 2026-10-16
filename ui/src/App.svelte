<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    addFolder, cancelScan, countPhotos, listDayGroups, listFolders, listPhotos, startScan,
    type DayGroup, type Folder, type Photo, type SortOrder, type View,
  } from './lib/api';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import PrintTest from './lib/PrintTest.svelte';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { click, emptySelection, single } from './lib/selection';
  import Shell from './lib/Shell.svelte';
  import Tree from './lib/Tree.svelte';
  import Viewer from './lib/Viewer.svelte';

  const initial: View = { order: loadPref<SortOrder>('order', 'newest'), folderId: null };
  let view = $state<View>({ ...initial });
  // The view the loaded groups/pages belong to. It trails `view` by one round trip, so the grid,
  // details and viewer never mix the new order with old data.
  let shown = $state<View>({ ...initial });
  let total = $state(0), version = $state(0);
  let groups = $state<DayGroup[]>([]);
  let folders = $state<Folder[]>([]);
  let selection = $state(emptySelection);
  let selectedPhoto = $state<Photo | null>(null);
  let activeGroup = $state(0);
  let scrollTo = $state<{ group: number; n: number } | null>(null);
  let jumps = 0;
  let scanning = $state(false), seen = $state(0), message = $state('');
  let openIndex = $state<number | null>(null);
  let page = $state<'library' | 'print'>('library');

  // activeGroup is set too: near the end the list cannot scroll far enough to put a short group on top.
  const jump = (group: number) => { activeGroup = group; scrollTo = { group, n: ++jumps }; };

  /** Reload the count and day groups for the current view; the grid refetches on `version`. */
  async function refresh(toTop = false) {
    const v = { ...view };
    const [n, g] = await Promise.all([countPhotos(v.folderId), listDayGroups(v.order, v.folderId)]);
    if (v.order !== view.order || v.folderId !== view.folderId) return; // a newer request owns the screen
    total = n;
    groups = g;
    shown = v;
    selection = emptySelection;
    version++;
    if (toTop) jump(0);
  }

  /** Live update while a scan runs: new photos appear without resetting selection or scroll. */
  let liveBusy = false;
  async function refreshLive() {
    if (liveBusy) return;
    liveBusy = true;
    try {
      const v = { ...view };
      const [n, g] = await Promise.all([countPhotos(v.folderId), listDayGroups(v.order, v.folderId)]);
      if (v.order !== view.order || v.folderId !== view.folderId || v.order !== shown.order || v.folderId !== shown.folderId) return;
      total = n;
      if (!sameGroups(groups, g)) { groups = g; version++; }
    } catch { /* the next progress tick retries */ } finally {
      setTimeout(() => (liveBusy = false), 1500); // at most one reload every ~1.5 s
    }
  }

  function setOrder(order: SortOrder) {
    if (order === view.order) return;
    view = { ...view, order };
    savePref('order', order);
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
  }

  function setFolder(folderId: number | null) {
    if (folderId === view.folderId) return;
    view = { ...view, folderId };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
  }

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const id = await addFolder(dir);
    folders = await listFolders();
    message = '';
    scanning = true; seen = 0; // before the call, so a fast scan-done cannot be overwritten
    try {
      await startScan(id);
    } catch (e) {
      message = String(e);
      if (!/already running/.test(message)) scanning = false; // busy => the other scan is still going
    }
  }

  // Details for a single selected photo (fetched by index in the current view).
  $effect(() => {
    const i = single(selection);
    const { order, folderId } = shown;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
    return () => { live = false; };
  });

  onMount(() => {
    refresh().catch((e) => (message = String(e)));
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) return;
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number }>('scan-progress', (e) => { seen = e.payload.seen; refreshLive(); }),
      listen('scan-done', () => { scanning = false; refresh().catch((e) => (message = String(e))); }),
      listen<{ message: string }>('scan-error', (e) => { message = e.payload.message; /* scan-done always follows */ }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<main>
  <header>
    <strong>Pixie</strong>
    <button onclick={chooseFolder}>Add photo folder…</button>
    <button onclick={() => setOrder(view.order === 'newest' ? 'oldest' : 'newest')}>
      {view.order === 'newest' ? 'Newest first ↓' : 'Oldest first ↑'}
    </button>
    <button onclick={() => (page = page === 'print' ? 'library' : 'print')}>Print test</button>
    {#if scanning}
      <span>Scanning… {seen} photos</span>
      <button onclick={() => cancelScan()}>Stop</button>
    {:else}
      <span>{total} photos</span>
    {/if}
    {#if message}<span class="err">{message}</span>{/if}
  </header>
  <section>
    {#if page === 'print'}
      <PrintTest />
    {:else if folders.length === 0 && total === 0 && !scanning}
      <p class="empty">Add a folder of photos to get started.</p>
    {:else}
      <Shell>
        {#snippet left()}
          <Tree {groups} {folders} {activeGroup} folderId={view.folderId} ongroup={jump} onfolder={setFolder} />
        {/snippet}
        {#snippet center()}
          {#if total === 0 && !scanning}
            <p class="empty">No photos here yet.</p>
          {:else}
            <Grid {groups} {version} view={shown} selected={selection.picked} {scrollTo}
                  onopen={(i) => (openIndex = i)}
                  onselect={(i, mods) => (selection = click(selection, i, mods))}
                  onvisiblegroup={(g) => (activeGroup = g)} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} count={selection.picked.size} />
        {/snippet}
      </Shell>
    {/if}
  </section>
  {#if openIndex !== null}
    <Viewer index={openIndex} {total} view={shown} onclose={() => (openIndex = null)} onnav={(i) => (openIndex = i)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }
  section { flex: 1; min-height: 0; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
</style>
