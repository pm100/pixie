<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    analysisRetry, analysisSetPaused, analysisStatus, cancelScan, countPhotos, getMaxPhotoId, openViewerWindow, listDayGroups, listFolderInfo, listFolders, listGeoPoints, listPhotos,
    listTags, removeFolder, setTagFilter, setUserTagFilter, userTags as fetchUserTags, type UserTagCount, startScan, setNetworkRescan, listTrips, renameTrip, tripRoute, placeFor,
    listPeople, faceOverview, peopleStatus, duplicateStatus, setPersonFilter, setCameraFilter, activeFilters, pickIds, pickPhoto, pickMany, clearPicks, tagPhotos, untagPhotos, getAnalysisOptions, setAnalysisOptions, loadStatus, type LoadStatus,
    facetCounts, onSuggestionCount, onSuggestionFound, onSuggestionLooking, onSuggestionsDone, loadProgress, scanFolders, suggestFolders, folderStates,
    type DayGroup, type Folder, type FolderInfo, type MediaKind, type Photo, type Route, type SortOrder, type Trip, type View,
  } from './lib/api';
  import type { AnalysisStatus } from './lib/analysis';
  import { toStates, type FolderStates } from './lib/folderState';
  import { cleanCollapsed, toggleSection, type SectionId } from './lib/sections';
  import { cleanTripPrefs, destinationOf, tripTitle, type TripPrefs } from './lib/trips';
  import { createLatest } from './lib/latest';
  import { hitsActive, tagLabel, type TagCount } from './lib/tagNav';
  import { personLabel, type PeopleStatus, type PersonInfo } from './lib/people';
  import PeopleTab from './lib/PeopleTab.svelte';
  import DuplicatesTab from './lib/DuplicatesTab.svelte';
  import { dupPass, type DupStatus } from './lib/dupes';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import PhotoMap from './lib/PhotoMap.svelte';
  import { parseMapView, sameBounds, type Bounds, type GeoPoint, type MapView } from './lib/geo';
  import { parseKindFilter } from './lib/kind';
  import { POPOUT_CLOSE, POPOUT_CLOSED, POPOUT_MOVED, POPOUT_READY, POPOUT_SHOW, parseMoved } from './lib/popout';
  import { periodLabel, periodRange } from './lib/periods';
  import { clearAllTitle, filterNames, offerClearAll } from './lib/filterSummary';
  import { parseRecentDays, recentLabel, recentSince } from './lib/recent';
  import { parseNetworkRescan, rescanMinutes, type NetworkRescan } from './lib/networkRescan';
  import { sameGroups } from './lib/layout';
  import { loadPref, savePref } from './lib/prefs';
  import { folderName, plural } from './lib/format';
  import AllSet from './lib/AllSet.svelte';
  import BackgroundNote from './lib/BackgroundNote.svelte';
  import BackgroundProgress from './lib/BackgroundProgress.svelte';
  import { backgroundProgress } from './lib/backgroundProgress';
  import LoadModal from './lib/LoadModal.svelte';
  import Welcome from './lib/Welcome.svelte';
  import { allSetSummary, phaseAfterScan, type Phase } from './lib/loadFlow';
  import { analysisView, type LoadProgress } from './lib/analysisView';
  import { indexFacets, type Facets } from './lib/facets';
  import { createStripFeed, type StripItem } from './lib/stripFeed';
  import { thumbUrl } from './lib/url';
  import { addFound, applyCount, notInLibrary, tickedPaths, toRows, toggle, type Row } from './lib/suggestions';
  import { run } from './lib/selection';
  import Settings, { type SettingsPage } from './lib/Settings.svelte';
  import Shell from './lib/Shell.svelte';
  import Tabs from './lib/Tabs.svelte';
  import { visibleTabs, type TabId } from './lib/tabs';
  import ToolsTab from './lib/ToolsTab.svelte';
  import { listingAffected } from './lib/userTags';
  import AnalysisStep from './lib/AnalysisStep.svelte';
  import AskTab from './lib/AskTab.svelte';
  import { newInstallId } from './lib/ask';
  import { placeFor as askPlace } from './lib/askGoTo';
  import { buildLibrary } from './lib/askLibrary';
  import type { Handlers, SettingValues } from './lib/askSettings';
  import { ALL_ON, clearsFilters, parseOptions, type AnalysisOptions } from './lib/analysisOptions';
  import { parseLimit } from './lib/emailSize';
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
  /** The grid is limited to the part of the map in view ("Only this area"); see Map mode below. */
  let areaOnly = $state(false);
  // The view the loaded groups/pages belong to. It trails `view` by one round trip, so the grid,
  // details and viewer never mix the new order with old data.
  let shown = $state<View>({ ...initial });
  let total = $state(0), version = $state(0);
  let groups = $state<DayGroup[]>([]);
  // The dates offered in the left pane. On the map a chosen period filters the pins and the grid; the pane must
  // stay whole so another period can still be picked, so it is built without that period.
  let navGroups = $state<DayGroup[]>([]);
  let periodKey = $state<string | null>(null); // the chosen date: y:2025, m:2025-06, d:2025-06-14 or undated
  // A chosen trip (its `since`). Like Recent and a chosen period it is a date range, so choosing one replaces the others.
  let tripKey = $state<number | null>(null);
  const recentOn = $derived(view.since !== null && periodKey === null && tripKey === null);
  let folders = $state<Folder[]>([]);
  /** The photo chosen in the grid (its position in the listing): its details show on the right. */
  let selected = $state<number | null>(null);
  let selectedPhoto = $state<Photo | null>(null);
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

  // Keep the pop-out in step with the photo chosen in the grid (the pop-out ignores this while its Follow is off).
  $effect(() => {
    const i = selected;
    if (!popoutOpen || i === null || total < 1 || i >= total) return;
    if (popoutSnap && popoutSnap.index === i && sameListing(popoutSnap.view, shown)) return;
    popoutSnap = { index: i, total, view: { ...shown } };
    sendShow(true);
  });
  let tab = $state<TabId>('browse');
  let viewerMode = $state<ViewerMode>(parseViewerMode(loadPref<unknown>('viewerMode', null)));
  /** The most an email may carry, in megabytes (Settings, Email): what is bigger is offered smaller or split. */
  let emailLimitMb = $state(parseLimit(loadPref<unknown>('emailLimitMb', null)));
  function setEmailLimit(mb: number) { emailLimitMb = parseLimit(mb); savePref('emailLimitMb', emailLimitMb); }
  function setViewerMode(m: ViewerMode) { viewerMode = m; savePref('viewerMode', m); }
  // Whether, and how often, folders on the network are scanned again (folders on this computer are watched, so need no
  // setting). The backend keeps the timer; it is told at start and whenever this changes.
  let networkRescan = $state<NetworkRescan>(parseNetworkRescan(loadPref<unknown>('networkRescan', null)));
  function setNetworkRescanPref(r: NetworkRescan) {
    networkRescan = r;
    savePref('networkRescan', r);
  }
  $effect(() => {
    setNetworkRescan(rescanMinutes(networkRescan)).catch(() => {});
  });
  // How many days "recent" means. Changing it while the Recent filter is on re-applies the filter.
  let recentDays = $state(parseRecentDays(loadPref<unknown>('recentDays', null)));
  function setRecentDays(n: number) {
    const days = parseRecentDays(n);
    if (days === recentDays) return;
    recentDays = days;
    savePref('recentDays', days);
    if (recentOn) {
      view = { ...view, since: recentSince(Math.floor(Date.now() / 1000), days) };
      selected = null;
      refresh(true).catch((e) => (message = String(e)));
      if (mode === 'map') { fitMap++; loadPoints(); }
    }
  }
  let analysis = $state<AnalysisStatus | null>(null);

  /** The pop-out pages through the listing by position; it is closed when the listing it pages through is gone. */
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
      const changed = f.tag !== activeTag || f.person !== activePerson || f.camera !== activeCamera || f.utag !== activeUtag;
      activeTag = f.tag;
      activeUtag = f.utag;
      activePerson = f.person;
      activeCamera = f.camera;
      return changed;
    } catch { return false; }
  }

  // Picked and your own tags: one of them can be chosen in the tree, and it combines with the others. `activeUtag` is `picked` or `tag:<name>`.
  let userTagList = $state<UserTagCount[]>([]), activeUtag = $state<string | null>(null);
  async function loadUserTags() {
    try { userTagList = await fetchUserTags(); } catch { /* the list just stays as it was */ }
  }
  async function setUtag(next: string | null) {
    try { await setUserTagFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }
  /** Picks or tags changed: read the tag list again, and if Picked or a tag is chosen, what it lists changed too (a tag
   *  that lost its last photo is gone from the library, so the chip goes with it). */
  async function utagChanged(touched?: string) {
    await loadUserTags();
    if (activeUtag === null) return;
    const before = activeUtag;
    await syncFilters();
    if (!listingAffected(before, touched, activeUtag === before)) return; // the listing is as it was: keep what is selected
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag) };
    refresh().catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  async function setTag(next: string | null) {
    try { await setTagFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(next, activePerson, activeCamera, activeUtag) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  // People: faces grouped by who they are, as a filter. It combines with a tag.
  let peopleList = $state<PersonInfo[]>([]), activePerson = $state<number | null>(null);
  /** The camera chosen in the tree, by its tidied name ("Pixel 7"). */
  let activeCamera = $state<string | null>(null);

  async function setCamera(next: string | null) {
    try { await setCameraFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(activeTag, activePerson, next, activeUtag) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }
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
      view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag) };
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
    view = { ...view, search: hitsActive(activeTag, next, activeCamera, activeUtag) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  /** The People tab changed someone: refresh the list, and re-take the photos of a chosen person (they may have changed). */
  async function peopleTabChanged() {
    await loadPeople();
    if (activePerson !== null) await setPerson(activePerson);
  }

  let settingsPage = $state<SettingsPage>('folders');
  let folderInfos = $state<FolderInfo[]>([]);
  // Quick pick: the photos picked with the marker on the grid (by id, kept in the library); the Tools tab holds them.
  let picked = $state<ReadonlySet<number>>(new Set());
  async function loadPicks() {
    try { picked = new Set(await pickIds()); } catch { /* the next time */ }
  }
  async function togglePick(id: number) {
    const on = !picked.has(id);
    const next = new Set(picked);
    if (on) next.add(id); else next.delete(id);
    picked = next;
    try { await pickPhoto(id, on); } catch (e) { message = String(e); await loadPicks(); }
    if (activeUtag === 'picked') void utagChanged();
  }
  /** A click on a photo in Browse. It chooses the photo (its details show). Ctrl-click also picks it, or takes it off Picked; Shift-click
   *  picks every photo from the one chosen to this one (the choice stays, for the next Shift-click). */
  function selectItem(i: number, mods: { ctrl: boolean; shift: boolean }) {
    if (mods.shift) { void pickPositions(run(selected, i)); return; }
    selected = i;
    if (mods.ctrl) void togglePickAt(i);
  }
  async function togglePickAt(i: number) {
    const v = shown;
    try {
      const [p] = await listPhotos(i, 1, v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search);
      if (p) await togglePick(p.id);
    } catch (e) { message = String(e); }
  }
  async function pickPositions(indexes: number[]) {
    const v = shown; // the listing the positions refer to
    const sorted = [...indexes].sort((a, b) => a - b);
    const ids: number[] = [];
    try {
      // The photos at those positions: one read for each run of neighbours (at most 500 at a time).
      for (let k = 0; k < sorted.length; ) {
        let len = 1;
        while (k + len < sorted.length && sorted[k + len] === sorted[k] + len && len < 500) len++;
        const page = await listPhotos(sorted[k], len, v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search);
        ids.push(...page.map((p) => p.id));
        k += len;
      }
      const fresh = ids.filter((id) => !picked.has(id));
      if (fresh.length === 0) return;
      picked = new Set([...picked, ...fresh]);
      await pickMany(fresh);
      if (activeUtag === 'picked') void utagChanged();
    } catch (e) {
      message = String(e);
      await loadPicks();
    }
  }
  async function clearAllPicks() {
    picked = new Set();
    try { await clearPicks(); } catch (e) { message = String(e); await loadPicks(); }
    if (activeUtag === 'picked') void utagChanged(); else void loadUserTags();
  }
  // The loading (files found and not yet stored, which get their thumbnails as they are): it goes on whatever analyses are on.
  // `loadPeak` is the most files waiting at once since it was last idle, which is what the pill's percentage is measured from.
  let loadState = $state<LoadStatus>({ pending: 0, paused: false });
  let loadPeak = $state(0);
  function setLoad(s: LoadStatus) {
    loadState = s;
    loadPeak = s.pending === 0 ? 0 : Math.max(loadPeak, s.pending);
  }
  // Which analyses are on (Settings, Picture analysis; asked on the welcome screen): they all are until the person says otherwise.
  let analysisOptions = $state<AnalysisOptions>({ ...ALL_ON });
  /** The choice on the welcome screen, applied when Start is pressed. */
  let welcomeOptions = $state<AnalysisOptions>({ ...ALL_ON });
  async function applyAnalysisOptions(next: AnalysisOptions) {
    const before = analysisOptions;
    analysisOptions = next;
    try { await setAnalysisOptions(next); } catch (e) { message = String(e); analysisOptions = before; return; }
    const drop = clearsFilters(next);
    if (drop.tag && activeTag !== null) await setTag(null);
    if (drop.person && activePerson !== null) await setPerson(null);
    if (!next.dupes && before.dupes) { refresh(true).catch((e) => (message = String(e))); void loadFolderInfos(); } // every copy shows again
  }
  // A tab whose analysis was switched off is gone: leave it.
  $effect(() => { if ((tab === 'faces' && !analysisOptions.faces) || (tab === 'dupes' && !analysisOptions.dupes)) tab = 'browse'; });
  const tabs = $derived(visibleTabs({ create: false, share: false, people: analysisOptions.faces, dupes: analysisOptions.dupes }));

  // Ask Pixie sends typed questions to a server, so it is off until the person turns it on (here or on the tab itself).
  let askOn = $state(loadPref<unknown>('askOn', false) === true);
  /** The random id the proxy counts the day's questions by; made when Ask Pixie is first turned on. */
  let askInstall = $state(loadPref<unknown>('askInstall', null));
  function setAskOn(on: boolean) {
    askOn = on;
    savePref('askOn', on);
    if (on && typeof askInstall !== 'string') { askInstall = newInstallId(); savePref('askInstall', askInstall); }
  }
  /** Whether Ask Pixie may be told the names in the library (tags, named people, trips); off until the person says so. */
  let askLibraryOn = $state(loadPref<unknown>('askLibrary', false) === true);
  function setAskLibrary(on: boolean) { askLibraryOn = on; savePref('askLibrary', on); }
  /** The Ask Pixie tab is built on first use and then kept, so the conversation survives a visit to another tab. */
  let askBuilt = $state(false);
  let askFrom = $state<TabId>('browse');
  $effect(() => { if (tab === 'ask') askBuilt = true; else askFrom = tab; });


  async function loadFolderInfos() {
    try { folderInfos = await listFolderInfo(); } catch (e) { message = String(e); }
    void refreshFolderStates();
  }

  // While photos are still being found and stored, the counts beside the folders follow along (every 3 seconds), not
  // only when a scan ends.
  let countsBusy = false;
  $effect(() => {
    const t = setInterval(async () => {
      const working = scanning || (analysis !== null && analysis.done < analysis.total);
      if (!working || countsBusy || document.visibilityState !== 'visible') return;
      countsBusy = true;
      try { folderInfos = await listFolderInfo(); } catch { /* the next time */ }
      countsBusy = false;
    }, 3000);
    return () => clearInterval(t);
  });

  /** Which folders can be reached right now; asked again every 30 seconds while the window is showing. */
  let folderStateMap = $state<FolderStates>({});
  async function refreshFolderStates() {
    try { folderStateMap = toStates(await folderStates()); } catch { /* the next time */ }
  }
  $effect(() => {
    const t = setInterval(() => { if (document.visibilityState === 'visible') void refreshFolderStates(); }, 30_000);
    return () => clearInterval(t);
  });

  /** Tabs only choose what is drawn: selection, scroll and filters live here and survive a switch. */
  function pickTab(id: TabId) {
    closeViewer();
    tab = id;
  }

  function noBrowserMenu(e: MouseEvent) {
    const t = e.target;
    if (t instanceof Element && t.closest('input, textarea, select, [contenteditable="true"]')) return;
    e.preventDefault();
  }

  const jump = (group: number) => { scrollTo = { group, n: ++jumps }; };

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
    selected = null;
    version++;
    if (toTop) {
      jump(0);
      followPopout();
    }
  }

  /** The listing was changed by a choice in the browse tree: the pop-out follows to the first item of the new listing
   *  (it ignores this when its Follow is off), or closes when nothing is left to show. */
  function followPopout() {
    if (!popoutOpen) return;
    if (total < 1) { closePopout(); return; }
    popoutSnap = { index: 0, total, view: { ...shown } };
    sendShow(true);
  }

  /** The day groups for the When list: the same as `g`, except that a chosen date (or Last 30 days) is left out, as
   *  every section is counted without its own filter, so the other dates stay there to choose. */
  async function navGroupsFor(v: View, g: DayGroup[]): Promise<DayGroup[]> {
    if (periodKey === null && !recentOn) return g;
    return listDayGroups(v.order, v.folderId, v.kind, v.bounds, v.maxId, null, null, v.search);
  }

  /** True while the user is looking at or holding something that refers to a position in the list. */
  const interacting = () => selected !== null || openIndex !== null || soloPhoto !== null;

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
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  /** Anything narrowing the photos shown (the library choice, folder and kind, are not counted). */

  /** "Show all": drop every filter (tag, person, folder, trip, dates and media type) in one go. */
  async function showAll() {
    try {
      await Promise.all([setTagFilter(null), setPersonFilter(null), setCameraFilter(null), setUserTagFilter(null)]);
    } catch (e) { message = String(e); }
    await syncFilters();
    periodKey = null;
    tripKey = null;
    areaOnly = false;
    view = { ...view, order: 'newest', search: hitsActive(activeTag, activePerson, activeCamera, activeUtag), since: null, until: null, folderId: null, kind: null, bounds: null };
    savePref('kind', null);
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  /** Show only one year, month or day, or the items without a date (the same key again clears it). */
  function setPeriod(key: string) {
    const next = key === periodKey ? null : key;
    const r = next === null ? null : periodRange(next);
    if (next !== null && !r) return;
    periodKey = next;
    tripKey = null;
    view = { ...view, order: 'newest', since: r?.since ?? null, until: r?.until ?? null };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  function setKind(kind: MediaKind | null) {
    if (kind === view.kind) return;
    view = { ...view, kind };
    savePref('kind', kind);
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  /** Show only one photo folder (the same folder again clears it). */
  function setFolder(id: number | null) {
    if (id === view.folderId) return;
    view = { ...view, folderId: id };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
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
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { fitMap++; loadPoints(); }
  }

  // Trips: runs of days away from home. Home is the user's to confirm (in Settings), so none are shown before that.
  let tripPrefs = $state<TripPrefs>(cleanTripPrefs(loadPref<unknown>('trips', null)));
  let trips = $state<Trip[]>([]);

  /** What the grid says when nothing matches. */
  const emptyMessage = $derived(view.search ? (activeUtag === 'picked' ? 'Nothing is picked.' : activeTag === null && activeUtag === null ? 'No photos of this person here.' : 'No photos with this tag here.') : periodKey !== null ? 'No photos or videos from this period here.' : tripKey !== null ? 'No photos or videos from this trip here.' : view.since !== null ? `No photos or videos in the ${recentLabel(recentDays).toLowerCase()}.` : `No ${view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.`);

  /** The filters in force, as the pills read (the date is the chosen period or Last 30 days). */
  const activeNames = $derived(filterNames({
    tag: activeTag === null ? null : tagLabel(activeTag),
    utag: activeUtag,
    person: activeInfo === null ? null : personLabel(activeInfo),
    camera: activeCamera,
    folder: view.folderId === null ? null : (folderInfos.find((f) => f.id === view.folderId) ? folderName(folderInfos.find((f) => f.id === view.folderId)!.path) : 'unknown'),
    trip: tripKey === null ? null : (trips.find((t) => t.since === tripKey) ? tripTitle(trips.find((t) => t.since === tripKey)!) : 'unknown'),
    date: periodKey !== null ? `Showing only ${periodLabel(periodKey)}` : recentOn ? recentLabel(recentDays) : null,
    kind: view.kind,
    area: areaOnly,
  }));
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

  const kindWord = (k: MediaKind | null, count: number) => (k === 'photo' ? plural(count, 'photo') : k === 'video' ? plural(count, 'video') : plural(count, 'item'));

  // Map mode (the Grid | Map switch in Browse's toolbar): a map above the grid. The map moves freely; the grid follows
  // the part in view only when the user asks ("Only this area", which then shows as a pill like every other filter).
  let mode = $state<'library' | 'map'>(loadPref<unknown>('browseMap', false) === true ? 'map' : 'library');
  let mapBounds: Bounds | null = null; // the area the map shows now
  let points = $state<GeoPoint[]>([]);
  /** Counts the choices made in the browse tree: the map shows all of what was chosen when the new points arrive. */
  let fitMap = $state(0);
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
    mapBounds = bounds;
    if (!areaOnly) return; // the map moves freely; only an area the user asked for filters the grid
    if (openIndex !== null || soloPhoto !== null) { pendingBounds = { b: bounds }; return; }
    if (sameBounds(bounds, view.bounds)) return;

    view = { ...view, bounds };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
  }

  /**
   * A red dot was clicked: the grid shows just what is at that spot and the photo is selected, so the
   * details pane shows it. The next pan or zoom of the map goes back to showing the visible area.
   */
  async function focusPhoto(p: Photo) {
    if (mode !== 'map' || p.gps_lat === null || p.gps_lon === null) return;
    areaOnly = true; // the grid is now limited to this spot, which is a filter like any other (it shows as a pill)
    const d = 0.00002; // about 2 m
    view = { ...view, bounds: { south: p.gps_lat - d, west: p.gps_lon - d, north: p.gps_lat + d, east: p.gps_lon + d } };
    selected = null;
    try {
      await refresh(true);
    } catch (e) {
      message = String(e);
      return;
    }
    if (total > 0) selected = 0;
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

  /** Show the map above the grid, or go back to the grid alone (which drops the map area; every other filter stays). */
  function setMapShown(on: boolean) {
    if (on === (mode === 'map')) return;
    savePref('browseMap', on);
    if (on) {
      mapStart = lastMap;
      mode = 'map';
      loadPoints();
      return;
    }
    pendingBounds = null;
    mode = 'library';
    points = [];
    mapBounds = null;
    if (areaOnly) {
      areaOnly = false;
      view = { ...view, bounds: null };
      selected = null;
      refresh(true).catch((e) => (message = String(e)));
    }
  }

  /** "Only this area": the grid follows the part of the map in view. Off again (the pill's ✕): the grid shows everything the
   *  other filters allow, wherever the map is. */
  function setAreaOnly(on: boolean) {
    if (on === areaOnly || mode !== 'map') return;
    areaOnly = on;
    pendingBounds = null;
    view = { ...view, bounds: on ? mapBounds : null };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
  }

  // ---- First run and adding folders: welcome, modal scan, analysing, all set ----
  type Sample = StripItem;
  let phase = $state<Phase>('off');
  let rows = $state<Row[]>([]);
  /** The folder picker is being used to add folders later (from Settings), not as the first-run welcome. */
  let addMode = $state(false);
  /** (adding) The wider search of the computer has been asked for. */
  /** The picker's rows: when adding, without folders the library already has. */
  const pickerRows = $derived(addMode ? notInLibrary(rows, folders.map((f) => f.path)) : rows);
  let sniffing = $state(false); // still sniffing for photo folders
  let looking = $state<string | null>(null);
  let strip = $state<Sample[]>([]); // the photos read most recently, oldest first
  let flowFolder = $state<string | null>(null);
  /** How far the analysis has got for the photos of this load's folders (null until first counted). */
  let flowLoad = $state<LoadProgress | null>(null);
  let facePeople = $state(0);
  /** The counts beside the entries of the left-hand tree under the filters in force; null until first counted. */
  let facets = $state<Facets | null>(null);
  const facetsLatest = createLatest();
  $effect(() => {
    // Anything that changes what the tree's counts mean: the view's filters, the chosen tag and person, the trips, and a
    // refresh of the listing (a scan added photos, a duplicate was hidden...).
    const v = view, tag = activeTag, person = activePerson, camera = activeCamera, utag = activeUtag, list = trips, chosenTrip = tripKey, days = recentDays;
    version;
    const token = facetsLatest.next();
    const timer = setTimeout(() => {
      facetCounts(v, tag, person, camera, utag, chosenTrip !== null, list.map((t): [number, number] => [t.since, t.until]), recentSince(Math.floor(Date.now() / 1000), days))
        .then((f) => { if (facetsLatest.isCurrent(token)) facets = indexFacets(f, list); })
        .catch(() => { /* the entries keep the counts they came with */ });
    }, 120);
    return () => clearTimeout(timer);
  });
  let allSet = $state<ReturnType<typeof allSetSummary> | null>(null);
  let flowProblems: string[] = []; // folders that could not be read during this flow, for the end screen
  let flowIds: number[] = [];
  let flowStopped = false, flowStartedAt = 0;
  let flowPhotos = 0; // photos (not videos) the scan found in this flow's folders
  let finishing = false; // the end screen is being prepared
  let flowStarted: Promise<void> = Promise.resolve(); // scan-done can arrive before scanFolders has answered
  let welcomeSubs: Promise<() => void>[] = [];
  let feed: ReturnType<typeof createStripFeed> | null = null; // decides which loaded picture the strip shows next
  // The strip goes on through both steps of the loading screen (finding the files, then analysing); it stops when that ends.
  $effect(() => {
    if (phase !== 'scanning' && phase !== 'analysing') {
      feed?.stop();
      feed = null;
    }
  });
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
  const flowView = $derived(analysisView({ tags: analysis, faces: peopleState, dupes: dupState, people: facePeople, load: flowLoad, on: analysisOptions }));
  // The folders just added: until their photos are all analysed, the bars (Settings, Picture analysis, and the indicator at
  // the top right) show how far those have got, out of all that was found in them. With nothing newly added they show
  // the whole library.
  let trackIds = $state<number[]>([]);
  let trackLoad = $state<LoadProgress | null>(null);
  $effect(() => {
    const ids = trackIds;
    if (ids.length === 0) { trackLoad = null; return; }
    let live = true;
    const tick = async () => {
      try { const lp = await loadProgress(ids); if (live) trackLoad = lp; } catch { /* the next time */ }
    };
    void tick();
    const t = setInterval(tick, 1500);
    return () => { live = false; clearInterval(t); };
  });
  // The load screen with every analysis off: it is still loading the files, which is what it says (not "analysis is off").
  const flowViewShown = $derived(
    !analysisOptions.tags && !analysisOptions.faces && !analysisOptions.dupes
      ? { ...flowView, headline: 'Loading your photos…', fraction: flowLoad && flowLoad.photos + flowLoad.pending > 0 ? flowLoad.photos / (flowLoad.photos + flowLoad.pending) : null }
      : flowView,
  );
  const trackView = $derived(analysisView({ tags: analysis, faces: peopleState, dupes: dupState, people: facePeople, load: trackLoad, on: analysisOptions }));
  const trackActive = $derived(trackIds.length > 0 && trackLoad !== null && !trackView.finished);
  // Done: back to the whole library (not while folders are still being read, when nothing may have been stored yet).
  $effect(() => {
    if (trackIds.length > 0 && trackLoad !== null && !scanning && trackView.finished) trackIds = [];
  });
  const trackNames = $derived(folderInfos.filter((f) => trackIds.includes(f.id)).map((f) => folderName(f.path)).join(', '));
  const settingsView = $derived(analysisView({ tags: analysis, faces: peopleState, dupes: dupState, people: facePeople, load: trackActive ? trackLoad : undefined, on: analysisOptions }));

  const stopWelcomeSubs = () => { welcomeSubs.forEach((s) => s.then((un) => un())); welcomeSubs = []; sniffing = false; };

  /** Suggest where the photos probably are. Counts and further candidates arrive as events, possibly before the list.
   *  `adding`: the same screen, used later to add folders: only the usual places are looked at (the rest of the computer
   *  on request), and nothing is ticked to begin with. */
  const openWelcome = () => openPicker(false);
  async function openPicker(adding: boolean) {
    addMode = adding;
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
    try {
      const list = toRows(await suggestFolders());
      rows = adding ? list.map((r) => ({ ...r, ticked: false })) : list;
    } catch (e) { message = String(e); sniffing = false; }
    ready = true;
    early.forEach((f) => f());
  }

  function cancelAdd() {
    stopWelcomeSubs();
    addMode = false;
    phase = 'off';
  }

  async function welcomeAdd() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    rows = addFound(rows, { path: dir, name: folderName(dir), kind: 'known', ticked: true, note: 'chosen by you', included_in: null });
  }

  /** Add these folders and scan them with the modal screens. A folder added while another scan runs joins that scan. */
  async function beginLoad(paths: string[]) {
    stopWelcomeSubs();
    addMode = false;
    message = '';
    flowLoad = null;
    trackIds = []; trackLoad = null;
    flowStopped = false; flowStartedAt = Date.now(); flowIds = []; flowPhotos = 0; finishing = false;
    strip = []; flowFolder = null; allSet = null; flowProblems = [];
    feed?.stop();
    feed = createStripFeed({ load: preload, onShow: (item) => (strip = [...strip.slice(-4), item]), minGapMs: 100 });
    scanning = true; seen = 0; indexed = 0; toIndex = 0; scanFolder = null;
    phase = 'scanning';
    flowStarted = (async () => {
      try {
        flowIds = (await scanFolders(paths)).folder_ids;
        trackIds = flowIds;
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

  // The first run has two screens: the folders (Next), then what Pixie does with the photos (Start). Adding folders later is one.
  const welcomeNext = async () => {
    if (addMode) await beginLoad(tickedPaths(pickerRows));
    else phase = 'options';
  };
  const optionsStart = async () => {
    await applyAnalysisOptions(welcomeOptions);
    await beginLoad(tickedPaths(pickerRows));
  };

  /** Add folders: the same folder picker as the welcome screen (see `openPicker`). */
  const chooseFolder = () => openPicker(true);

  /** The scan ended: stopped, nothing found, or on to the analysing step. */
  async function finishScanPhase(found: number) {
    await flowStarted;
    await loadFolderInfos();
    // The walk only listed the files: the analysis stores each as it reads it, so the photos so far are those stored plus
    // those still to come.
    const lp = flowIds.length > 0 ? await loadProgress(flowIds) : null;
    flowPhotos = lp ? lp.photos + lp.pending : 0;
    if (lp) flowLoad = lp; // fresh at the moment the walk ended
    const next = phaseAfterScan({ stopped: flowStopped, found, photos: flowPhotos });
    if (next === 'off') {
      phase = 'off';
      refresh(true).catch((e) => (message = String(e)));
    } else if (next === 'allset') {
      await allStored(); // videos are stored by the same pass: wait for them, so the end screen counts them all
      await buildAllSet(); // the screen is ready before it is shown, so nothing blinks in between
      phase = 'allset';
    } else {
      phase = 'analysing';
    }
  }

  /** Wait until every file the walk found has been stored (nothing is left outstanding in the analysis). */
  async function allStored() {
    while (flowIds.length > 0) {
      const lp = await loadProgress(flowIds).catch(() => null);
      if (lp && lp.pending + lp.pending_videos === 0) return;
      await new Promise((r) => setTimeout(r, 300));
    }
  }

  async function buildAllSet() {
    await loadFolderInfos(); // the counts are as of now: the photos are stored as they are analysed, not when the walk ends
    const infos = folderInfos.filter((f) => flowIds.includes(f.id));
    let photos = 0, videos = 0;
    for (const f of infos) {
      photos += await countPhotos(f.id, 'photo');
      videos += await countPhotos(f.id, 'video');
    }
    const overview = await faceOverview().catch(() => null);
    allSet = allSetSummary({
      folders: infos.map((f) => ({ name: folderName(f.path), count: f.count })),
      photos, videos, seconds: (Date.now() - flowStartedAt) / 1000, facesFound: overview !== null && overview.groups + overview.people > 0, problems: flowProblems,
    });
  }

  /** Stop on the load screen. While the folders are being read: stop reading (what was found is kept, and the scan's end
   *  takes us to Browse). Once photos are being analysed: pause the analysis, which can be resumed from the indicator at
   *  the top right or in Settings. */
  const stopLoad = async () => {
    if (phase === 'scanning') {
      flowStopped = true;
      cancelScan();
      return;
    }
    try { await analysisSetPaused(true); } catch (e) { message = String(e); }
    feed?.stop();
    feed = null;
    phase = 'off';
    refresh(true).catch((e) => (message = String(e)));
  };

  /** Leave the modal: whatever is still running (the scan, then the analysis) carries on in the background. A note says
   *  so, and says how to tell when it has finished (the indicator at the top right, below). */
  const carryOnInBackground = () => {
    feed?.stop();
    feed = null;
    phase = 'background';
    refresh(true).catch((e) => (message = String(e)));
  };

  /** What the indicator at the top right shows: the scan, a model download or the analysis, whichever is running. */
  // (an analysis that is switched off is not part of the work, whatever its last numbers were)
  const bg = $derived(backgroundProgress({ scanning, scan: { seen, indexed, toIndex }, analysis: analysisOptions.tags ? analysis : null, people: analysisOptions.faces ? peopleState : null, dupes: analysisOptions.dupes ? dupPass(dupState) : null, load: { fraction: settingsView.fraction }, loading: { ...loadState, peak: loadPeak } }));
  const openAnalysisDetails = () => { settingsPage = 'analysis'; pickTab('settings'); };

  // While the analysing screen is showing, count how far its folders' photos have got once a second; while Settings,
  // Picture analysis is showing, count the people found. (The analyses' states arrive as events.)
  $effect(() => {
    const flow = phase === 'scanning' || phase === 'analysing';
    if (!flow && !(tab === 'settings' && settingsPage === 'analysis')) return;
    const tick = async () => {
      try {
        // Counted from the folders' own photos, so duplicates found among them (or hiding older photos elsewhere) cannot
        // throw the totals off.
        if (flow && flowIds.length > 0) flowLoad = await loadProgress(flowIds);
        const o = await faceOverview();
        facePeople = o.groups + o.people;
      } catch { /* the next tick tries again */ }
    };
    void tick();
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  });
  // The step ends when every counter is done (each analysis has counted the photos the scan found, so a stale status cannot
  // end it early). The end screen is prepared first and swapped in only when ready, so it never flashes or blinks.
  $effect(() => {
    // (and everything the walk found has been stored: videos, which are not analysed, are stored by the same pass)
    // (the count must be known: an unmeasured load is not an empty one, or with every analysis off this would end at once)
    if (phase !== 'analysing' || finishing || !flowView.finished || flowLoad === null || flowLoad.pending_videos > 0 || flowLoad.pending > 0) return;
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
    void loadPicks(); // picks in it went with it
    closePopout(); // its photos may be gone
    loadTags();
    void loadUserTags();
    loadPeople();
    folders = await listFolders();
    await loadFolderInfos();
    closeViewer();
    selected = null;
    await refresh(true);
    if (mode === 'map') loadPoints();
  }

  /** The Email and Print buttons in Browse's details: the Tools tab opens at that tool, working on that one photo. */
  let toolsStart = $state<{ photo: Photo | null; page: 'details' | 'email' | 'print' | 'tags'; results?: boolean } | null>(null);
  function workOn(page: 'email' | 'print') {
    if (!selectedPhoto) return;
    toolsStart = { photo: selectedPhoto, page };
    tab = 'tools';
  }
  // Opening Tools any other way starts it as usual.
  $effect(() => { if (tab !== 'tools') toolsStart = null; });

  const askValues = $derived<SettingValues>({ viewerMode, emailLimitMb, recentDays, networkRescan, analysis: analysisOptions, trips: tripPrefs });
  const askHandlers: Handlers = {
    viewer: (m) => setViewerMode(m),
    emailLimit: (mb) => setEmailLimit(mb),
    recentDays: (n) => setRecentDays(n),
    networkRescan: (r) => setNetworkRescanPref(r),
    analysis: (o) => void applyAnalysisOptions(o),
    trips: (p) => setTripPrefs(p),
  };
  // What Ask Pixie last found (listed in Tools, Working on), and what its buttons do with it.
  let askResults = $state<number[]>([]);
  const askKnown = $derived({ tags: userTagList.map((t) => t.name), people: peopleList.map((p) => ({ id: p.id, name: p.name })) });
  /** What Ask Pixie may be told about the library at this moment: nothing unless the person allowed it. */
  const askLibraryNow = () => (askLibraryOn ? buildLibrary({ total: folderInfos.reduce((n, f) => n + f.count, 0), picked: picked.size, tags: userTagList, people: peopleList, trips, destinations }) : null);
  async function askPick(ids: number[]) {
    const fresh = ids.filter((id) => !picked.has(id));
    if (fresh.length === 0) return;
    picked = new Set([...picked, ...fresh]);
    try { await pickMany(fresh); } catch (e) { await loadPicks(); throw e; }
    if (activeUtag === 'picked') void utagChanged();
  }
  async function askTag(ids: number[], tag: string, undo = false): Promise<string> {
    if (undo) { await untagPhotos(ids, tag); await utagChanged(tag); return tag; }
    const stored = await tagPhotos(ids, tag);
    await utagChanged(stored);
    return stored;
  }
  function askOpen(tool: 'details' | 'email' | 'print') {
    toolsStart = { photo: null, page: tool, results: true };
    pickTab('tools');
  }
  /** A button Ask Pixie offered: go to that tab, Settings page or tool. */
  function askGo(target: string) {
    const place = askPlace(target);
    if (!place) return;
    if (place.settings) settingsPage = place.settings;
    toolsStart = place.tools ? { photo: null, page: place.tools } : null;
    pickTab(place.tab);
  }

  // Details for the chosen photo (fetched by index in the current view).
  $effect(() => {
    const i = selected;
    const { order, folderId, kind, bounds, maxId, since, until, search } = shown;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId, kind, bounds, maxId, since, until, search).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
    return () => { live = false; };
  });

  onMount(() => {
    refresh().catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints(); // the map was left on
    loadFolderInfos();
    void loadPicks();
    loadStatus().then(setLoad).catch(() => {});
    getAnalysisOptions().then((o) => { analysisOptions = parseOptions(o); welcomeOptions = { ...analysisOptions }; }).catch(() => {});
    void loadTrips();
    analysisStatus().then((s) => (analysis = s)).catch(() => {});
    peopleStatus().then((s) => (peopleState = s)).catch(() => {});
    duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {});
    loadTags();
    void loadUserTags();
    loadPeople();
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) { void openWelcome(); return; }
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      listen<LoadStatus>('load-status', (e) => setLoad(e.payload)),
      listen<AnalysisStatus>('analysis-status', (e) => { analysis = e.payload; loadTagsSoon(); }),
      listen<DupStatus>('duplicate-status', (e) => { dupState = e.payload; dupHiddenChanged(); }),
      listen<PeopleStatus>('people-status', (e) => { peopleState = e.payload; loadPeopleSoon(); }),
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number; indexed: number; to_index: number; folder_id: number; latest: Sample | null }>('scan-progress', (e) => {
        seen = e.payload.seen; indexed = e.payload.indexed; toIndex = e.payload.to_index; scanFolder = e.payload.folder_id;
        scanning = true; // a scan the app started by itself (a watched folder changed, the timer) shows too, and can be stopped
        if (phase !== 'scanning') { if (phase === 'off' || phase === 'background') refreshLive(); return; }
        const f = folders.find((x) => x.id === e.payload.folder_id);
        flowFolder = f ? folderName(f.path) : flowFolder;
        if (e.payload.latest) feed?.offer(e.payload.latest);
      }),
      // A photo the analysis has just finished goes by on the loading screen's strip; with the library showing, the grid
      // follows as photos are stored.
      listen<Sample>('analysis-latest', (e) => {
        if (phase === 'scanning' || phase === 'analysing') feed?.offer(e.payload);
        else if (phase === 'off' || phase === 'background') refreshLive();
      }),
      listen<{ folders: number; found: number }>('scan-done', (e) => {
        loadTags();
        loadPeople();
        scanning = false;
        scanFolder = null;
        loadFolderInfos();
        void loadTrips();
        if (openIndex !== null || soloPhoto !== null) pendingRefresh = true;
        else refresh().catch((e) => (message = String(e)));
        if (mode === 'map') loadPoints();
        void refreshFolderStates();
        if (phase === 'scanning') void finishScanPhase(e.payload.found);
      }),
      listen(POPOUT_READY, () => sendShow(false)),
      listen(POPOUT_CLOSED, () => { popoutOpen = false; }),
      listen(POPOUT_MOVED, (e) => {
        const m = parseMoved(e.payload);
        // Only follow while the main window still shows the listing the pop-out is paging through.
        if (!m || m.total !== total || !sameListing(m.view, shown)) return;
        popoutSnap = { index: m.index, total, view: { ...shown } };
        selected = m.index;
      }),
      listen<{ message: string }>('scan-error', (e) => {
        message = e.payload.message; // scan-done always follows
        void refreshFolderStates();
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
      {#if message}<span class="err">{message}</span>{/if}
    {/snippet}
    <div class="browse" inert={tab === 'settings' || tab === 'faces' || tab === 'dupes' || tab === 'tools' || tab === 'ask'}>
      <div class="topbars">
      <div class="tools">
        <span class="count">{kindWord(view.kind, total)}{areaOnly ? ' in the map area' : ''}</span>
        {#if activeTag !== null}
          <span class="filter">Auto tagged “{tagLabel(activeTag)}”
            <button onclick={() => setTag(null)} aria-label="Clear tag" title="Clear tag">✕</button></span>
        {/if}
        {#if activeUtag !== null && activeUtag !== 'picked'}
          <span class="filter">Tagged “{activeUtag.replace(/^tag:/, '')}”
            <button onclick={() => setUtag(null)} aria-label="Clear tag" title="Clear tag">✕</button></span>
        {/if}
        {#if activeInfo !== null}
          <span class="filter">Person “{personLabel(activeInfo)}”
            <button onclick={() => setPerson(null)} aria-label="Clear person" title="Clear person">✕</button></span>
        {/if}
        {#if activeCamera !== null}
          <span class="filter">Camera “{activeCamera}”
            <button onclick={() => setCamera(null)} aria-label="Clear camera" title="Clear camera">✕</button></span>
        {/if}
        {#if view.kind !== null}
          <span class="filter">{view.kind === 'photo' ? 'Photos only' : 'Videos only'}
            <button onclick={() => setKind(null)} aria-label="Show photos and videos" title="Show photos and videos">✕</button></span>
        {/if}
        {#if areaOnly}
          <span class="filter">In the map area
            <button onclick={() => setAreaOnly(false)} aria-label="Show everything, not just the map area" title="Show everything, not just the map area">✕</button></span>
        {/if}
        {#if view.folderId !== null}
          <span class="filter">Folder “{folderInfos.find((f) => f.id === view.folderId) ? folderName(folderInfos.find((f) => f.id === view.folderId)!.path) : 'unknown'}”
            <button onclick={() => setFolder(null)} aria-label="Clear folder" title="Clear folder">✕</button></span>
        {/if}
        {#if tripKey !== null}
          <span class="filter">Trip “{trips.find((t) => t.since === tripKey) ? tripTitle(trips.find((t) => t.since === tripKey)!) : 'unknown'}”
            <button onclick={() => setTrip(null)} aria-label="Clear trip" title="Clear trip">✕</button></span>
        {/if}
        {#if recentOn}
          <span class="filter">{recentLabel(recentDays)}
            <button onclick={() => setRecent(false)} aria-label="Show all dates" title="Show all dates">✕</button></span>
        {/if}
        {#if periodKey !== null}
          <span class="filter">Showing only {periodLabel(periodKey)}
            <button onclick={() => setPeriod(periodKey!)} aria-label="Show all dates" title="Show all dates">✕</button></span>
        {/if}
        {#if offerClearAll(activeNames)}<button class="linkbtn" onclick={showAll} title={clearAllTitle(activeNames)}>Clear all</button>{/if}
      </div>
      <div class="tools second">
        <!-- Picked: always here. Pressing it shows only the picked photos (and again, all); Clear takes everything off Picked. -->
        <span class="pickedbox">
          <button class="pickedbtn" class:on={activeUtag === 'picked'} aria-pressed={activeUtag === 'picked'} onclick={() => setUtag(activeUtag === 'picked' ? null : 'picked')}
                  title={activeUtag === 'picked' ? 'Show all photos again' : 'Show only the picked photos'}>★ Picked {picked.size.toLocaleString()}</button>
          <button class="linkbtn" onclick={clearAllPicks} disabled={picked.size === 0} title="Take everything off Picked (the photos stay in the library)">Clear</button>
        </span>
        <div class="viewctl">
          <label class="check" title="Show a map above the photos">
            <input type="checkbox" checked={mode === 'map'} onchange={(e) => setMapShown(e.currentTarget.checked)} /> Map
          </label>
          <!-- Only while the map is on; then the grid follows the part of the map in view, and a pill says so. -->
          <label class="check" class:off={mode !== 'map'} title={mode === 'map' ? 'Show only the photos in the part of the map you can see' : 'Turn the map on to filter by it'}>
            <input type="checkbox" checked={areaOnly} disabled={mode !== 'map'} onchange={(e) => setAreaOnly(e.currentTarget.checked)} /> Filter by map
          </label>
        </div>
      </div>
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
          <Nav groups={navGroups} kind={view.kind} recent={recentOn}
               period={periodKey} recentText={recentLabel(recentDays)}
               {facets} {tags} tag={activeTag} ontag={setTag} userTags={userTagList} utag={activeUtag} onutag={setUtag}
               people={peopleList} facesFound={(peopleState?.done ?? 0) > 0} person={activePerson} onperson={setPerson}
               cameras={facets?.cameras ?? []} camera={activeCamera} oncamera={setCamera} analysisOn={analysisOptions} onopenanalysis={openAnalysisDetails}
               folders={folderInfos} folderStates={folderStateMap} folderId={view.folderId} onfolder={setFolder}
               {trips} tripsReady={tripPrefs.confirmed} trip={tripKey} ontrip={setTrip} {destinations} onvisibletrips={(l) => void resolveDestinations(l)} onrenametrip={renameTripTo} onsettrips={openTripSettings} onnamepeople={() => pickTab('faces')}
               {collapsed} ontoggle={toggleNav}
               onperiod={setPeriod} onkind={setKind} onrecent={setRecent} />
        {/snippet}
        {#snippet center()}
          {#if mode === 'map'}
            <div class="mapsplit">
              <div class="maptop">
                <PhotoMap {points} {fitMap} initial={mapStart} route={tripKey !== null ? routeData : null} stopNames={routeNames}
                          onbounds={setBounds}
                          onpin={focusPhoto}
                          onopenphoto={(p) => (soloPhoto = p)}
                          onmove={(v) => { lastMap = v; savePref('mapView', v); }} />
              </div>
              <div class="mapbottom">
                {#if total === 0 && !scanning}
                  <p class="empty small">{areaOnly ? 'No photos or videos in this part of the map.' : emptyMessage}</p>
                {:else}
                  <Grid {groups} {version} view={shown} {selected} {scrollTo} picks={picked} onpick={togglePick}
                        onopen={openItem}
                        onselect={selectItem} />
                {/if}
              </div>
            </div>
          {:else if total === 0 && !scanning}
            <p class="empty">{emptyMessage}</p>
          {:else}
            <Grid {groups} {version} view={shown} {selected} {scrollTo} picks={picked} onpick={togglePick}
                  onopen={openItem}
                  onselect={selectItem} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} onemail={() => workOn('email')} onprint={() => workOn('print')} ontagschange={(t) => void utagChanged(t)} />
        {/snippet}
      </Shell>
        {/if}
      </div>
    </div>
    {#if peopleTabBuilt}
      <div class="page-over" hidden={tab !== 'faces'}>
        <PeopleTab active={tab === 'faces'} status={peopleState} onchange={peopleTabChanged} />
      </div>
    {/if}
    {#if tab === 'tools'}
      <div class="page-over">
        <ToolsTab active={tab === 'tools'} start={toolsStart} results={askResults} picks={picked} onunpick={togglePick} onclear={clearAllPicks} {emailLimitMb} userTags={userTagList} onchange={(t) => void utagChanged(t)} />
      </div>
    {/if}
    {#if tab === 'dupes'}
      <div class="page-over">
        <DuplicatesTab active={tab === 'dupes'} onchange={() => { duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {}); refresh().catch((e) => (message = String(e))); loadFolderInfos(); }} />
      </div>
    {/if}
    {#if askBuilt}
      <div class="page-over" hidden={tab !== 'ask'}>
        <AskTab on={askOn && typeof askInstall === 'string'} installId={typeof askInstall === 'string' ? askInstall : ''} values={askValues} handlers={askHandlers} from={askFrom} language={navigator.language} known={askKnown} getLibrary={askLibraryNow} onresults={(ids) => (askResults = ids)} onopen={askOpen} onpick={askPick} ontag={askTag}
                ongo={askGo} onturnon={() => setAskOn(true)} ondecline={() => pickTab('browse')} onsettings={() => { settingsPage = 'ask'; pickTab('settings'); }} />
      </div>
    {/if}
    {#if tab === 'settings'}
      <div class="page-over">
        <Settings askOn={askOn} onask={setAskOn} askLibrary={askLibraryOn} onasklibrary={setAskLibrary} loading={loadState} analysisOptions={analysisOptions} onanalysisoptions={applyAnalysisOptions} emailLimitMb={emailLimitMb} onemaillimit={setEmailLimit} folders={folderInfos} folderStates={folderStateMap} analysing={trackActive ? { ids: trackIds, fraction: trackView.fraction } : null} {scanning} {scanFolder} scan={{ seen, indexed, toIndex }} onstop={() => cancelScan()} onadd={chooseFolder} onremove={removeFolderAndRefresh}
                  {networkRescan} onnetworkrescan={setNetworkRescanPref}
                  page={settingsPage} onpage={(p) => (settingsPage = p)} {tripPrefs} ontripprefs={setTripPrefs} {viewerMode} onviewer={setViewerMode} {recentDays} onrecentdays={setRecentDays}
                  analysis={settingsView} analysisNote={trackActive ? `Showing the folders you just added: ${trackNames}` : null} onanalysispause={(p) => analysisSetPaused(p)} onanalysisretry={() => analysisRetry()} />
      </div>
    {/if}
  </Tabs>
  {#if openIndex !== null || soloPhoto !== null}
    <Viewer index={openIndex ?? 0} {total} view={shown} solo={soloPhoto} onclose={closeViewer} onnav={(i) => (openIndex = i)}
            onpopout={() => popOut(openIndex ?? 0)} />
  {/if}
</main>

{#if phase === 'welcome'}
  <Welcome rows={pickerRows} searching={sniffing} {looking} ontoggle={(p) => (rows = toggle(rows, p))} onadd={welcomeAdd}
           onskip={() => { stopWelcomeSubs(); phase = 'off'; }} onstart={welcomeNext}
           adding={addMode} oncancel={cancelAdd} />
{:else if phase === 'options'}
  <AnalysisStep options={welcomeOptions} onoptions={(o) => (welcomeOptions = o)} onback={() => (phase = 'welcome')} onstart={optionsStart} />
{:else if phase === 'scanning' || phase === 'analysing'}
  <LoadModal {phase} scan={{ seen, indexed, toIndex }} folder={flowFolder} {strip} view={flowViewShown} notice={message}
             onstop={stopLoad} onbackground={carryOnInBackground}
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
  /* Plain buttons look the same on every system (the Mac's own are tiny); any style written for a button wins over this. */
  :global(:where(button)) { font: inherit; font-size: 14px; padding: 5px 12px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; cursor: pointer; }
  :global(:where(button:disabled)) { opacity: .5; cursor: default; }
  :global(:where(select)) { font: inherit; font-size: 14px; padding: 4px 8px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; }
  :global(:where(select:disabled)) { opacity: .5; }
  :global(:where(input[type='checkbox'], input[type='radio'])) { width: 16px; height: 16px; vertical-align: -3px; }
  main { display: flex; flex-direction: column; height: 100vh; }
  .browse { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .topbars { border-bottom: 1px solid #eee; }
  .tools { display: flex; gap: 12px; align-items: center; padding: 6px 14px; }
  .tools.second { padding-top: 0; }
  .content { flex: 1; min-height: 0; }
  .count { font-size: 13px; font-weight: 600; color: #333; font-variant-numeric: tabular-nums; }
  .filter { display: inline-flex; align-items: center; gap: 6px; padding: 2px 4px 2px 10px; border-radius: 12px; background: #e3ecff; color: #1a3f9a; font-size: 13px; }
  .filter button { border: 0; background: none; color: inherit; font: inherit; cursor: pointer; padding: 0 6px; border-radius: 10px; }
  /* A quiet link, not a button: it sits in the row of pills without looking like one of them. */
  .pickedbox { display: inline-flex; align-items: center; gap: 2px; }
  .pickedbtn { padding: 2px 10px; border-radius: 12px; font-size: 13px; font-variant-numeric: tabular-nums; }
  .pickedbtn.on { background: #e3ecff; border-color: #2f6fed; color: #1a3f9a; }
  .linkbtn:disabled { opacity: .5; cursor: default; text-decoration: none; }
  .linkbtn { border: 0; background: none; padding: 2px 4px; font: inherit; font-size: 13px; color: #1a3f9a; cursor: pointer; }
  .linkbtn:hover { text-decoration: underline; }
  .viewctl { margin-left: auto; display: flex; gap: 16px; align-items: center; }
  /* Two independent switches: the map, and whether the grid follows it. */
  .check { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: #333; cursor: pointer; }
  .check.off { color: #999; cursor: default; }
  .check input { margin: 0; }
  .filter button:hover { background: rgba(47, 111, 237, .2); }
  .page-over { position: absolute; inset: 0; z-index: 5; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; }
  .mapbottom { flex: 1; min-height: 0; }
  .empty.small { margin-top: 24px; }
</style>
