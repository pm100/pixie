<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    addFolder, analysisRetry, analysisSetPaused, analysisStatus, cancelScan, countPhotos, getMaxPhotoId, openViewerWindow, listDayGroups, listFolderInfo, listFolders, listGeoPoints, listPhotos,
    listTags, removeFolder, searchPhotos, setTagFilter, startScan,
    listPeople, peopleStatus, setPersonFilter,
    type DayGroup, type Folder, type FolderInfo, type MediaKind, type Photo, type SortOrder, type View,
  } from './lib/api';
  import { searchNote, type AnalysisStatus } from './lib/analysis';
  import { createLatest } from './lib/latest';
  import { searchActive, tagLabel, type TagCount } from './lib/tagNav';
  import { personLabel, type PeopleStatus, type PersonInfo } from './lib/people';
  import FacesTab from './lib/FacesTab.svelte';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import MailSettings from './lib/MailSettings.svelte';
  import PhotoMap from './lib/PhotoMap.svelte';
  import { parseMapView, sameBounds, type Bounds, type GeoPoint, type MapView } from './lib/geo';
  import { parseKindFilter } from './lib/kind';
  import { POPOUT_CLOSE, POPOUT_CLOSED, POPOUT_MOVED, POPOUT_READY, POPOUT_SHOW, parseMoved } from './lib/popout';
  import { periodLabel, periodRange } from './lib/periods';
  import { parseRecentDays, recentLabel, recentSince } from './lib/recent';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { scanText } from './lib/scanProgress';
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
    search: false,
  };
  const same = (a: View, b: View) =>
    a.order === b.order && a.folderId === b.folderId && a.kind === b.kind && a.since === b.since && a.until === b.until && a.search === b.search && sameBounds(a.bounds, b.bounds);
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
  let scanFolder = $state<number | null>(null); // the folder the running scan is on (null: none yet)
  let indexed = $state(0), toIndex = $state(0); // files read so far, and how many will be read (0 while walking)
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
  // Search: the box holds what is being typed; `searchTerm` is the search that is applied to the view.
  let analysis = $state<AnalysisStatus | null>(null);
  let searchText = $state(''), searchTerm = $state<string | null>(null), searching = $state(false);
  const searchReady = $derived(analysis?.models === 'ready');

  // Only the latest search or clear may change the view; an older one finishing late is ignored.
  const latestSearch = createLatest();
  /** The pop-out pages through the search result by position, so it cannot outlive a change of that result. */
  function closePopout() {
    if (popoutOpen) { void emit(POPOUT_CLOSE); popoutOpen = false; popoutSnap = null; }
  }

  async function runSearch() {
    const text = searchText.trim();
    if (text === '') return clearSearch();
    const token = latestSearch.next();
    searching = true;
    try {
      await searchPhotos(text);
      if (!latestSearch.isCurrent(token)) return;
      closePopout();
      searchTerm = text;
      view = { ...view, search: true };
      selection = emptySelection;
      await refresh(true);
      if (mode === 'map') loadPoints();
    } catch (e) {
      message = String(e);
    } finally {
      if (latestSearch.isCurrent(token)) searching = false;
    }
  }

  async function clearSearch() {
    latestSearch.next(); // a search still in flight must not bring the results back
    searching = false;
    searchText = '';
    searchTerm = null;
    const next = searchActive(null, activeTag, activePerson); // a chosen tag or person keeps the view restricted
    if (!view.search && !next) return;
    closePopout();
    view = { ...view, search: next };
    selection = emptySelection;
    searchPhotos('').catch(() => {}); // forget the text hits
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  // Tags: what the photos show, as a filter. It combines with a text search (a photo must match both).
  let tags = $state<TagCount[]>([]), activeTag = $state<string | null>(null);
  let tagsAt = 0;
  async function loadTags() {
    tagsAt = Date.now();
    try { tags = await listTags(); } catch { /* the list just stays as it was */ }
  }
  /** Tags appear as analysis proceeds; refresh the list now and then, not on every progress event. */
  function loadTagsSoon() {
    if (Date.now() - tagsAt > 4000) loadTags();
  }

  async function setTag(next: string | null) {
    try { await setTagFilter(next); } catch (e) { message = String(e); return; }
    activeTag = next;
    closePopout();
    view = { ...view, search: searchActive(searchTerm, next, activePerson) };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  // People: faces grouped by who they are, as a filter. It combines with a text search and a tag.
  let peopleList = $state<PersonInfo[]>([]), activePerson = $state<number | null>(null);
  let peopleState = $state<PeopleStatus | null>(null);
  let peopleAt = 0;
  /** Counts people edits made in Browse, so the People tab knows to reload when it is shown again. */
  let peopleEdits = $state(0);
  /** The People tab is built on first use and then kept, so switching back to it is instant. */
  let peopleTabBuilt = $state(false);
  $effect(() => { if (tab === 'faces') peopleTabBuilt = true; });
  const latestPeople = createLatest();
  const activeInfo = $derived(peopleList.find((p) => p.id === activePerson) ?? null);

  async function loadPeople() {
    peopleAt = Date.now();
    const token = latestPeople.next();
    let list: PersonInfo[];
    try { list = await listPeople(); } catch { return; /* the list just stays as it was */ }
    if (!latestPeople.isCurrent(token)) return;
    peopleList = list;
    if (activePerson !== null && !list.some((p) => p.id === activePerson)) await setPerson(null); // merged away or hidden
  }
  /** People appear as the scan proceeds; refresh the list now and then, not on every progress event. */
  function loadPeopleSoon() {
    if (Date.now() - peopleAt > 4000) loadPeople();
  }

  async function setPerson(next: number | null) {
    try { await setPersonFilter(next); } catch (e) { message = String(e); return; }
    activePerson = next;
    closePopout();
    view = { ...view, search: searchActive(searchTerm, activeTag, next) };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  /** From the Faces tab: look at this person's photos in Browse. */
  async function showPerson(id: number) {
    if (tab !== 'browse') pickTab('browse');
    await setPerson(id);
  }

  /** A face was marked "not this person": the lists and, if a person is chosen, the filter must follow. */
  async function facesChanged() {
    peopleEdits++;
    await loadPeople();
    if (activePerson !== null) await setPerson(activePerson);
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
      countPhotos(v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
      listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
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
    return listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, null, null, v.search);
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
        countPhotos(v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
        listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
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
    const { folderId, kind, since, until, search } = view;
    try {
      const pts = await listGeoPoints(folderId, kind, since, until, search);
      if (folderId !== view.folderId || kind !== view.kind || since !== view.since || until !== view.until || search !== view.search) return; // the filters moved on
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
    if (mode === 'map') { mapResetKey++; return; } // clicking Map again shows the whole map
    mapStart = lastMap;
    mode = 'map';
    tab = 'map';
    loadPoints();
  }

  /** Start a scan of one folder (or all, with null) and show progress. */
  async function runScan(id: number | null, full = false) {
    message = '';
    scanning = true; seen = 0; indexed = 0; toIndex = 0; scanFolder = id; // before the call, so a fast scan-done cannot be overwritten
    try {
      await startScan(id, full);
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
    closePopout(); // its photos may be gone
    loadTags();
    loadPeople();
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
    const { order, folderId, kind, bounds, maxId, since, until, search } = shown;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId, kind, bounds, maxId, since, until, search).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
    return () => { live = false; };
  });

  onMount(() => {
    refresh().catch((e) => (message = String(e)));
    loadFolderInfos();
    analysisStatus().then((s) => (analysis = s)).catch(() => {});
    peopleStatus().then((s) => (peopleState = s)).catch(() => {});
    loadTags();
    loadPeople();
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) return;
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      listen<AnalysisStatus>('analysis-status', (e) => { analysis = e.payload; loadTagsSoon(); }),
      listen<PeopleStatus>('people-status', (e) => { peopleState = e.payload; loadPeopleSoon(); }),
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number; indexed: number; to_index: number; folder_id: number }>('scan-progress', (e) => {
        seen = e.payload.seen; indexed = e.payload.indexed; toIndex = e.payload.to_index; scanFolder = e.payload.folder_id;
        refreshLive();
      }),
      listen('scan-done', () => {
        loadTags();
        loadPeople();
        scanning = false;
        scanFolder = null;
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
      <span>{total.toLocaleString()} {kindWord(view.kind)}{mode === 'map' ? ' in view' : ''}</span>
      {#if message}<span class="err">{message}</span>{/if}
    {/snippet}
    <div class="browse" inert={tab === 'settings' || tab === 'faces'}>
      <div class="tools">
        <button onclick={() => setOrder(view.order === 'newest' ? 'oldest' : 'newest')}>
          {view.order === 'newest' ? 'Newest first ↓' : 'Oldest first ↑'}
        </button>
        <form class="search" onsubmit={(e) => { e.preventDefault(); runSearch(); }}>
          <input type="search" bind:value={searchText} placeholder="Search, e.g. dog on a beach" aria-label="Search photos"
                 disabled={!searchReady || searching}
                 title={searchReady ? 'Describe what you are looking for' : 'Search is not ready yet: see Settings, Search'}
                 onkeydown={(e) => { if (e.key === 'Escape') clearSearch(); }} />
        </form>
        {#if view.search && searchTerm !== null}
          <span class="filter">Matching “{searchTerm}”
            <button onclick={clearSearch} aria-label="Clear search" title="Clear search">✕</button></span>
        {/if}
        {#if activeTag !== null}
          <span class="filter">Tagged “{tagLabel(activeTag)}”
            <button onclick={() => setTag(null)} aria-label="Clear tag" title="Clear tag">✕</button></span>
        {/if}
        {#if activeInfo !== null}
          <span class="filter">Person “{personLabel(activeInfo)}”
            <button onclick={() => setPerson(null)} aria-label="Clear person" title="Clear person">✕</button></span>
        {/if}
        {#if view.search && analysis && searchNote(analysis)}<span class="note">{searchNote(analysis)}</span>{/if}
        {#if periodKey !== null}
          <span class="filter">Showing only {periodLabel(periodKey)}
            <button onclick={() => setPeriod(periodKey!)} aria-label="Show all dates" title="Show all dates">✕</button></span>
        {/if}
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
               {tags} tag={activeTag} ontag={setTag}
               people={peopleList} person={activePerson} onperson={setPerson}
               scanLine={scanning ? scanText({ seen, indexed, toIndex }) : null}
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
                          onmove={(v) => { lastMap = v; savePref('mapView', v); }} />
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
            <p class="empty">{view.search ? (searchTerm !== null ? `Nothing matches “${searchTerm}”.` : activeTag === null ? 'No photos of this person here.' : 'No photos with this tag here.') : periodKey !== null ? 'No photos or videos from this period here.' : view.since !== null ? `No photos or videos in the ${recentLabel(recentDays).toLowerCase()}.` : `No ${view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.`}</p>
          {:else}
            <Grid {groups} {version} view={shown} selected={selection.picked} {scrollTo}
                  onopen={openItem}
                  onselect={(i, mods) => (selection = click(selection, i, mods))}
                  onvisiblegroup={(g) => (activeGroup = g)} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} count={selection.picked.size} people={peopleList} onfaceschange={facesChanged} />
        {/snippet}
      </Shell>
        {/if}
      </div>
      <SelectionBar count={selection.picked.size} onclear={() => (selection = emptySelection)} />
    </div>
    {#if peopleTabBuilt}
      <div class="page-over" hidden={tab !== 'faces'}>
        <FacesTab active={tab === 'faces'} status={peopleState} edits={peopleEdits} onchange={loadPeople} onshow={showPerson} />
      </div>
    {/if}
    {#if tab === 'settings'}
      <div class="page-over">
        <Settings folders={folderInfos} {scanning} {scanFolder} scan={{ seen, indexed, toIndex }} onstop={() => cancelScan()} onadd={chooseFolder} onremove={removeFolderAndRefresh}
                  onrescan={(id, full) => runScan(id, full)} onemail={() => (settingsOpen = true)}
                  page={settingsPage} onpage={(p) => (settingsPage = p)} {viewerMode} onviewer={setViewerMode} {recentDays} onrecentdays={setRecentDays}
                  {analysis} people={peopleState} onanalysispause={(p) => analysisSetPaused(p)} onanalysisretry={() => analysisRetry()} />
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
  .search input { width: 260px; max-width: 40vw; font: inherit; font-size: 13px; padding: 3px 8px; border: 1px solid #c3c8d4; border-radius: 12px; }
  .search input:disabled { background: #f3f4f7; color: #888; }
  .note { font-size: 12px; color: #666; }
  .filter { display: inline-flex; align-items: center; gap: 6px; padding: 2px 4px 2px 10px; border-radius: 12px; background: #e3ecff; color: #1a3f9a; font-size: 13px; }
  .filter button { border: 0; background: none; color: inherit; font: inherit; cursor: pointer; padding: 0 6px; border-radius: 10px; }
  .filter button:hover { background: rgba(47, 111, 237, .2); }
  .page-over { position: absolute; inset: 0; z-index: 5; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; }
  .mapbottom { flex: 1; min-height: 0; }
  .empty.small { margin-top: 24px; }
</style>
