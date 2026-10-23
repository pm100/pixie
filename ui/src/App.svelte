<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    analysisRetry, analysisSetPaused, analysisStatus, cancelScan, countPhotos, getMaxPhotoId, openViewerWindow, listDayGroups, listFolderInfo, listFolders, listGeoPoints, listPhotos,
    listTags, removeFolder, setTagFilter, startScan, listTrips, renameTrip, tripRoute, placeFor,
    listPeople, peopleStatus, duplicateStatus, setPersonFilter, activeFilters,
    listFacePeople, onSuggestionCount, onSuggestionFound, onSuggestionLooking, onSuggestionsDone, loadProgress, scanFolders, suggestFolders,
    type DayGroup, type Folder, type FolderInfo, type MediaKind, type Photo, type Route, type SortOrder, type Trip, type View,
  } from './lib/api';
  import type { AnalysisStatus } from './lib/analysis';
  import { cleanCollapsed, toggleSection, type SectionId } from './lib/sections';
  import { cleanTripPrefs, destinationOf, tripTitle, type TripPrefs } from './lib/trips';
  import { createLatest } from './lib/latest';
  import { hitsActive, tagLabel, type TagCount } from './lib/tagNav';
  import { personLabel, type PeopleStatus, type PersonInfo } from './lib/people';
  import FacesTab from './lib/FacesTab.svelte';
  import DuplicatesTab from './lib/DuplicatesTab.svelte';
  import { dupPass, type DupStatus } from './lib/dupes';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import PhotoMap from './lib/PhotoMap.svelte';
  import { parseMapView, sameBounds, type Bounds, type GeoPoint, type MapView } from './lib/geo';
  import { parseKindFilter } from './lib/kind';
  import { POPOUT_CLOSE, POPOUT_CLOSED, POPOUT_MOVED, POPOUT_READY, POPOUT_SHOW, parseMoved } from './lib/popout';
  import { periodLabel, periodRange } from './lib/periods';
  import { parseRecentDays, recentLabel, recentSince } from './lib/recent';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { scanText } from './lib/scanProgress';
  import { folderName } from './lib/format';
  import AllSet from './lib/AllSet.svelte';
  import BackgroundNote from './lib/BackgroundNote.svelte';
  import BackgroundProgress from './lib/BackgroundProgress.svelte';
  import { backgroundProgress } from './lib/backgroundProgress';
  import LoadModal from './lib/LoadModal.svelte';
  import Welcome from './lib/Welcome.svelte';
  import { allSetSummary, phaseAfterScan, type Phase } from './lib/loadFlow';
  import { analysisView, type LoadProgress } from './lib/analysisView';
  import { createStripFeed, type StripItem } from './lib/stripFeed';
  import { thumbUrl } from './lib/url';
  import { addFound, applyCount, tickedPaths, toRows, toggle, type Row } from './lib/suggestions';
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
    order: 'newest', // newest first, except while a trip is chosen: a trip reads best from its first day to its last
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
  // A chosen trip (its `since`). Like Recent and a chosen period it is a date range, so choosing one replaces the others.
  let tripKey = $state<number | null>(null);
  const recentOn = $derived(view.since !== null && periodKey === null && tripKey === null);
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
    if (recentOn) {
      view = { ...view, since: recentSince(Math.floor(Date.now() / 1000), days) };
      selection = emptySelection;
      refresh(true).catch((e) => (message = String(e)));
      if (mode === 'map') loadPoints();
    }
  }
  let analysis = $state<AnalysisStatus | null>(null);

  /** The pop-out pages through the listing by position, so it cannot outlive a change of what the listing shows. */
  function closePopout() {
    if (popoutOpen) { void emit(POPOUT_CLOSE); popoutOpen = false; popoutSnap = null; }
  }

  // Auto tags: what the photos show, as a filter. It combines with a person (a photo must match both).
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

  /** Take the tag and person from the backend, which is what the listings really use: a chip can never be missing
   *  for a filter that is on (or shown for one that is off), however calls overlapped. True when something changed. */
  async function syncFilters(): Promise<boolean> {
    try {
      const f = await activeFilters();
      const changed = f.tag !== activeTag || f.person !== activePerson;
      activeTag = f.tag;
      activePerson = f.person;
      return changed;
    } catch { return false; }
  }

  async function setTag(next: string | null) {
    try { await setTagFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    closePopout();
    view = { ...view, search: hitsActive(next, activePerson) };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  // People: faces grouped by who they are, as a filter. It combines with a tag.
  let peopleList = $state<PersonInfo[]>([]), activePerson = $state<number | null>(null);
  let peopleState = $state<PeopleStatus | null>(null);
  let dupState = $state<DupStatus | null>(null);
  /** How many duplicates Browse is hiding, as of the list on screen. */
  let dupHiddenShown: number | null = null;
  let dupTimer: ReturnType<typeof setTimeout> | undefined;
  /** Extra copies were hidden or shown: bring Browse up to date, in a batch and without resetting scroll. If the user
   *  is holding a selection or has the viewer open (which would shift under them), try again a little later. */
  function dupHiddenChanged() {
    clearTimeout(dupTimer);
    const want = dupState?.hidden ?? null;
    if (want === null || dupHiddenShown === null || want === dupHiddenShown) return;
    dupTimer = setTimeout(async () => {
      if (await refreshLive()) dupHiddenShown = want;
      else dupHiddenChanged();
    }, 3000);
  }
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
    if (await syncFilters()) { // a filter was on that the screen did not show: show it now
      view = { ...view, search: hitsActive(activeTag, activePerson) };
      refresh(true).catch((e) => (message = String(e)));
    }
    if (activePerson !== null && !list.some((p) => p.id === activePerson)) await setPerson(null); // merged away or hidden
  }
  /** People appear as the scan proceeds; refresh the list now and then, not on every progress event. */
  function loadPeopleSoon() {
    if (Date.now() - peopleAt > 4000) loadPeople();
  }

  async function setPerson(next: number | null) {
    try { await setPersonFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    closePopout();
    view = { ...view, search: hitsActive(activeTag, next) };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  /** The People tab changed someone: refresh the list, and re-take the photos of a chosen person (they may have changed). */
  async function peopleTabChanged() {
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
  async function refreshLive(): Promise<boolean> {
    if (liveBusy || interacting()) return false;
    liveBusy = true;
    try {
      const v = { ...view };
      v.maxId = await getMaxPhotoId();
      const [n, g] = await Promise.all([
        countPhotos(v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
        listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search),
      ]);
      const ng = await navGroupsFor(v, g);
      if (!same(v, view) || !same(v, shown) || interacting()) return false;
      if (!sameGroups(groups, g)) {
        total = n;
        groups = g;
        navGroups = ng;
        shown = v; // the new snapshot, together with the new groups
        version++;
      }
      return true;
    } catch { /* the next progress tick retries */ return false; } finally {
      setTimeout(() => (liveBusy = false), 1500); // at most one reload every ~1.5 s
    }
  }

  function setRecent(on: boolean) {
    if (on === recentOn) return; // already in that state
    periodKey = null; // Recent, a chosen period and a chosen trip are all a date range: one replaces the others
    tripKey = null;
    view = { ...view, order: 'newest', since: on ? recentSince(Math.floor(Date.now() / 1000), recentDays) : null, until: null };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  /** Anything narrowing the photos shown (the library choice, folder and kind, are not counted). */
  const filtered = $derived(activeTag !== null || activePerson !== null || periodKey !== null || tripKey !== null || view.folderId !== null || view.since !== null);

  /** "Show all": drop the tag, person and date range in one go. */
  async function showAll() {
    closePopout();
    try {
      await Promise.all([setTagFilter(null), setPersonFilter(null)]);
    } catch (e) { message = String(e); }
    await syncFilters();
    periodKey = null;
    tripKey = null;
    view = { ...view, order: 'newest', search: hitsActive(activeTag, activePerson), since: null, until: null, folderId: null };
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
    tripKey = null;
    view = { ...view, order: 'newest', since: r?.since ?? null, until: r?.until ?? null };
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

  /** Show only one photo folder (the same folder again clears it). */
  function setFolder(id: number | null) {
    if (id === view.folderId) return;
    view = { ...view, folderId: id };
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  /** Show only one trip (the same trip again clears it): its days become the date range. */
  function setTrip(t: Trip | null) {
    if (t === null || t.since === tripKey) {
      if (tripKey === null) return;
      tripKey = null;
      view = { ...view, order: 'newest', since: null, until: null };
      void loadRoute(null);
    } else {
      periodKey = null;
      tripKey = t.since;
      view = { ...view, order: 'oldest', since: t.since, until: t.until };
      void loadRoute(t);
    }
    selection = emptySelection;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  // Trips: runs of days away from home. Home is the user's to confirm (in Settings), so none are shown before that.
  let tripPrefs = $state<TripPrefs>(cleanTripPrefs(loadPref<unknown>('trips', null)));
  let trips = $state<Trip[]>([]);
  let tripsAsked = 0;
  async function loadTrips() {
    const mine = ++tripsAsked;
    const p = tripPrefs;
    if (!p.confirmed || !p.home) { trips = []; return; }
    try {
      const got = await listTrips(p.home.lat, p.home.lon, p.radiusKm, p.minNights);
      if (mine === tripsAsked) { trips = got; forgetDestinations(); }
    } catch (e) { message = String(e); }
  }
  // Where each trip went: found for the trips on show, a few at a time, since each place is looked up once and then kept.
  let destinations = $state<Record<number, string>>({});
  let destDone = new Map<number, string>(); // by trip start; '' when it could not be worked out
  let destRun = 0;
  function forgetDestinations() { destDone = new Map(); destinations = {}; }
  /** The place line of each of the stops with the most days (the rest are left unnamed), looked up one after another. */
  async function placesOf(r: Route, run: () => boolean): Promise<{ names: (string | null)[]; complete: boolean }> {
    const names: (string | null)[] = r.stops.map(() => null);
    const order = r.stops.map((s, i) => i).sort((a, b) => r.stops[b].days - r.stops[a].days).slice(0, 8);
    let complete = true;
    for (const i of order) {
      if (!run()) { complete = false; break; }
      try { names[i] = (await placeFor(r.stops[i].lat, r.stops[i].lon, true))?.locality ?? null; } catch { complete = false; /* offline or busy: say so */ }
    }
    return { names, complete };
  }
  async function resolveDestinations(list: Trip[]) {
    const mine = ++destRun, p = tripPrefs;
    if (!p.home) return;
    for (const t of list) {
      if (mine !== destRun) return;
      if (destDone.has(t.since)) continue;
      try {
        const r = await tripRoute(t.since, t.until, p.home.lat, p.home.lon, p.radiusKm, p.stopKm);
        const found = await placesOf(r, () => mine === destRun);
        if (mine !== destRun) return;
        // A lookup that failed (offline) leaves the trip to be tried again later; a partial answer would name the wrong places.
        if (!found.complete) continue;
        const dest = destinationOf(r, found.names);
        destDone.set(t.since, dest ?? '');
        if (dest) destinations = { ...destinations, [t.since]: dest };
      } catch { destDone.set(t.since, ''); }
    }
  }

  // The route of the chosen trip, for the map.
  let routeData = $state<Route | null>(null), routeNames = $state<(string | null)[]>([]);
  let routeRun = 0;
  async function loadRoute(t: Trip | null) {
    const mine = ++routeRun, p = tripPrefs;
    routeData = null; routeNames = [];
    if (!t || !p.home) return;
    try {
      const r = await tripRoute(t.since, t.until, p.home.lat, p.home.lon, p.radiusKm, p.stopKm);
      if (mine !== routeRun) return;
      routeData = r;
      const found = await placesOf(r, () => mine === routeRun);
      if (mine === routeRun) routeNames = found.names;
    } catch (e) { message = String(e); }
  }

  function setTripPrefs(next: TripPrefs) {
    tripPrefs = next;
    savePref('trips', next);
    forgetDestinations();
    if (tripKey !== null) setTrip(null); // the chosen trip may no longer exist
    void loadTrips();
  }
  async function renameTripTo(t: Trip, name: string) {
    try { await renameTrip(t.since, t.until, name); } catch (e) { message = String(e); }
    void loadTrips();
  }
  const openTripSettings = () => { settingsPage = 'trips'; pickTab('settings'); };

  // Sections of the Browse list the user has folded away.
  let collapsed = $state<SectionId[]>(cleanCollapsed(loadPref<unknown>('collapsedSections', null)));
  function toggleNav(id: SectionId) {
    collapsed = toggleSection(collapsed, id);
    savePref('collapsedSections', collapsed);
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

  // ---- First run and adding folders: welcome, modal scan, analysing, all set ----
  type Sample = StripItem;
  let phase = $state<Phase>('off');
  let rows = $state<Row[]>([]);
  let sniffing = $state(false); // still sniffing for photo folders
  let looking = $state<string | null>(null);
  let strip = $state<Sample[]>([]); // the photos read most recently, oldest first
  let flowFolder = $state<string | null>(null);
  /** How far the analysis has got for the photos of this load's folders (null until first counted). */
  let flowLoad = $state<LoadProgress | null>(null);
  let facePeople = $state(0);
  let allSet = $state<ReturnType<typeof allSetSummary> | null>(null);
  let flowProblems: string[] = []; // folders that could not be read during this flow, for the end screen
  let flowIds: number[] = [];
  let flowStopped = false, flowStartedAt = 0;
  let flowPhotos = 0; // photos (not videos) the scan found in this flow's folders
  let finishing = false; // the end screen is being prepared
  let flowStarted: Promise<void> = Promise.resolve(); // scan-done can arrive before scanFolders has answered
  let welcomeSubs: Promise<() => void>[] = [];
  let feed: ReturnType<typeof createStripFeed> | null = null; // decides which loaded picture the strip shows next
  /** Load a thumbnail in the background so the strip can show it the moment it is ready (the browser caches it). */
  const preload = (item: Sample) =>
    new Promise<boolean>((resolve) => {
      const img = new Image();
      img.onload = () => resolve(true);
      img.onerror = () => resolve(false);
      img.src = thumbUrl('grid', item.id, item.mtime, item.size);
    });
  /** One bar and what each analysis has found, for the analysing screen (which counts only this load's photos) and for
   *  Settings, Picture analysis (which counts the whole library). */
  const flowView = $derived(analysisView({ tags: analysis, faces: peopleState, dupes: dupState, people: facePeople, load: flowLoad }));
  const settingsView = $derived(analysisView({ tags: analysis, faces: peopleState, dupes: dupState, people: facePeople }));

  const stopWelcomeSubs = () => { welcomeSubs.forEach((s) => s.then((un) => un())); welcomeSubs = []; sniffing = false; };

  /** Suggest where the photos probably are. Counts and further candidates arrive as events, possibly before the list. */
  async function openWelcome() {
    rows = [];
    sniffing = true;
    looking = null;
    phase = 'welcome';
    let ready = false;
    const early: (() => void)[] = [];
    const later = (f: () => void) => (ready ? f() : early.push(f));
    welcomeSubs = [
      onSuggestionCount((c) => later(() => (rows = applyCount(rows, c)))),
      onSuggestionFound((s) => later(() => (rows = addFound(rows, s)))),
      onSuggestionLooking((what) => (looking = what)),
      onSuggestionsDone(() => (sniffing = false)),
    ];
    try { rows = toRows(await suggestFolders()); } catch (e) { message = String(e); sniffing = false; }
    ready = true;
    early.forEach((f) => f());
  }

  async function welcomeAdd() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    rows = addFound(rows, { path: dir, name: folderName(dir), kind: 'known', ticked: true, note: 'chosen by you', included_in: null });
  }

  /** Add these folders and scan them with the modal screens. A folder added while another scan runs joins that scan. */
  async function beginLoad(paths: string[]) {
    stopWelcomeSubs();
    message = '';
    flowLoad = null;
    flowStopped = false; flowStartedAt = Date.now(); flowIds = []; flowPhotos = 0; finishing = false;
    strip = []; flowFolder = null; allSet = null; flowProblems = [];
    feed?.stop();
    feed = createStripFeed({ load: preload, onShow: (item) => (strip = [...strip.slice(-4), item]), minGapMs: 100 });
    scanning = true; seen = 0; indexed = 0; toIndex = 0; scanFolder = null;
    phase = 'scanning';
    flowStarted = (async () => {
      try {
        flowIds = (await scanFolders(paths)).folder_ids;
        folders = await listFolders();
      } catch (e) {
        message = String(e);
        feed?.stop();
        scanning = false;
        phase = 'off';
        refresh(true).catch((err) => (message = String(err)));
      }
    })();
    await flowStarted;
  }

  const welcomeStart = () => beginLoad(tickedPaths(rows));

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    await beginLoad([dir]); // folders already exist when this is used: no welcome
  }

  /** The scan ended: stopped, nothing found, or on to the analysing step. */
  async function finishScanPhase() {
    feed?.stop();
    feed = null;
    await flowStarted;
    await loadFolderInfos();
    const mine = folderInfos.filter((f) => flowIds.includes(f.id));
    const found = mine.reduce((n, f) => n + f.count, 0);
    flowPhotos = 0;
    for (const f of mine) flowPhotos += await countPhotos(f.id, 'photo');
    const next = phaseAfterScan({ stopped: flowStopped, found, photos: flowPhotos });
    if (next === 'off') {
      phase = 'off';
      refresh(true).catch((e) => (message = String(e)));
    } else if (next === 'allset') {
      await buildAllSet(); // the screen is ready before it is shown, so nothing blinks in between
      phase = 'allset';
    } else {
      phase = 'analysing';
    }
  }

  async function buildAllSet() {
    const infos = folderInfos.filter((f) => flowIds.includes(f.id));
    let photos = 0, videos = 0;
    for (const f of infos) {
      photos += await countPhotos(f.id, 'photo');
      videos += await countPhotos(f.id, 'video');
    }
    const people = await listFacePeople(false).catch(() => []);
    allSet = allSetSummary({
      folders: infos.map((f) => ({ name: folderName(f.path), count: f.count })),
      photos, videos, seconds: (Date.now() - flowStartedAt) / 1000, facesFound: people.length > 0, problems: flowProblems,
    });
  }

  /** Leave the modal: whatever is still running (the scan, then the analysis) carries on in the background. A note says
   *  so, and says how to tell when it has finished (the indicator at the top right, below). */
  const carryOnInBackground = () => {
    feed?.stop();
    feed = null;
    phase = 'background';
    refresh(true).catch((e) => (message = String(e)));
  };

  /** What the indicator at the top right shows: the scan, a model download or the analysis, whichever is running. */
  const bg = $derived(backgroundProgress({ scanning, scan: { seen, indexed, toIndex }, analysis, people: peopleState, dupes: dupPass(dupState) }));
  const openAnalysisDetails = () => { settingsPage = 'analysis'; pickTab('settings'); };

  // While the analysing screen is showing, count how far its folders' photos have got once a second; while Settings,
  // Picture analysis is showing, count the people found. (The analyses' states arrive as events.)
  $effect(() => {
    const flow = phase === 'analysing';
    if (!flow && !(tab === 'settings' && settingsPage === 'analysis')) return;
    const tick = async () => {
      try {
        // Counted from the folders' own photos, so duplicates found among them (or hiding older photos elsewhere) cannot
        // throw the totals off.
        if (flow && flowIds.length > 0) flowLoad = await loadProgress(flowIds);
        facePeople = (await listFacePeople(false)).length;
      } catch { /* the next tick tries again */ }
    };
    void tick();
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  });
  // The step ends when every counter is done (each analysis has counted the photos the scan found, so a stale status cannot
  // end it early). The end screen is prepared first and swapped in only when ready, so it never flashes or blinks.
  $effect(() => {
    if (phase !== 'analysing' || finishing || !flowView.finished) return;
    finishing = true;
    void buildAllSet().then(() => {
      if (phase === 'analysing') phase = 'allset'; // still here: not sent to the background meanwhile
    });
  });

  /** The end of the load flow: leave it for the Browse tab (in the library view, not the map), at the top of the list. */
  function startBrowsing() {
    phase = 'off';
    pickTab('browse');
    refresh(true).catch((e) => (message = String(e)));
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
    void loadTrips();
    analysisStatus().then((s) => (analysis = s)).catch(() => {});
    peopleStatus().then((s) => (peopleState = s)).catch(() => {});
    duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {});
    loadTags();
    loadPeople();
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) { void openWelcome(); return; }
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      listen<AnalysisStatus>('analysis-status', (e) => { analysis = e.payload; loadTagsSoon(); }),
      listen<DupStatus>('duplicate-status', (e) => { dupState = e.payload; dupHiddenChanged(); }),
      listen<PeopleStatus>('people-status', (e) => { peopleState = e.payload; loadPeopleSoon(); }),
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number; indexed: number; to_index: number; folder_id: number; latest: Sample | null }>('scan-progress', (e) => {
        seen = e.payload.seen; indexed = e.payload.indexed; toIndex = e.payload.to_index; scanFolder = e.payload.folder_id;
        if (phase !== 'scanning') { if (phase === 'off' || phase === 'background') refreshLive(); return; }
        const f = folders.find((x) => x.id === e.payload.folder_id);
        flowFolder = f ? folderName(f.path) : flowFolder;
        if (e.payload.latest) feed?.offer(e.payload.latest);
      }),
      listen('scan-done', () => {
        loadTags();
        loadPeople();
        scanning = false;
        scanFolder = null;
        loadFolderInfos();
        void loadTrips();
        if (openIndex !== null || soloPhoto !== null) pendingRefresh = true;
        else refresh().catch((e) => (message = String(e)));
        if (mode === 'map') loadPoints();
        if (phase === 'scanning') void finishScanPhase();
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
      listen<{ message: string }>('scan-error', (e) => {
        message = e.payload.message; // scan-done always follows
        if (phase !== 'off' && !flowProblems.includes(e.payload.message)) flowProblems = [...flowProblems, e.payload.message];
      }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<!-- The webview's own right-click menu (Share, Magnify image, ...) does nothing useful here, so it is off
     everywhere except in text fields, where copy and paste are wanted. -->
<svelte:window oncontextmenu={noBrowserMenu} />
<main inert={phase !== 'off'}>
  <Tabs {tabs} active={tab} onpick={pickTab}>
    {#snippet status()}
      <BackgroundProgress progress={bg} onclick={openAnalysisDetails} />
      <span>{total.toLocaleString()} {kindWord(view.kind)}{mode === 'map' ? ' in view' : ''}</span>
      {#if message}<span class="err">{message}</span>{/if}
    {/snippet}
    <div class="browse" inert={tab === 'settings' || tab === 'faces' || tab === 'dupes'}>
      <div class="tools">
        {#if activeTag !== null}
          <span class="filter">Tagged “{tagLabel(activeTag)}”
            <button onclick={() => setTag(null)} aria-label="Clear tag" title="Clear tag">✕</button></span>
        {/if}
        {#if activeInfo !== null}
          <span class="filter">Person “{personLabel(activeInfo)}”
            <button onclick={() => setPerson(null)} aria-label="Clear person" title="Clear person">✕</button></span>
        {/if}
        {#if view.folderId !== null}
          <span class="filter">Folder “{folderInfos.find((f) => f.id === view.folderId) ? folderName(folderInfos.find((f) => f.id === view.folderId)!.path) : 'unknown'}”
            <button onclick={() => setFolder(null)} aria-label="Clear folder" title="Clear folder">✕</button></span>
        {/if}
        {#if tripKey !== null}
          <span class="filter">Trip “{trips.find((t) => t.since === tripKey) ? tripTitle(trips.find((t) => t.since === tripKey)!) : 'unknown'}”
            <button onclick={() => setTrip(null)} aria-label="Clear trip" title="Clear trip">✕</button></span>
        {/if}
        {#if periodKey !== null}
          <span class="filter">Showing only {periodLabel(periodKey)}
            <button onclick={() => setPeriod(periodKey!)} aria-label="Show all dates" title="Show all dates">✕</button></span>
        {/if}
        {#if filtered}<button class="showall" onclick={showAll} title="Clear the tag, person and dates">Show all</button>{/if}
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
          <Nav groups={navGroups} {activeGroup} kind={view.kind} recent={recentOn}
               filterDates={mode === 'map'} period={periodKey} recentText={recentLabel(recentDays)}
               {tags} tag={activeTag} ontag={setTag}
               people={peopleList} facesFound={(peopleState?.done ?? 0) > 0} person={activePerson} onperson={setPerson}
               folders={folderInfos} folderId={view.folderId} onfolder={setFolder}
               {trips} tripsReady={tripPrefs.confirmed} trip={tripKey} ontrip={setTrip} {destinations} onvisibletrips={(l) => void resolveDestinations(l)} onrenametrip={renameTripTo} onsettrips={openTripSettings}
               {collapsed} ontoggle={toggleNav}
               scanLine={scanning ? scanText({ seen, indexed, toIndex }) : null}
               ongroup={jump} onperiod={setPeriod} onkind={setKind} onrecent={setRecent} />
        {/snippet}
        {#snippet center()}
          {#if mode === 'map'}
            <div class="mapsplit">
              <div class="maptop">
                <PhotoMap {points} initial={mapStart} resetKey={mapResetKey} route={tripKey !== null ? routeData : null} stopNames={routeNames}
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
            <p class="empty">{view.search ? (activeTag === null ? 'No photos of this person here.' : 'No photos with this tag here.') : periodKey !== null ? 'No photos or videos from this period here.' : tripKey !== null ? 'No photos or videos from this trip here.' : view.since !== null ? `No photos or videos in the ${recentLabel(recentDays).toLowerCase()}.` : `No ${view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.`}</p>
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
    {#if peopleTabBuilt}
      <div class="page-over" hidden={tab !== 'faces'}>
        <FacesTab active={tab === 'faces'} status={peopleState} edits={peopleEdits} onchange={peopleTabChanged} />
      </div>
    {/if}
    {#if tab === 'dupes'}
      <div class="page-over">
        <DuplicatesTab active={tab === 'dupes'} onchange={() => { duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {}); refresh().catch((e) => (message = String(e))); loadFolderInfos(); }} />
      </div>
    {/if}
    {#if tab === 'settings'}
      <div class="page-over">
        <Settings folders={folderInfos} {scanning} {scanFolder} scan={{ seen, indexed, toIndex }} onstop={() => cancelScan()} onadd={chooseFolder} onremove={removeFolderAndRefresh}
                  onrescan={(id, full) => runScan(id, full)}
                  page={settingsPage} onpage={(p) => (settingsPage = p)} {tripPrefs} ontripprefs={setTripPrefs} {viewerMode} onviewer={setViewerMode} {recentDays} onrecentdays={setRecentDays}
                  analysis={settingsView} onanalysispause={(p) => analysisSetPaused(p)} onanalysisretry={() => analysisRetry()} />
      </div>
    {/if}
  </Tabs>
  {#if openIndex !== null || soloPhoto !== null}
    <Viewer index={openIndex ?? 0} {total} view={shown} solo={soloPhoto} onclose={closeViewer} onnav={(i) => (openIndex = i)}
            onpopout={() => popOut(openIndex ?? 0)} />
  {/if}
</main>

{#if phase === 'welcome'}
  <Welcome {rows} searching={sniffing} {looking} ontoggle={(p) => (rows = toggle(rows, p))} onadd={welcomeAdd}
           onskip={() => { stopWelcomeSubs(); phase = 'off'; }} onstart={welcomeStart} />
{:else if phase === 'scanning' || phase === 'analysing'}
  <LoadModal {phase} scan={{ seen, indexed, toIndex }} folder={flowFolder} {strip} view={flowView} notice={message}
             onstop={() => { flowStopped = true; cancelScan(); }} onbackground={carryOnInBackground}
             onretry={() => analysisRetry()} onresume={() => analysisSetPaused(false)} />
{:else if phase === 'background'}
  <BackgroundNote scanning={scanning} onpeople={() => { phase = 'off'; pickTab('faces'); }}
                  onbrowse={startBrowsing} />
{:else if phase === 'allset' && allSet}
  <AllSet summary={allSet} onpeople={() => { phase = 'off'; pickTab('faces'); }}
          onbrowse={startBrowsing}
          onadd={() => { phase = 'off'; void chooseFolder(); }} />
{/if}

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; font-size: 14px; background: #fff; color: #111; }
  main { display: flex; flex-direction: column; height: 100vh; }
  .browse { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .tools { display: flex; gap: 12px; align-items: center; padding: 6px 14px; border-bottom: 1px solid #eee; }
  .content { flex: 1; min-height: 0; }
  .filter { display: inline-flex; align-items: center; gap: 6px; padding: 2px 4px 2px 10px; border-radius: 12px; background: #e3ecff; color: #1a3f9a; font-size: 13px; }
  .filter button { border: 0; background: none; color: inherit; font: inherit; cursor: pointer; padding: 0 6px; border-radius: 10px; }
  .showall { font: inherit; font-size: 13px; }
  .filter button:hover { background: rgba(47, 111, 237, .2); }
  .page-over { position: absolute; inset: 0; z-index: 5; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; }
  .mapbottom { flex: 1; min-height: 0; }
  .empty.small { margin-top: 24px; }
</style>
