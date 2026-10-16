<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    addFolder, cancelScan, countPhotos, listDayGroups, listFolders, listGeoPoints, listPhotos, startScan,
    type DayGroup, type Folder, type MediaKind, type Photo, type SortOrder, type View,
  } from './lib/api';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import PhotoMap from './lib/PhotoMap.svelte';
  import PrintTest from './lib/PrintTest.svelte';
  import { parseMapView, sameBounds, type Bounds, type GeoPoint, type MapView } from './lib/geo';
  import { parseKindFilter } from './lib/kind';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { click, emptySelection, single } from './lib/selection';
  import Shell from './lib/Shell.svelte';
  import Tree from './lib/Tree.svelte';
  import Viewer from './lib/Viewer.svelte';

  const initial: View = {
    order: loadPref<SortOrder>('order', 'newest'),
    folderId: null,
    kind: parseKindFilter(loadPref<unknown>('kind', null)),
    bounds: null,
  };
  const same = (a: View, b: View) =>
    a.order === b.order && a.folderId === b.folderId && a.kind === b.kind && sameBounds(a.bounds, b.bounds);
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
    const [n, g] = await Promise.all([countPhotos(v.folderId, v.kind, v.bounds), listDayGroups(v.order, v.folderId, v.kind, v.bounds)]);
    if (!same(v, view)) return; // a newer request owns the screen
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
      const [n, g] = await Promise.all([countPhotos(v.folderId, v.kind, v.bounds), listDayGroups(v.order, v.folderId, v.kind, v.bounds)]);
      if (!same(v, view) || !same(v, shown)) return;
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
    if (mode === 'map') loadPoints();
  }

  function setKind(kind: MediaKind | null) {
    if (kind === view.kind) return;
    view = { ...view, kind };
    savePref('kind', kind);
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  const kindWord = (k: MediaKind | null) => (k === 'photo' ? 'photos' : k === 'video' ? 'videos' : 'items');

  // Map mode: a map above the grid; the grid shows what is inside the visible map area.
  let mode = $state<'library' | 'map'>('library');
  let points = $state<GeoPoint[]>([]);
  let mapResetKey = $state(0); // bumped when Map is clicked again: back to the whole map
  let soloPhoto = $state<Photo | null>(null); // a photo opened from the map's popup
  let lastMap: MapView | null = parseMapView(loadPref<unknown>('mapView', null)); // latest position, kept for the next opening
  let mapStart = $state<MapView | null>(lastMap); // where the map opens; read once by the map

  /** Points for the map follow the folder and kind filters; the map's own area is not part of them. */
  async function loadPoints() {
    const { folderId, kind } = view;
    try {
      const pts = await listGeoPoints(folderId, kind);
      if (folderId !== view.folderId || kind !== view.kind) return; // the filters moved on
      points = pts;
    } catch (e) {
      message = String(e);
    }
  }

  // A resize (or any move) of the map while the viewer is open must not swap the photo under it:
  // the new area is held until the viewer closes.
  let pendingBounds: { b: Bounds | null } | null = null;

  function setBounds(bounds: Bounds | null) {
    if (mode !== 'map') return;
    if (openIndex !== null || soloPhoto !== null) { pendingBounds = { b: bounds }; return; }
    if (sameBounds(bounds, view.bounds)) return;

    view = { ...view, bounds };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
  }

  /**
   * A red dot was clicked: the grid shows just what is at that spot and the photo is selected, so the
   * details pane shows it. The next pan or zoom of the map goes back to showing the visible area.
   */
  async function focusPhoto(p: Photo) {
    if (mode !== 'map' || p.gps_lat === null || p.gps_lon === null) return;
    const d = 0.00002; // about 2 m
    view = { ...view, bounds: { south: p.gps_lat - d, west: p.gps_lon - d, north: p.gps_lat + d, east: p.gps_lon + d } };
    selection = emptySelection;
    try {
      await refresh(true);
    } catch (e) {
      message = String(e);
      return;
    }
    if (total > 0) selection = click(emptySelection, 0, { ctrl: false, shift: false });
  }

  function closeViewer() {
    openIndex = null;
    soloPhoto = null;
    if (pendingBounds) {
      const p = pendingBounds;
      pendingBounds = null;
      setBounds(p.b);
    }
  }

  function leaveMap() {
    pendingBounds = null;
    mode = 'library';
    points = [];
    view = { ...view, bounds: null };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
  }

  function toggleMap() {
    if (mode === 'map') { mapResetKey++; return; } // clicking Map again shows the whole map; Close map leaves it
    mapStart = lastMap;
    mode = 'map';
    loadPoints();
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
    const { order, folderId, kind, bounds } = shown;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId, kind, bounds).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
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
      listen('scan-done', () => {
        scanning = false;
        refresh().catch((e) => (message = String(e)));
        if (mode === 'map') loadPoints();
      }),
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
    <span class="seg" role="group" aria-label="Show">
      <button aria-pressed={view.kind === null} onclick={() => setKind(null)}>All</button>
      <button aria-pressed={view.kind === 'photo'} onclick={() => setKind('photo')}>Photos</button>
      <button aria-pressed={view.kind === 'video'} onclick={() => setKind('video')}>Videos</button>
    </span>
    <button onclick={() => (page = page === 'print' ? 'library' : 'print')}>Print test</button>
    {#if scanning}
      <span>Scanning… {seen} files</span>
      <button onclick={() => cancelScan()}>Stop</button>
    {:else}
      <span>{total} {kindWord(view.kind)}{mode === 'map' ? ' in view' : ''}</span>
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
          <Tree {groups} {folders} {activeGroup} folderId={view.folderId} mapActive={mode === 'map'}
                ongroup={jump} onfolder={setFolder} onmap={toggleMap} />
        {/snippet}
        {#snippet center()}
          {#if mode === 'map'}
            <div class="mapsplit">
              <div class="maptop">
                <PhotoMap {points} initial={mapStart} resetKey={mapResetKey}
                          onbounds={setBounds}
                          onpin={focusPhoto}
                          onopenphoto={(p) => (soloPhoto = p)}
                          onmove={(v) => { lastMap = v; savePref('mapView', v); }}
                          onclose={leaveMap} />
              </div>
              <div class="mapbottom">
                {#if total === 0 && !scanning}
                  <p class="empty small">No photos or videos in this part of the map.</p>
                {:else}
                  <Grid {groups} {version} view={shown} selected={selection.picked} {scrollTo}
                        onopen={(i) => (openIndex = i)}
                        onselect={(i, mods) => (selection = click(selection, i, mods))}
                        onvisiblegroup={(g) => (activeGroup = g)} />
                {/if}
              </div>
            </div>
          {:else if total === 0 && !scanning}
            <p class="empty">No {view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.</p>
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
  {#if openIndex !== null || soloPhoto !== null}
    <Viewer index={openIndex ?? 0} {total} view={shown} solo={soloPhoto} onclose={closeViewer} onnav={(i) => (openIndex = i)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }
  section { flex: 1; min-height: 0; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; }
  .mapbottom { flex: 1; min-height: 0; }
  .empty.small { margin-top: 24px; }
  .seg { display: inline-flex; }
  .seg button { border-radius: 0; margin: 0 -1px 0 0; }
  .seg button:first-child { border-radius: 4px 0 0 4px; }
  .seg button:last-child { border-radius: 0 4px 4px 0; }
  .seg button[aria-pressed='true'] { background: #2f6fed; color: #fff; border-color: #2f6fed; }
</style>
