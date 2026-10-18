<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    addFolder, cancelScan, countPhotos, getMaxPhotoId, openViewerWindow, listDayGroups, listFolderInfo, listFolders, listGeoPoints, listPhotos,
    removeFolder, startScan,
    type DayGroup, type Folder, type FolderInfo, type MediaKind, type Photo, type SortOrder, type View,
  } from './lib/api';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import MailSettings from './lib/MailSettings.svelte';
  import PhotoMap from './lib/PhotoMap.svelte';
  import { parseMapView, sameBounds, type Bounds, type GeoPoint, type MapView } from './lib/geo';
  import { parseKindFilter } from './lib/kind';
  import { POPOUT_CLOSE, POPOUT_CLOSED, POPOUT_MOVED, POPOUT_READY, POPOUT_SHOW, parseMoved } from './lib/popout';
  import { periodRange } from './lib/periods';
  import { parseRecentDays, recentLabel, recentSince } from './lib/recent';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { click, emptySelection, single } from './lib/selection';
  import SelectionBar from './lib/SelectionBar.svelte';
  import Settings, { type SettingsPage } from './lib/Settings.svelte';
  import Shell from './lib/Shell.svelte';
  import Tabs from './lib/Tabs.svelte';
  import { pickAction, visibleTabs, type TabId } from './lib/tabs';
  import { parseViewerMode, type ViewerMode } from './lib/viewerPref';
  import Nav from './lib/Nav.svelte';
  import Viewer from './lib/Viewer.svelte';

  const initial: View = {
    order: loadPref<SortOrder>('order', 'newest'),
    folderId: null,
    kind: parseKindFilter(loadPref<unknown>('kind', null)),
    bounds: null,
    maxId: null,
    since: null,
    until: null,
  };
  const same = (a: View, b: View) =>
    a.order === b.order && a.folderId === b.folderId && a.kind === b.kind && a.since === b.since && a.until === b.until && sameBounds(a.bounds, b.bounds);
  let view = $state<View>({ ...initial });
  // The view the loaded groups/pages belong to. It trails `view` by one round trip, so the grid,
  // details and viewer never mix the new order with old data.
  let shown = $state<View>({ ...initial });
  let total = $state(0), version = $state(0);
  let groups = $state<DayGroup[]>([]);
  // The dates offered in the left pane. On the map a chosen period filters the pins and the grid; the pane must
  // stay whole so another period can still be picked, so it is built without that period.
  let navGroups = $state<DayGroup[]>([]);
  let periodKey = $state<string | null>(null); // y:2025, m:2025-06 or d:2025-06-14, only on the map
  let folders = $state<Folder[]>([]);
  let selection = $state(emptySelection);
  let selectedPhoto = $state<Photo | null>(null);
  let activeGroup = $state(0);
  let scrollTo = $state<{ group: number; n: number } | null>(null);
  let jumps = 0;
  let scanning = $state(false), seen = $state(0), message = $state('');
  let openIndex = $state<number | null>(null);

  // The separate viewer window: it keeps the snapshot it was sent, and follows the selection when asked to.
  let popoutOpen = false;
  // What the pop-out was last told to show: the position together with the listing that position refers to.
  let popoutSnap: { index: number; total: number; view: View } | null = null;
  const sameListing = (a: View, b: View) => same(a, b) && a.maxId === b.maxId;

  function sendShow(follow: boolean) {
    if (!popoutSnap) return;
    void emit(POPOUT_SHOW, { ...popoutSnap, follow });
  }

  /** Show item `i` in the separate window (creating it if needed) and close the in-window viewer. */
  async function popOut(i: number) {
    popoutSnap = { index: i, total, view: { ...shown } }; // before closeViewer(), which may start a refresh
    popoutOpen = true;
    closeViewer();
    try { await openViewerWindow(); } catch (e) { message = String(e); popoutOpen = false; return; }
    sendShow(false); // a window that already existed will not say "ready" again; a new one asks below
  }

  const openItem = (i: number) => { if (viewerMode === 'popout') void popOut(i); else openIndex = i; };

  // Keep the pop-out in step with the grid's selection (the pop-out ignores this while its Follow is off).
  $effect(() => {
    const i = single(selection);
    if (!popoutOpen || i === null || total < 1 || i >= total) return;
    if (popoutSnap && popoutSnap.index === i && sameListing(popoutSnap.view, shown)) return;
    popoutSnap = { index: i, total, view: { ...shown } };
    sendShow(true);
  });
  let settingsOpen = $state(false); // the email account dialog
  let tab = $state<TabId>('browse');
  let viewerMode = $state<ViewerMode>(parseViewerMode(loadPref<unknown>('viewerMode', null)));
  function setViewerMode(m: ViewerMode) { viewerMode = m; savePref('viewerMode', m); }
  // How many days "recent" means. Changing it while the Recent filter is on re-applies the filter.
  let recentDays = $state(parseRecentDays(loadPref<unknown>('recentDays', null)));
  function setRecentDays(n: number) {
    const days = parseRecentDays(n);
    if (days === recentDays) return;
    recentDays = days;
    savePref('recentDays', days);
    if (view.since !== null && periodKey === null) {
      view = { ...view, since: recentSince(Math.floor(Date.now() / 1000), days) };
      selection = emptySelection;
      refresh(true).catch((e) => (message = String(e)));
      if (mode === 'map') loadPoints();
    }
  }
  let settingsPage = $state<SettingsPage>('folders');
  let folderInfos = $state<FolderInfo[]>([]);
  const tabs = visibleTabs({ create: false, share: false });

  async function loadFolderInfos() {
    try { folderInfos = await listFolderInfo(); } catch (e) { message = String(e); }
  }

  /** Tabs only choose what is drawn: selection, scroll and filters live here and survive a switch. */
  function pickTab(id: TabId) {
    closeViewer();
    switch (pickAction(id, mode, tab)) {
      case 'enter-map': toggleMap(); break;
      case 'show-map': tab = 'map'; break;
      case 'reset-map': mapResetKey++; break;
      case 'leave-map': leaveMap(); break;
      default: tab = id;
    }
  }

  function noBrowserMenu(e: MouseEvent) {
    const t = e.target;
    if (t instanceof Element && t.closest('input, textarea, select, [contenteditable="true"]')) return;
    e.preventDefault();
  }

  // activeGroup is set too: near the end the list cannot scroll far enough to put a short group on top.
  const jump = (group: number) => { activeGroup = group; scrollTo = { group, n: ++jumps }; };

  /** Reload the count and day groups for the current view; the grid refetches on `version`. */
  async function refresh(toTop = false) {
    const v = { ...view };
    // The listing is frozen at this snapshot until the next refresh: photos a scan adds meanwhile have
    // higher ids and cannot shift the positions that selection, details and the viewer rely on.
    v.maxId = await getMaxPhotoId();
    const [n, g] = await Promise.all([
      countPhotos(v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until),
      listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until),
    ]);
    const ng = await navGroupsFor(v, g);
    if (!same(v, view)) return; // a newer request owns the screen
    total = n;
    groups = g;
    navGroups = ng;
    shown = v;
    selection = emptySelection;
    version++;
    if (toTop) jump(0);
  }

  /** The day groups for the left pane: the same as `g`, except that on the map a chosen period is left out. */
  async function navGroupsFor(v: View, g: DayGroup[]): Promise<DayGroup[]> {
    if (mode !== 'map' || periodKey === null) return g;
    return listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, null, null);
  }

  /** True while the user is looking at or holding something that refers to a position in the list. */
  const interacting = () => selection.picked.size > 0 || openIndex !== null || soloPhoto !== null;

  /**
   * Live update while a scan runs: new photos appear without resetting scroll. Only while the user is
   * not interacting: moving the snapshot shifts positions, which would swap the photo they selected or
   * are viewing for another one.
   */
  let liveBusy = false;
  async function refreshLive() {
    if (liveBusy || interacting()) return;
    liveBusy = true;
    try {
      const v = { ...view };
      v.maxId = await getMaxPhotoId();
      const [n, g] = await Promise.all([
        countPhotos(v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until),
        listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until),
      ]);
      const ng = await navGroupsFor(v, g);
      if (!same(v, view) || !same(v, shown) || interacting()) return;
      if (!sameGroups(groups, g)) {
        total = n;
        groups = g;
        navGroups = ng;
        shown = v; // the new snapshot, together with the new groups
        version++;
      }
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

  function setRecent(on: boolean) {
    if (on === (view.since !== null && periodKey === null)) return; // already in that state
    periodKey = null; // Recent and a chosen period are both a date range: one replaces the other
    view = { ...view, since: on ? recentSince(Math.floor(Date.now() / 1000), recentDays) : null, until: null };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  /** On the map: show only one year, month or day (the same key again clears it). */
  function setPeriod(key: string) {
    const next = key === periodKey ? null : key;
    const r = next === null ? null : periodRange(next);
    if (next !== null && !r) return;
    periodKey = next;
    view = { ...view, since: r?.since ?? null, until: r?.until ?? null };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    loadPoints();
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
    const { folderId, kind, since, until } = view;
    try {
      const pts = await listGeoPoints(folderId, kind, since, until);
      if (folderId !== view.folderId || kind !== view.kind || since !== view.since || until !== view.until) return; // the filters moved on
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

  // A scan that finishes while the viewer is open must not swap what is on screen: the refresh waits.
  let pendingRefresh = false;

  function closeViewer() {
    openIndex = null;
    soloPhoto = null;
    if (pendingBounds) {
      const p = pendingBounds;
      pendingBounds = null;
      pendingRefresh = false; // setBounds refreshes anyway
      setBounds(p.b);
    } else if (pendingRefresh) {
      pendingRefresh = false;
      refresh().catch((e) => (message = String(e)));
    }
  }

  function leaveMap() {
    pendingBounds = null;
    mode = 'library';
    tab = 'browse';
    points = [];
    // A period chosen on the map is a map-only filter; Recent stays.
    const clearPeriod = periodKey !== null;
    periodKey = null;
    view = { ...view, bounds: null, ...(clearPeriod ? { since: null, until: null } : {}) };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
  }

  function toggleMap() {
    if (mode === 'map') { mapResetKey++; return; } // clicking Map again shows the whole map; Close map leaves it
    mapStart = lastMap;
    mode = 'map';
    tab = 'map';
    loadPoints();
  }

  /** Start a scan of one folder (or all, with null) and show progress. */
  async function runScan(id: number | null) {
    message = '';
    scanning = true; seen = 0; // before the call, so a fast scan-done cannot be overwritten
    try {
      await startScan(id);
    } catch (e) {
      message = String(e);
      if (!/already running/.test(message)) scanning = false; // busy => the other scan is still going
    }
  }

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const id = await addFolder(dir);
    folders = await listFolders();
    await loadFolderInfos();
    await runScan(id);
  }

  async function removeFolderAndRefresh(id: number) {
    await removeFolder(id);
    if (popoutOpen) { void emit(POPOUT_CLOSE); popoutOpen = false; popoutSnap = null; } // its photos may be gone
    folders = await listFolders();
    await loadFolderInfos();
    closeViewer();
    selection = emptySelection;
    await refresh(true);
    if (mode === 'map') loadPoints();
  }

  // Details for a single selected photo (fetched by index in the current view).
  $effect(() => {
    const i = single(selection);
    const { order, folderId, kind, bounds, maxId, since, until } = shown;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId, kind, bounds, maxId, since, until).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
    return () => { live = false; };
  });

  onMount(() => {
    refresh().catch((e) => (message = String(e)));
    loadFolderInfos();
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
        loadFolderInfos();
        if (openIndex !== null || soloPhoto !== null) pendingRefresh = true;
        else refresh().catch((e) => (message = String(e)));
        if (mode === 'map') loadPoints();
      }),
      listen(POPOUT_READY, () => sendShow(false)),
      listen(POPOUT_CLOSED, () => { popoutOpen = false; }),
      listen(POPOUT_MOVED, (e) => {
        const m = parseMoved(e.payload);
        // Only follow while the main window still shows the listing the pop-out is paging through.
        if (!m || m.total !== total || !sameListing(m.view, shown)) return;
        popoutSnap = { index: m.index, total, view: { ...shown } };
        selection = click(emptySelection, m.index, { ctrl: false, shift: false });
      }),
      listen<{ message: string }>('scan-error', (e) => { message = e.payload.message; /* scan-done always follows */ }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<!-- The webview's own right-click menu (Share, Magnify image, ...) does nothing useful here, so it is off
     everywhere except in text fields, where copy and paste are wanted. -->
<svelte:window oncontextmenu={noBrowserMenu} />
<main>
  <Tabs {tabs} active={tab} onpick={pickTab}>
    {#snippet status()}
      {#if scanning}
        <span>Scanning… {seen} files</span>
        <button onclick={() => cancelScan()}>Stop</button>
      {:else}
        <span>{total} {kindWord(view.kind)}{mode === 'map' ? ' in view' : ''}</span>
      {/if}
      {#if message}<span class="err">{message}</span>{/if}
    {/snippet}
    <div class="browse" inert={tab === 'settings'}>
      <div class="tools">
        <button onclick={() => setOrder(view.order === 'newest' ? 'oldest' : 'newest')}>
          {view.order === 'newest' ? 'Newest first ↓' : 'Oldest first ↑'}
        </button>
      </div>
      <div class="content">
        {#if folders.length === 0 && total === 0 && !scanning}
          <div class="empty">
            <p>Add a folder of photos to get started.</p>
            <button onclick={chooseFolder}>Add photo folder…</button>
          </div>
        {:else}
      <Shell>
        {#snippet left()}
          <Nav groups={navGroups} {activeGroup} kind={view.kind} recent={view.since !== null && periodKey === null}
               filterDates={mode === 'map'} period={periodKey} recentText={recentLabel(recentDays)}
               scanLine={scanning ? `Scanning… ${seen.toLocaleString()} found` : null}
               ongroup={jump} onperiod={setPeriod} onkind={setKind} onrecent={setRecent} />
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
                        onopen={openItem}
                        onselect={(i, mods) => (selection = click(selection, i, mods))}
                        onvisiblegroup={(g) => (activeGroup = g)} />
                {/if}
              </div>
            </div>
          {:else if total === 0 && !scanning}
            <p class="empty">{periodKey !== null ? 'No photos or videos from this period here.' : view.since !== null ? `No photos or videos in the ${recentLabel(recentDays).toLowerCase()}.` : `No ${view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.`}</p>
          {:else}
            <Grid {groups} {version} view={shown} selected={selection.picked} {scrollTo}
                  onopen={openItem}
                  onselect={(i, mods) => (selection = click(selection, i, mods))}
                  onvisiblegroup={(g) => (activeGroup = g)} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} count={selection.picked.size} />
        {/snippet}
      </Shell>
        {/if}
      </div>
      <SelectionBar count={selection.picked.size} onclear={() => (selection = emptySelection)} />
    </div>
    {#if tab === 'settings'}
      <div class="page-over">
        <Settings folders={folderInfos} {scanning} onadd={chooseFolder} onremove={removeFolderAndRefresh}
                  onrescan={(id) => runScan(id)} onemail={() => (settingsOpen = true)}
                  page={settingsPage} onpage={(p) => (settingsPage = p)} {viewerMode} onviewer={setViewerMode} {recentDays} onrecentdays={setRecentDays} />
      </div>
    {/if}
  </Tabs>
  {#if settingsOpen}
    <MailSettings onclose={() => (settingsOpen = false)} />
  {/if}
  {#if openIndex !== null || soloPhoto !== null}
    <Viewer index={openIndex ?? 0} {total} view={shown} solo={soloPhoto} onclose={closeViewer} onnav={(i) => (openIndex = i)}
            onpopout={() => popOut(openIndex ?? 0)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; font-size: 14px; background: #fff; color: #111; }
  main { display: flex; flex-direction: column; height: 100vh; }
  .browse { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .tools { display: flex; gap: 12px; align-items: center; padding: 6px 14px; border-bottom: 1px solid #eee; }
  .content { flex: 1; min-height: 0; }
  .page-over { position: absolute; inset: 0; z-index: 5; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; }
  .mapbottom { flex: 1; min-height: 0; }
  .empty.small { margin-top: 24px; }
</style>
