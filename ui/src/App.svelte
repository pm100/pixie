<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { emit, listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    activitySnapshot, onActivity, analysisRetry, analysisSetPaused, analysisStatus, cancelScan, countPhotos, getMaxPhotoId, photoPosition, openViewerWindow, listDayGroups, listFolderInfo, listFolders, listGeoPoints, listPhotos,
    listTags, removeFolder, setTagFilter, setUserTagFilter, userTags as fetchUserTags, type UserTagCount, startScan, setNetworkRescan, listTrips, renameTrip, tripRoute, placeFor,
    listPeople, faceOverview, peopleStatus, duplicateStatus, setPersonFilter, setCameraFilter, setMissingFilter, getMasterFolder, setMasterFolder, getSyncSubfolder, setSyncSubfolder, setFolderFeeds, shareCounts as fetchShareCounts, setShareFilter, copyToMaster, stopMasterCopy, masterCopyStatus, locationCounts, getDateRules, setDateRules, activeFilters, pickIds, pickAllMatching, pickVideoCount, pickPhoto, pickMany, clearPicks, getAnalysisOptions, setAnalysisOptions,
    hiddenCount, hidePhotos, setShowHidden, unhidePhotos, openDataFolder, openInDefaultPlayer, showInFolder, copyToClipboard, makePrivate, releasePrivate, photoShare, hiddenAmong,
    facetCounts, onSuggestionCount, onSuggestionFound, onSuggestionLooking, onSuggestionsDone, loadProgress, scanFolders, suggestFolders, folderStates,
    type DayGroup, type Folder, type FolderInfo, type MediaKind, type Photo, type Route, type SortOrder, type Trip, type View,
  } from './lib/api';
  import type { AnalysisStatus } from './lib/analysis';
  import { toStates, type FolderStates } from './lib/folderState';
  import { cleanCollapsed, toggleSection, type SectionId } from './lib/sections';
  import { cleanTripPrefs, destinationOf, tripTitle, type TripPrefs } from './lib/trips';
  import { createLatest } from './lib/latest';
  import { hitsActive, tagLabel, type TagCount } from './lib/tagNav';
  import { SHARE_PRIVATE, shouldRemind, type CopyStatus, type ShareChoice } from './lib/master';
  import { personLabel, type PeopleStatus, type PersonInfo } from './lib/people';
  import PeopleTab from './lib/PeopleTab.svelte';
  import { directCount, parseBar } from './lib/actionBar';
  import PhotoActions from './lib/PhotoActions.svelte';
  import type { DupStatus } from './lib/dupes';
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
  import { shouldFit, type MapChange } from './lib/mapFit';
  import { DEFAULT_DATE_RULES, type DateRules } from './lib/dateRules';
  import { missingFromMap } from './lib/mapNote';
  import AllSet from './lib/AllSet.svelte';
  import BackgroundNote from './lib/BackgroundNote.svelte';
  import { view as activityView, type ActivitySnapshot } from './lib/activity';
  import ProcessingDialog from './lib/ProcessingDialog.svelte';
  import ProcessingPill from './lib/ProcessingPill.svelte';
  import { pauseActions } from './lib/scanPause';
  import LoadModal from './lib/LoadModal.svelte';
  import Welcome from './lib/Welcome.svelte';
  import AppIcon from './lib/AppIcon.svelte';
  import { allSetSummary, phaseAfterScan, type Phase } from './lib/loadFlow';
  import { analysisView, type LoadProgress } from './lib/analysisView';
  import { indexFacets, type Facets } from './lib/facets';
  import { createStripFeed, type StripItem } from './lib/stripFeed';
  import { thumbUrl } from './lib/url';
  import { addFound, applyCount, chooseByHand, notInLibrary, tickedPaths, toRows, toggle, type Row } from './lib/suggestions';
  import { run } from './lib/selection';
  import Settings, { type SettingsPage } from './lib/Settings.svelte';
  import Shell from './lib/Shell.svelte';
  import { pickAllState } from './lib/pickAll';
  import Tabs from './lib/Tabs.svelte';
  import { visibleTabs, type TabId } from './lib/tabs';
  import ToolsTab from './lib/ToolsTab.svelte';
  import { isToolGroup, toolById } from './lib/tools';
  import { listingAffected } from './lib/userTags';
  import AnalysisStep from './lib/AnalysisStep.svelte';
  import AskPanel from './lib/AskPanel.svelte';
  import { newInstallId, photoState } from './lib/ask';
  import { placeFor as askPlace } from './lib/askGoTo';
  import type { Handlers, SettingValues } from './lib/askSettings';
  import AskCard from './lib/AskCard.svelte';
  import { ALL_ON, clearsFilters, parseOptions, type AnalysisOptions } from './lib/analysisOptions';
  import { parseLimit } from './lib/emailSize';
  import { parseViewerMode, type ViewerMode } from './lib/viewerPref';
  import { allCommands, contextModel, menuModel, onCommandsChanged } from './lib/commands';
  import { registerAppCommands, registerGoCommands, type CommandContext } from './lib/appCommands';
  import { commandForKey, isTextField } from './lib/shortcuts';
  import { applyMenu, popupMenu, toSpec } from './lib/menuBar';
  import { registerPhotoCommands, type PhotoCommandContext, type PhotoTarget } from './lib/photoCommands';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Nav from './lib/Nav.svelte';
  import Viewer from './lib/Viewer.svelte';
  import SyncReminder from './lib/SyncReminder.svelte';

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
  /** The first listing and the list of folders have both come back. Until then nothing is known about the library, so a splash covers the window and "add a folder" is not offered. */
  let started = $state(false);
  /** The photo chosen in the grid (its position in the listing): its details show on the right. */
  let selected = $state<number | null>(null);
  let selectedPhoto = $state<Photo | null>(null);
  let scrollTo = $state<{ group: number; n: number } | null>(null);
  let jumps = 0;
  let scanning = $state(false), message = $state('');
  let scanFolder = $state<number | null>(null); // the folder the running scan is on (null: none yet)
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

  /** Which dates count as missing, and when the file's date stands in (Settings, Dates). A change is applied to the whole library at once. */
  let dateRules = $state<DateRules>(DEFAULT_DATE_RULES);
  async function changeDateRules(next: DateRules) {
    dateRules = next;
    try { await setDateRules(next); } catch (e) { message = String(e); return; }
    // Dates moved: the listing, the date tree, the trips and the counts all follow them.
    await refresh(true).catch((e) => (message = String(e)));
    void loadTrips();
    refreshLocations();
    version++;
  }
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
      if (mode === 'map') { askFit('filter'); loadPoints(); }
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
      const changed = f.tag !== activeTag || f.person !== activePerson || f.camera !== activeCamera || f.utag !== activeUtag || f.missing !== activeMissing || f.share !== activeShare;
      activeTag = f.tag;
      activeUtag = f.utag;
      activePerson = f.person;
      activeCamera = f.camera;
      activeMissing = f.missing;
      activeShare = f.share as ShareChoice | null;
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
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
  }
  /** Picks or tags changed: read the tag list again, and if Picked or a tag is chosen, what it lists changed too (a tag
   *  that lost its last photo is gone from the library, so the chip goes with it). */
  async function utagChanged(touched?: string) {
    await loadUserTags();
    if (activeUtag === null) return;
    const before = activeUtag;
    await syncFilters();
    if (!listingAffected(before, touched, activeUtag === before)) return; // the listing is as it was: keep what is selected
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    refresh().catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }

  async function setTag(next: string | null) {
    try { await setTagFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(next, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
  }

  // People: faces grouped by who they are, as a filter. It combines with a tag.
  let peopleList = $state<PersonInfo[]>([]), activePerson = $state<number | null>(null);
  /** The camera chosen in the tree, by its tidied name ("Pixel 7"). */
  let activeCamera = $state<string | null>(null);
  /** The kind of missing data chosen in the tree (`location`: only items with no position). */
  let activeMissing = $state<string | null>(null);

  async function setMissing(next: string | null) {
    try { await setMissingFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    // An item with no position can never be inside a map area, so the two cannot be on together.
    if (next !== null && areaOnly) { areaOnly = false; pendingBounds = null; }
    view = { ...view, bounds: next !== null ? null : view.bounds, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
  }

  // The sync folder: none until one is chosen, and nothing of the feature shows without one. `activeShare` is the New / Private choice in the tree.
  let syncSub = $state('');
  let master = $state<string | null>(null), shareState = $state({ unsynced: 0, private: 0 }), activeShare = $state<ShareChoice | null>(null);
  // The copy of the items not yet synced into the sync folder: what waits and how the last run went, and how far a running one has got.
  let copyStatus = $state<CopyStatus | null>(null);
  /** The sync folder and the counts have been read at least once. */
  let shareLoaded = $state(false);
  async function loadShare() {
    try {
      master = await getMasterFolder();
      syncSub = (await getSyncSubfolder()) ?? '';
      shareState = master === null ? { unsynced: 0, private: 0 } : await fetchShareCounts();
      copyStatus = master === null ? null : await masterCopyStatus();
      if (master === null && activeShare !== null) await setShare(null);
    } catch { /* the entries stay as they were */ }
    shareLoaded = true;
  }
  async function setShare(next: ShareChoice | null) {
    try { await setShareFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
  }
  /** Items changed hands (copied, made private, a folder ticked): the counts and the listing are read again. */
  async function shareChanged() {
    await syncFilters();
    await loadShare();
    view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints();
  }
  /** One photo was made private or taken out of private in the details panel: counts and listing are read again, and the
   * selection stays where it is (in New the photo leaves the list and the next one takes its place, for stepping through). */
  async function sharePhotoChanged() {
    await syncFilters();
    await loadShare();
    refresh().catch((e) => (message = String(e)));
  }
  // The start-up reminder about new items to sync: looked at once per start, a few seconds after the window is up, and never again
  // that session (a count that grows later, from ticking a folder or from a scan, does not bring it up). It can be turned off (kept
  // on this computer).
  let remindSync = $state(loadPref<unknown>('syncRemind', true) !== false);
  let reminderChecked = false;
  let reminder = $state(false);
  function setRemindSync(on: boolean) {
    remindSync = on;
    savePref('syncRemind', on);
  }
  // Once the window is up, the counts are read and nothing is being scanned, so that the count has settled (hidden copies are left out of it).
  $effect(() => {
    if (reminderChecked || phase !== 'off' || scanning || !shareLoaded) return;
    const t = setTimeout(() => {
      reminderChecked = true;
      reminder = shouldRemind(master, shareState.unsynced, remindSync);
    }, 4000);
    return () => clearTimeout(t);
  });
  function closeReminder(dontShow: boolean) {
    reminder = false;
    if (dontShow) setRemindSync(false);
  }
  async function reviewNew(dontShow: boolean) {
    closeReminder(dontShow);
    pickTab('browse');
    await setShare('new');
  }
  async function syncFromReminder(dontShow: boolean) {
    closeReminder(dontShow);
    await copyNow();
  }
  // The checker finds items already in the sync folder (by their hashes) as it works: the New count follows, in batches.
  let syncCountTimer: ReturnType<typeof setTimeout> | undefined;
  function syncCountSoon() {
    if (master === null) return;
    clearTimeout(syncCountTimer);
    syncCountTimer = setTimeout(() => void loadShare(), 2000);
  }
  async function copyNow() {
    syncPaused = false; // (a sync started by hand, or the reminder, is no longer waiting to be resumed)
    try { await copyToMaster(); } catch (e) { message = String(e); return; }
    await loadShare();
  }
  /** The master is one of the watched folders (or none): nothing is scanned, the folder is already known. */
  async function chooseMaster(path: string | null) {
    try { await setMasterFolder(path); } catch (e) { message = String(e); return; }
    await shareChanged();
    await loadFolderInfos();
  }
  /** The folder inside the sync folder that copies go into; a name that cannot be used rejects, for the tool to show. */
  async function chooseSyncSub(name: string) {
    await setSyncSubfolder(name);
    await loadShare();
    await shareChanged();
  }
  async function feedsMaster(id: number, feeds: boolean) {
    try { await setFolderFeeds(id, feeds); } catch (e) { message = String(e); }
    await shareChanged(); // what is in the folder is held (or let go) at once
    await loadFolderInfos();
  }

  async function setCamera(next: string | null) {
    try { await setCameraFilter(next); } catch (e) { message = String(e); return; }
    await syncFilters();
    view = { ...view, search: hitsActive(activeTag, activePerson, next, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
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
      if (await refreshLive()) { dupHiddenShown = want; void loadShare(); } // the New count leaves out hidden copies too
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
      view = { ...view, search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare) };
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
    view = { ...view, search: hitsActive(activeTag, next, activeCamera, activeUtag, activeMissing, activeShare) };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('filter'); loadPoints(); }
  }

  /** The People tab changed someone: refresh the list, and re-take the photos of a chosen person (they may have changed). */
  async function peopleTabChanged() {
    await loadPeople();
    if (activePerson !== null) await setPerson(activePerson);
  }

  let settingsPage = $state<SettingsPage>('folders');
  let folderInfos = $state<FolderInfo[]>([]);
  // Quick pick: the photos picked with the marker on the grid (by id, kept in the library); the Send, Create and Organize tabs work on them.
  let picked = $state<ReadonlySet<number>>(new Set());
  // Hidden photos: left out of Browse (and People, Trips, the tools) until Show hidden is on. Group by date is remembered.
  let groupByDate = $state(loadPref<unknown>('groupByDate', true) !== false);
  function setGroupByDate(on: boolean) { groupByDate = on; savePref('groupByDate', on); }
  let showHidden = $state(false);
  let hiddenTotal = $state(0);
  async function loadHiddenTotal() {
    try { hiddenTotal = await hiddenCount(); } catch { /* the next time */ }
  }
  async function setShowHiddenOn(on: boolean) {
    try { await setShowHidden(on); } catch (e) { message = String(e); return; }
    showHidden = on;
    selected = null;
    refresh().catch((e) => (message = String(e)));
  }
  /** Photos were hidden or shown again: the counts, Picked and the listing are read again. */
  async function hiddenChanged() {
    await Promise.all([loadHiddenTotal(), loadPicks()]);
    if (activeUtag === 'picked') void utagChanged(); else void loadUserTags();
    refresh().catch((e) => (message = String(e)));
  }
  async function hidePicked() {
    try { await hidePhotos([...picked]); } catch (e) { message = String(e); }
    await hiddenChanged();
  }
  async function unhidePicked() {
    try { await unhidePhotos([...picked]); } catch (e) { message = String(e); }
    await hiddenChanged();
  }
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
  /** Pick every photo the filters show, however many, to the ones picked now (done in the library in one go). */
  const pickAll = $derived(pickAllState(total, activeUtag === 'picked', picked.size));
  async function pickAllShown() {
    if (!pickAll.enabled) return;
    const v = shown; // the listing the button refers to
    try {
      await pickAllMatching(v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search);
    } catch (e) {
      message = String(e);
    }
    await loadPicks();
    if (activeUtag === 'picked') void utagChanged(); else void loadUserTags();
  }
  async function clearAllPicks() {
    picked = new Set();
    try { await clearPicks(); } catch (e) { message = String(e); await loadPicks(); }
    if (activeUtag === 'picked') void utagChanged(); else void loadUserTags();
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
  }
  // A tab whose analysis was switched off is gone: leave it.
  $effect(() => { if (tab === 'faces' && !analysisOptions.faces) tab = 'browse'; });
  const tabs = $derived(visibleTabs({ people: analysisOptions.faces }));

  // Ask Pixie is on unless turned off (Settings, Ask Pixie). It sends the typed questions to a server, which the panel and that page say.
  let askOn = $state(loadPref<unknown>('askOn', true) !== false);
  /** The random id the server counts the day's questions by; made the first time it is needed. */
  let askInstall = $state(loadPref<unknown>('askInstall', null));
  function setAskOn(on: boolean) {
    askOn = on;
    savePref('askOn', on);
  }
  $effect(() => {
    if (askOn && typeof askInstall !== 'string') { askInstall = newInstallId(); savePref('askInstall', askInstall); }
  });
  /** The Ask Pixie panel, docked at the right: opened by the ? button and F1, built on first use and then kept, so the conversation survives closing it. */
  let helpOpen = $state(false);
  let helpBuilt = $state(false);
  /** The ? button, which the card hangs from. */
  let helpEl = $state<HTMLElement | null>(null);
  function toggleHelp() {
    helpOpen = !helpOpen;
    if (helpOpen) helpBuilt = true;
  }

  async function loadFolderInfos() {
    try { folderInfos = await listFolderInfo(); } catch (e) { message = String(e); }
    void refreshFolderStates();
    void loadShare();
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
      void loadShare();
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
    if (mode === 'map') { askFit('selection'); loadPoints(); }
  }

  /** Anything narrowing the photos shown (the library choice, folder and kind, are not counted). */

  /** "Show all": drop every filter (tag, person, folder, trip, dates and media type) in one go. */
  async function showAll() {
    try {
      await Promise.all([setTagFilter(null), setPersonFilter(null), setCameraFilter(null), setMissingFilter(null), setShareFilter(null), setUserTagFilter(null)]);
    } catch (e) { message = String(e); }
    await syncFilters();
    periodKey = null;
    tripKey = null;
    areaOnly = false;
    view = { ...view, order: 'newest', search: hitsActive(activeTag, activePerson, activeCamera, activeUtag, activeMissing, activeShare), since: null, until: null, folderId: null, kind: null, bounds: null };
    savePref('kind', null);
    selected = null;
    await refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('selection'); loadPoints(); }
  }

  /** The "Show in Browse" button on a saved video frame: close the viewer, go to Browse, drop the filters if one hides the frame, and select it. */
  async function showFrame(id: number) {
    closeViewer();
    pickTab('browse');
    void getCurrentWindow().setFocus();
    let at = await photoPosition(id, shown).catch(() => null);
    if (at === null) {
      await showAll();
      at = await photoPosition(id, shown).catch(() => null);
    }
    if (at === null) return;
    selected = at;
    reveal = { index: at, n: ++reveals };
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
    if (mode === 'map') { askFit('selection'); loadPoints(); }
  }

  function setKind(kind: MediaKind | null) {
    if (kind === view.kind) return;
    view = { ...view, kind };
    savePref('kind', kind);
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('selection'); loadPoints(); }
  }

  /** Show only one photo folder (the same folder again clears it). */
  function setFolder(id: number | null) {
    if (id === view.folderId) return;
    view = { ...view, folderId: id };
    selected = null;
    refresh(true).catch((e) => (message = String(e)));
    if (mode === 'map') { askFit('selection'); loadPoints(); }
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
    if (mode === 'map') { askFit('selection'); loadPoints(); }
  }

  // Trips: runs of days away from home. Home is the user's to confirm (in Settings), so none are shown before that.
  let tripPrefs = $state<TripPrefs>(cleanTripPrefs(loadPref<unknown>('trips', null)));
  let trips = $state<Trip[]>([]);

  /** What the grid says when nothing matches. */
  const emptyMessage = $derived(view.search ? (activeShare === 'new' ? 'Nothing here is waiting to be synced.' : activeShare === 'private' ? 'Nothing here is private.' : activeMissing !== null ? 'No photos or videos here are missing a location.' : activeUtag === 'picked' ? 'Nothing is picked.' : activeTag === null && activeUtag === null ? 'No photos of this person here.' : 'No photos with this tag here.') : periodKey !== null ? 'No photos or videos from this period here.' : tripKey !== null ? 'No photos or videos from this trip here.' : view.since !== null ? `No photos or videos in the ${recentLabel(recentDays).toLowerCase()}.` : `No ${view.kind === 'video' ? 'videos' : view.kind === 'photo' ? 'photos' : 'photos or videos'} here yet.`);

  /** The filters in force, as the pills read (the date is the chosen period or Last 30 days). */
  const activeNames = $derived(filterNames({
    tag: activeTag === null ? null : tagLabel(activeTag),
    utag: activeUtag,
    person: activeInfo === null ? null : personLabel(activeInfo),
    camera: activeCamera,
    missing: activeMissing,
    share: activeShare,
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
  /** The user has moved or zoomed the map by hand since it last fitted the photos (PhotoMap says). */
  let mapMoved = false;
  /** After a change in Browse: ask the map to fit the photos, unless the rule in `shouldFit` says to leave it. */
  function askFit(change: MapChange) { if (shouldFit(change, areaOnly, mapMoved)) fitMap++; }
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

  /** Asks the grid to bring a photo into view (see Grid's `reveal`). */
  let reveal = $state<{ index: number; n: number } | null>(null);
  let reveals = 0;
  /** A red dot was clicked: the photo is selected (so the details pane shows it) and the grid scrolls to it. Nothing else changes: the map stays where it is and the grid keeps its filters. */
  async function revealPhoto(p: Photo) {
    if (mode !== 'map') return;
    let at: number | null;
    try {
      at = await photoPosition(p.id, shown);
    } catch (e) {
      message = String(e);
      return;
    }
    if (at === null) return; // not in the listing (a filter hides it)
    selected = at;
    reveal = { index: at, n: ++reveals };
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
  /** How far the analysis has got for the photos of this load's folders (null until first counted). */
  let flowLoad = $state<LoadProgress | null>(null);
  let facePeople = $state(0);
  /** The counts beside the entries of the left-hand tree under the filters in force; null until first counted. */
  let facets = $state<Facets | null>(null);
  const facetsLatest = createLatest();
  /** The line under the map about items it cannot show (none while they are the ones listed). */
  const mapReminder = $derived(activeMissing === null ? missingFromMap(facets?.noLocation ?? 0) : null);
  $effect(() => {
    // Anything that changes what the tree's counts mean: the view's filters, the chosen tag and person, the trips, and a
    // refresh of the listing (a scan added photos, a duplicate was hidden...).
    const v = view, tag = activeTag, person = activePerson, camera = activeCamera, utag = activeUtag, missing = activeMissing, share = activeShare, list = trips, chosenTrip = tripKey, days = recentDays;
    version;
    const token = facetsLatest.next();
    const timer = setTimeout(() => {
      facetCounts(v, tag, person, camera, utag, chosenTrip !== null, list.map((t): [number, number] => [t.since, t.until]), recentSince(Math.floor(Date.now() / 1000), days), missing, share)
        .then((f) => { if (facetsLatest.isCurrent(token)) facets = indexFacets(f, list); })
        .catch(() => { /* the entries keep the counts they came with */ });
    }, 120);
    return () => clearTimeout(timer);
  });
  let allSet = $state<ReturnType<typeof allSetSummary> | null>(null);
  let flowProblems: string[] = []; // folders that could not be read during this flow, for the end screen
  let flowIds: number[] = [];
  let flowStartedAt = 0;
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
  /** How many of the library's items have no location (the Locations line of Settings, Picture analysis). */
  let libLocations = $state<{ without: number; undated: number; total: number } | null>(null);
  const refreshLocations = () => { locationCounts().then((c) => (libLocations = c)).catch(() => { /* the line just stays as it was */ }); };

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

  /** Why the folder just chosen in the picker was not added. */
  let pickNotice = $state<string | null>(null);
  async function welcomeAdd() {
    pickNotice = null;
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const r = chooseByHand(rows, folders.map((f) => f.path), { path: dir, name: folderName(dir), kind: 'known', ticked: true, note: 'chosen by you', included_in: null });
    rows = r.rows;
    pickNotice = r.note;
  }

  /** This flow adds folders later (from Settings), not the first run. */
  let flowAdding = false;
  /** Add these folders and scan them with the modal screens. A folder added while another scan runs joins that scan. */
  async function beginLoad(paths: string[]) {
    stopWelcomeSubs();
    flowAdding = addMode;
    addMode = false;
    message = '';
    flowLoad = null;
    flowStartedAt = Date.now(); flowIds = []; flowPhotos = 0; finishing = false;
    strip = []; allSet = null; flowProblems = [];
    feed?.stop();
    feed = createStripFeed({ load: preload, onShow: (item) => (strip = [...strip.slice(-4), item]), minGapMs: 100 });
    scanning = true; scanFolder = null;
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
    const next = phaseAfterScan({ stopped: false, found, photos: flowPhotos, adding: flowAdding });
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

  /** Leave the modal: whatever is still running (the scan, then the analysis) carries on in the background. A note says
   *  so, and says how to tell when it has finished (the Processing button at the top right, below). */
  const carryOnInBackground = () => {
    feed?.stop();
    feed = null;
    phase = 'background';
    refresh(true).catch((e) => (message = String(e)));
  };

  // Everything running in the background, as one snapshot from the app about once a second; the pill and its screen show it.
  let activity = $state<ActivitySnapshot | null>(null);
  let activityOpen = $state(false);
  const pill = $derived(activityView(activity).pill);
  // The Processing screen shows the pictures going by too: a strip of its own, fed while the screen is open.
  let dialogStrip = $state<Sample[]>([]);
  let dialogFeed: ReturnType<typeof createStripFeed> | null = null;
  $effect(() => {
    if (!activityOpen) return;
    dialogStrip = [];
    const f = createStripFeed({ load: preload, onShow: (item) => (dialogStrip = [...dialogStrip.slice(-4), item]), minGapMs: 100 });
    dialogFeed = f;
    return () => { f.stop(); if (dialogFeed === f) dialogFeed = null; };
  });
  // Pause covers everything. A folder scan it stops is started again by Resume (it only reads what is new or changed).
  let scanPaused = $state(false);
  let cancelling = false; // a pause has stopped the scan and the scan has not ended yet
  function pauseScan() { scanPaused = true; cancelling = true; void cancelScan(); }
  function resumeScan() { scanPaused = false; scanning = true; startScan(null).catch(() => (scanning = false)); }
  let syncPaused = $state(false); // a sync into the sync folder was stopped by Pause and has not been started again
  function pauseResume(paused: boolean) {
    const a = pauseActions(paused, scanning, scanPaused, (copyStatus?.running ?? false) || (activity?.tasks ?? []).some((t) => t.kind === 'copy_to_master' && t.state === 'running'), syncPaused);
    analysisSetPaused(a.analysisPaused).catch((e) => (message = String(e)));
    if (a.cancelScan) pauseScan();
    if (a.startScan) resumeScan();
    if (a.stopSync) { syncPaused = true; void stopMasterCopy(); }
    if (a.startSync) { syncPaused = false; void copyNow(); }
  }
  const openAnalysisDetails = () => { settingsPage = 'analysis'; pickTab('settings'); };

  // The menu bar and the app's own shortcuts are made from the command registry (lib/commands.ts); this is what the commands need from the app.
  const mac = navigator.userAgent.includes('Mac');
  const isPaused = () => { const av = activityView(activity); return av.pill === 'paused' || av.held || scanPaused || syncPaused; };
  /** What Copy copies: the Picked photos, or else the photo chosen. */
  const copyIds = (): number[] => (picked.size > 0 ? [...picked] : selectedPhoto ? [selectedPhoto.id] : []);
  const commandContext: CommandContext = {
    mac,
    tabs: () => tabs,
    pickTab,
    addFolders: chooseFolder,
    rescan: () => { if (scanning) return; scanning = true; startScan(null).catch(() => (scanning = false)); },
    syncNow: () => void copyNow(),
    canSync: () => master !== null,
    paused: isPaused,
    canTogglePause: () => activityView(activity).running.length > 0 || scanPaused || syncPaused,
    togglePause: () => pauseResume(!isPaused()),
    openDataFolder: () => { openDataFolder().catch((e) => (message = String(e))); },
    exit: () => { void getCurrentWindow().close(); },
    pickAllShown: () => void pickAllShown(),
    canPickAll: () => pickAll.enabled,
    clearPicks: () => void clearAllPicks(),
    anyPicked: () => picked.size > 0,
    viewerMode: () => viewerMode,
    setViewerMode,
    showProcessing: () => (activityOpen = true),
    toggleFullScreen: () => { const w = getCurrentWindow(); void w.isFullscreen().then((f) => w.setFullscreen(!f)); },
    toggleHelp,
    showAbout: () => { settingsPage = 'about'; pickTab('settings'); },
    inTextField: () => isTextField(document.activeElement),
    editText: (op) => { document.execCommand(op); },
    copyIds,
    copyImageOk: () => { const ids = copyIds(); return ids.length === 1 && (picked.size === 1 || selectedPhoto?.kind === 'photo'); },
    copyToClipboard: (form) => { const ids = copyIds(); if (ids.length > 0) copyToClipboard(ids, form).catch((e) => (message = String(e))); },
  };
  let commandsVersion = $state(0);
  let commandsChanges = 0; // a plain counter: `commandsVersion++` would read the state, and the effects that register commands would then depend on it and loop
  $effect(() => onCommandsChanged(() => { commandsVersion = ++commandsChanges; }));
  $effect(() => registerAppCommands(commandContext));
  $effect(() => registerGoCommands(commandContext)); // reads the shown tabs, so it follows People turning on or off
  // Rebuild the native menu whenever a command comes or goes, or a label or greyed-out state it read changes. The model is made here,
  // synchronously, so every piece of state the commands read is tracked.
  $effect(() => {
    void commandsVersion;
    const spec = toSpec(menuModel(allCommands()), mac);
    applyMenu(spec, (id) => {
      const c = allCommands().find((x) => x.id === id);
      if (c) Promise.resolve(c.run(undefined)).catch((e) => (message = String(e)));
    }, mac).catch((e) => console.error('The menu bar could not be built', e));
  });
  // Right-click on a photo in Browse. A photo among several Picked acts for all of them; any other photo is chosen first and acts alone.
  async function setPickedIds(ids: number[], on: boolean) {
    const next = new Set(picked);
    for (const id of ids) { if (on) next.add(id); else next.delete(id); }
    picked = next;
    try { if (on) await pickMany(ids); else for (const id of ids) await pickPhoto(id, false); } catch (e) { message = String(e); await loadPicks(); }
    if (activeUtag === 'picked') void utagChanged();
  }
  async function setHiddenIds(ids: number[], on: boolean) {
    try { await (on ? hidePhotos(ids) : unhidePhotos(ids)); } catch (e) { message = String(e); }
    await hiddenChanged();
  }
  const photoCommandContext: PhotoCommandContext = {
    canCopy: !mac, // (a Mac cannot put photos on the clipboard yet)
    copy: (ids, form) => { copyToClipboard(ids, form).catch((e) => (message = String(e))); },
    open: openItem,
    setPicked: (ids, on) => void setPickedIds(ids, on),
    openPlayer: (id) => { openInDefaultPlayer(id).catch((e) => (message = String(e))); },
    showInFolder: (id) => { showInFolder(id).catch((e) => (message = String(e))); },
    openTool,
    setPrivate: async (id, on) => {
      try { await (on ? makePrivate([id]) : releasePrivate([id])); } catch (e) { message = String(e); }
      await sharePhotoChanged();
    },
    setHidden: (ids, on) => void setHiddenIds(ids, on),
  };
  $effect(() => registerPhotoCommands(photoCommandContext));
  async function photoMenu(i: number, e: MouseEvent) {
    e.preventDefault();
    const v = shown; // the listing the position refers to
    try {
      const [p] = await listPhotos(i, 1, v.order, v.folderId, v.kind, v.bounds, v.maxId, v.since, v.until, v.search);
      if (!p) return;
      const group = picked.has(p.id) && picked.size > 1;
      const ids = group ? [...picked] : [p.id];
      if (!group) selected = i;
      const single = ids.length === 1;
      const hidden = await hiddenAmong(ids);
      const target: PhotoTarget = {
        ids, index: single ? i : null, kind: single ? p.kind : 'mixed',
        picked: ids.every((id) => picked.has(id)),
        private: single && master !== null ? (await photoShare(p.id)) === SHARE_PRIVATE : null,
        hidden: hidden.length === ids.length,
      };
      await popupMenu(contextModel(allCommands(), 'photo', target), (id) => {
        const c = allCommands().find((x) => x.id === id);
        if (c) Promise.resolve(c.run(target)).catch((err) => (message = String(err)));
      });
    } catch (err) { message = String(err); }
  }
  function onAppKey(e: KeyboardEvent) {
    const c = commandForKey(allCommands(), e, mac);
    if (!c) return;
    e.preventDefault();
    void c.run(undefined);
  }

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

  /** Which photos Send, Create and Organize work on (Picked, Current or a tag). Shared, so the choice carries from one of those tabs to the next. */
  let workSource = $state('picked');
  /** The tool a tab is to open at (from an Email button, the More menu or Ask Pixie); otherwise a tab opens at the tool it was last left at. */
  let toolStart = $state<string | null>(null);
  /** Open a tool in its tab, working on the picked photos or on the photo chosen here. */
  /** Browse, showing only the items that have no date. */
  function showUndated() {
    pickTab('browse');
    if (periodKey !== 'undated') setPeriod('undated');
  }
  function openTool(toolId: string, source: 'picked' | 'current') {
    const t = toolById(toolId);
    if (!t) return;
    workSource = source;
    toolStart = toolId;
    pickTab(t.group);
  }
  const askValues = $derived<SettingValues>({ viewerMode, emailLimitMb, recentDays, networkRescan, analysis: analysisOptions, trips: tripPrefs });
  const askHandlers: Handlers = {
    viewer: (m) => setViewerMode(m),
    emailLimit: (mb) => setEmailLimit(mb),
    recentDays: (n) => setRecentDays(n),
    networkRescan: (r) => setNetworkRescanPref(r),
    analysis: (o) => void applyAnalysisOptions(o),
    trips: (p) => setTripPrefs(p),
  };
  /** A button Ask Pixie offered: go to that tab, Settings page or tool. */
  /** What the person has to work on, read when a question is asked. */
  const askPhotos = async () => photoState(picked.size, await pickVideoCount().catch(() => 0), selectedPhoto ? selectedPhoto.kind : null, workSource);
  function askGo(target: string) {
    const place = askPlace(target);
    if (!place) return;
    if (place.settings) settingsPage = place.settings;
    toolStart = place.tool ?? null;
    pickTab(place.tab);
  }
  // The start tool is for the tab it was meant for, once: opening any other tab, or this one later, starts it as usual (at the tool it was last left at).
  $effect(() => { if (toolStart !== null && !(isToolGroup(tab) && toolById(toolStart)?.group === tab)) toolStart = null; });
  /** The photos the buttons beside Picked act on. */
  const pickedIds = $derived([...picked]);
  /** The buttons on the action bars (Settings, Browsing), and how many of them fit in Browse's second line. */
  let barActions = $state(parseBar(loadPref<unknown>('actionBar', null)));
  function setBarActions(next: string[]) { barActions = parseBar(next); savePref('actionBar', barActions); }
  let secondLineW = $state(0);
  const barLimit = $derived(secondLineW === 0 ? barActions.length : directCount(secondLineW, barActions.length));

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
    const firstListing = refresh().catch((e) => (message = String(e)));
    if (mode === 'map') loadPoints(); // the map was left on
    loadFolderInfos();
    void loadPicks();
    void loadHiddenTotal();
    getAnalysisOptions().then((o) => { analysisOptions = parseOptions(o); welcomeOptions = { ...analysisOptions }; }).catch(() => {});
    void loadTrips();
    analysisStatus().then((s) => (analysis = s)).catch(() => {});
    peopleStatus().then((s) => (peopleState = s)).catch(() => {});
    duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {});
    loadTags();
    refreshLocations();
    getDateRules().then((r) => (dateRules = r)).catch(() => { /* the defaults stay */ });
    void loadUserTags();
    loadPeople();
    // Re-scan all known folders on launch to pick up changes.
    const folderList = listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) { void openWelcome(); return; }
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    void Promise.allSettled([firstListing, folderList]).then(() => { started = true; });
    activitySnapshot().then((snapshot) => { activity = snapshot; }).catch(() => {});
    const subs = [
      onActivity((snapshot) => { activity = snapshot; }),
      listen<AnalysisStatus>('analysis-status', (e) => { analysis = e.payload; loadTagsSoon(); }),
      listen<DupStatus>('duplicate-status', (e) => { dupState = e.payload; dupHiddenChanged(); syncCountSoon(); }),
      listen<PeopleStatus>('people-status', (e) => { peopleState = e.payload; loadPeopleSoon(); }),
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number; indexed: number; to_index: number; folder_id: number; latest: Sample | null }>('scan-progress', (e) => {
        scanFolder = e.payload.folder_id;
        scanning = true; // a scan the app started by itself (a watched folder changed, the timer) shows too, and can be stopped
        if (!cancelling) scanPaused = false; // a scan is going, so nothing is waiting to be resumed
        if (e.payload.latest) dialogFeed?.offer(e.payload.latest);
        if (phase !== 'scanning') { if (phase === 'off' || phase === 'background') refreshLive(); return; }
        if (e.payload.latest) feed?.offer(e.payload.latest);
      }),
      // A photo the analysis has just finished goes by on the loading screen's strip; with the library showing, the grid
      // follows as photos are stored.
      listen<Sample>('analysis-latest', (e) => {
        dialogFeed?.offer(e.payload);
        if (phase === 'scanning' || phase === 'analysing') feed?.offer(e.payload);
        else if (phase === 'off' || phase === 'background') refreshLive();
      }),
      listen<{ folders: number; found: number }>('scan-done', (e) => {
        cancelling = false;
        loadTags();
        refreshLocations();
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
      // A frame was saved from a video (in the viewer, here or in its own window): it is in the library already, so the listing is read again
      // (when the viewer closes, if it is open here).
      listen('frame-saved', () => {
        void loadUserTags(); // the tag "Video frame" may be new, and has one more photo
        if (openIndex !== null || soloPhoto !== null) pendingRefresh = true;
        else refresh().catch((e) => (message = String(e)));
      }),
      listen<{ id: number }>('show-frame', (e) => void showFrame(e.payload.id)),
      listen(POPOUT_READY, () => sendShow(false)),
      listen(POPOUT_CLOSED, () => { popoutOpen = false; }),
      listen(POPOUT_MOVED, (e) => {
        const m = parseMoved(e.payload);
        // Only follow while the main window still shows the listing the pop-out is paging through.
        if (!m || m.total !== total || !sameListing(m.view, shown)) return;
        popoutSnap = { index: m.index, total, view: { ...shown } };
        selected = m.index;
      }),
      listen('master-copy-done', () => { void loadShare(); }),
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
<svelte:window oncontextmenu={noBrowserMenu} onkeydown={onAppKey} />
<main inert={phase !== 'off' || activityOpen}>
  <Tabs {tabs} active={tab} onpick={pickTab} onhelp={toggleHelp} {helpOpen} bind:helpButton={helpEl}>
    {#snippet status()}
      <ProcessingPill {pill} onclick={() => (activityOpen = true)} />
      {#if message}<span class="err">{message}</span>{/if}
    {/snippet}
    <div class="browse" inert={tab !== 'browse'}>
      <div class="topbars">
      <div class="tools">
        <span class="count">{kindWord(view.kind, total)}{areaOnly ? ' in the map area' : ''}</span>
        <button class="linkbtn" onclick={() => void pickAllShown()} disabled={!pickAll.enabled} title={pickAll.title}>Pick all</button>
        <label class="check" title={groupByDate ? 'Show the photos as one grid, without a heading for each day' : 'Put the photos under a heading for each day'}>
          <input type="checkbox" checked={groupByDate} onchange={(e) => setGroupByDate(e.currentTarget.checked)} /> Group by date
        </label>
        <label class="check" title={showHidden ? 'Leave the hidden photos out of Browse again' : hiddenTotal > 0 ? `Also list the ${plural(hiddenTotal, 'hidden item')}, marked, so they can be unhidden` : 'Also list the photos you hid, marked, so they can be unhidden (none are hidden now)'}>
          <input type="checkbox" checked={showHidden} onchange={(e) => void setShowHiddenOn(e.currentTarget.checked)} /> Show hidden
        </label>
        {#if activeTag !== null}
          <span class="filter">Auto tagged “{tagLabel(activeTag)}”
            <button onclick={() => setTag(null)} aria-label="Clear tag" title="Clear tag">✕</button></span>
        {/if}
        {#if activeUtag === 'picked'}
          <span class="filter">Picked ★
            <button onclick={() => setUtag(null)} aria-label="Show all photos again" title="Show all photos again">✕</button></span>
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
        {#if activeMissing !== null}
          <span class="filter">No location
            <button onclick={() => setMissing(null)} aria-label="Show items with or without a location" title="Show items with or without a location">✕</button></span>
        {/if}
        {#if activeShare !== null}
          <span class="filter">{activeShare === 'new' ? 'New, not yet synced' : 'Private'}
            <button onclick={() => setShare(null)} aria-label="Stop showing only {activeShare === 'new' ? 'the new items' : 'the private items'}" title="Stop showing only {activeShare === 'new' ? 'the new items' : 'the private items'}">✕</button></span>
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
      <div class="tools second" bind:clientWidth={secondLineW}>
        <!-- Two boxed sections, each with its title: Picked (what is picked, and the buttons that act on it) and Map. -->
        <!-- Only while something is picked: with nothing picked it would offer nothing. -->
        {#if picked.size > 0}
        <div class="section" role="group" aria-label="Picked photos">
          <span class="stitle" title="The picked photos: the circle in a photo's top-left corner picks it">Picked ★ {picked.size.toLocaleString()}</span>
          <label class="check" title={activeUtag === 'picked' ? 'Show all photos again' : 'Show only the picked photos'}>
            <input type="checkbox" checked={activeUtag === 'picked'} onchange={(e) => setUtag(e.currentTarget.checked ? 'picked' : null)} /> Filter
          </label>
          <button class="linkbtn" onclick={clearAllPicks} title="Take everything off Picked (the photos stay in the library)">Clear</button>
          <button class="linkbtn" onclick={() => void hidePicked()} title="Hide the picked photos from Browse, People, Trips and the tools. The files are never touched; Show hidden brings them back">Hide</button>
          {#if showHidden}
            <button class="linkbtn" onclick={() => void unhidePicked()} title="Show the hidden photos among the picked ones in Browse again">Unhide</button>
          {/if}
          <PhotoActions source="picked" ids={pickedIds} chosen={barActions} limit={barLimit} showTag onopen={openTool} ontagchange={(t) => void utagChanged(t)} />
        </div>
        {/if}
        <div class="section" role="group" aria-label="Map">
          <span class="stitle">Map</span>
          <label class="check" title="Show a map above the photos">
            <input type="checkbox" checked={mode === 'map'} onchange={(e) => setMapShown(e.currentTarget.checked)} /> Display
          </label>
          <!-- Only while the map is on; then the grid follows the part of the map in view, and a pill says so. -->
          <label class="check" class:off={mode !== 'map' || activeMissing !== null} title={activeMissing !== null ? 'Items with no location are not on the map, so they cannot be filtered by it' : mode === 'map' ? 'Show only the photos in the part of the map you can see' : 'Turn the map on to filter by it'}>
            <input type="checkbox" checked={areaOnly} disabled={mode !== 'map' || activeMissing !== null} onchange={(e) => setAreaOnly(e.currentTarget.checked)} /> Filter
          </label>
        </div>
      </div>
      </div>
      <div class="content">
        {#if !started}
          <!-- the splash covers the window (below) -->
        {:else if folders.length === 0 && total === 0 && !scanning}
          <div class="empty">
            <p>Add a folder of photos to get started.</p>
            <button onclick={chooseFolder} title="Choose a folder of photos and videos to add to the library">Add photo folder…</button>
          </div>
        {:else}
      <Shell>
        {#snippet left()}
          <Nav groups={navGroups} kind={view.kind} recent={recentOn}
               period={periodKey} recentText={recentLabel(recentDays)}
               {facets} {tags} tag={activeTag} ontag={setTag} userTags={userTagList} utag={activeUtag} onutag={setUtag}
               people={peopleList} facesFound={(peopleState?.done ?? 0) > 0} person={activePerson} onperson={setPerson}
               cameras={facets?.cameras ?? []} camera={activeCamera} oncamera={setCamera} missing={activeMissing} onmissing={setMissing} noLocation={facets?.noLocation ?? 0} {master} onopensync={() => openTool('sync', 'picked')} shareCounts={shareState} share={activeShare} onshare={setShare} libMissing={{ location: (libLocations?.without ?? 0) > 0, date: (libLocations?.undated ?? 0) > 0 }} analysisOn={analysisOptions} onopenanalysis={openAnalysisDetails}
               folders={folderInfos} folderStates={folderStateMap} folderId={view.folderId} onfolder={setFolder}
               {trips} tripsReady={tripPrefs.confirmed} trip={tripKey} ontrip={setTrip} {destinations} onvisibletrips={(l) => void resolveDestinations(l)} onrenametrip={renameTripTo} onsettrips={openTripSettings} onnamepeople={() => pickTab('faces')}
               {collapsed} ontoggle={toggleNav}
               onperiod={setPeriod} onkind={setKind} onrecent={setRecent} />
        {/snippet}
        {#snippet center()}
          {#if mode === 'map'}
            <div class="mapsplit">
              <div class="maptop">
                <div class="mapbox">
                <PhotoMap {points} {fitMap} onhand={(moved) => (mapMoved = moved)} emptyNote={activeMissing !== null ? 'These items have no location, so none can be shown on the map.' : undefined} initial={mapStart} route={tripKey !== null ? routeData : null} stopNames={routeNames}
                          onbounds={setBounds}
                          onpin={revealPhoto}
                          onopenphoto={(p) => (soloPhoto = p)}
                          onmove={(v) => { lastMap = v; savePref('mapView', v); }} />
                </div>
                {#if mapReminder !== null}
                  <p class="reminder" role="status">{mapReminder} <button class="linkbtn" onclick={() => setMissing('location')} title="Show only the items that have no location">Show them</button></p>
                {/if}
              </div>
              <div class="mapbottom">
                {#if total === 0 && !scanning}
                  <p class="empty small">{areaOnly ? 'No photos or videos in this part of the map.' : emptyMessage}</p>
                {:else}
                  <Grid {groups} {version} view={shown} {selected} {scrollTo} {reveal} picks={picked} onpick={togglePick} grouped={groupByDate} {showHidden}
                        onopen={openItem}
                        onselect={selectItem} oncontext={photoMenu} />
                {/if}
              </div>
            </div>
          {:else if total === 0 && !scanning}
            <p class="empty">{emptyMessage}</p>
          {:else}
            <Grid {groups} {version} view={shown} {selected} {scrollTo} {reveal} picks={picked} onpick={togglePick} grouped={groupByDate} {showHidden}
                  onopen={openItem}
                  onselect={selectItem} oncontext={photoMenu} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} chosen={barActions} {master} onsharechange={sharePhotoChanged} onhiddenchange={() => void hiddenChanged()} onopen={openTool} ontagschange={(t) => void utagChanged(t)} />
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
    {#if isToolGroup(tab)}
      {#key tab}
        <div class="page-over">
          <ToolsTab {dateRules} ondaterules={changeDateRules} onshowundated={showUndated} group={tab} tool={toolStart} active bind:source={workSource} photo={selectedPhoto} browse={shown} shownCount={total} picks={picked} onunpick={togglePick} onclear={clearAllPicks} {emailLimitMb} userTags={userTagList} onchange={(t) => void utagChanged(t)}
                    folders={folderInfos} {master} onmaster={chooseMaster} subfolder={syncSub} onsubfolder={chooseSyncSub} {copyStatus} oncopy={copyNow} onstopcopy={() => void stopMasterCopy()} onfeeds={feedsMaster} remind={remindSync} onremind={setRemindSync} onduplicates={() => { duplicateStatus().then((s) => { dupState = s; dupHiddenShown = s.hidden; }).catch(() => {}); refresh().catch((e) => (message = String(e))); loadFolderInfos(); }} />
        </div>
      {/key}
    {/if}
    {#if tab === 'settings'}
      <div class="page-over">
        <Settings askOn={askOn} onask={setAskOn} locations={libLocations} analysisOptions={analysisOptions} onanalysisoptions={applyAnalysisOptions} emailLimitMb={emailLimitMb} onemaillimit={setEmailLimit} folders={folderInfos} folderStates={folderStateMap} {scanning} {scanFolder} onstop={pauseScan} onadd={chooseFolder} onremove={removeFolderAndRefresh} {master}
                  {networkRescan} onnetworkrescan={setNetworkRescanPref}
                  page={settingsPage} onpage={(p) => (settingsPage = p)} {tripPrefs} ontripprefs={setTripPrefs} onopentool={(id) => openTool(id, 'picked')} {viewerMode} onviewer={setViewerMode} actionBar={barActions} onactionbar={setBarActions} {recentDays} onrecentdays={setRecentDays} />
      </div>
    {/if}
  </Tabs>
  {#if helpBuilt}
    <AskCard open={helpOpen} anchor={helpEl} onclose={() => (helpOpen = false)}>
      <AskPanel on={askOn && typeof askInstall === 'string'} installId={typeof askInstall === 'string' ? askInstall : ''} values={askValues} handlers={askHandlers}
                from={tab} photos={askPhotos} language={navigator.language} ongo={askGo} onsettings={() => { settingsPage = 'ask'; pickTab('settings'); }} />
    </AskCard>
  {/if}
  {#if openIndex !== null || soloPhoto !== null}
    <Viewer index={openIndex ?? 0} {total} view={shown} solo={soloPhoto} onclose={closeViewer} onnav={(i) => (openIndex = i)}
            onpopout={() => popOut(openIndex ?? 0)} />
  {/if}
</main>

{#if activityOpen}
  <ProcessingDialog snapshot={activity} strip={dialogStrip} scanPaused={scanPaused || syncPaused} onpause={pauseResume} onretry={() => analysisRetry()} onclose={() => (activityOpen = false)} />
{/if}

{#if !started}
  <div class="splash" role="status" aria-label="Pixie is starting">
    <AppIcon size={96} />
    <p>Starting Pixie…</p>
  </div>
{/if}

{#if reminder && master !== null}
  <SyncReminder held={shareState.unsynced} folder={master} copying={copyStatus?.running ?? false} onreview={reviewNew} onsync={syncFromReminder} onclose={closeReminder} />
{/if}

{#if phase === 'welcome'}
  <Welcome rows={pickerRows} searching={sniffing} {looking} ontoggle={(p) => (rows = toggle(rows, p))} onadd={welcomeAdd}
           onskip={() => { stopWelcomeSubs(); phase = 'off'; }} onstart={welcomeNext}
           adding={addMode} oncancel={cancelAdd} notice={pickNotice} />
{:else if phase === 'options'}
  <AnalysisStep options={welcomeOptions} onoptions={(o) => (welcomeOptions = o)} onback={() => (phase = 'welcome')} onstart={optionsStart} />
{:else if phase === 'scanning' || phase === 'analysing'}
  <LoadModal {strip} snapshot={activity} scanPaused={scanPaused || syncPaused} notice={message}
             onbackground={carryOnInBackground} onpause={pauseResume} onretry={() => analysisRetry()} />
{:else if phase === 'background'}
  <BackgroundNote scanning={scanning} onpeople={() => { phase = 'off'; pickTab('faces'); }}
                  onbrowse={startBrowsing} />
{:else if phase === 'allset' && allSet}
  <AllSet summary={allSet} onpeople={() => { phase = 'off'; pickTab('faces'); }}
          onbrowse={startBrowsing}
          onadd={() => { phase = 'off'; void chooseFolder(); }} />
{/if}

<style>
  /* The inner margin of a page or tool pane (Settings, the Clean up tools, People): one place, so they all line up. */
  :global(:root) { --page-pad-y: 14px; --page-pad-x: 18px; --page-pad: var(--page-pad-y) var(--page-pad-x); }
  :global(body) { margin: 0; font-family: system-ui, sans-serif; font-size: 14px; background: #fff; color: #111; }
  /* Plain buttons look the same on every system (the Mac's own are tiny); any style written for a button wins over this. */
  :global(:where(button)) { font: inherit; font-size: 14px; padding: 5px 12px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; cursor: pointer; }
  :global(:where(button:disabled)) { opacity: .5; cursor: default; }
  /* The one main action of a screen, dialog or panel is blue (class="primary"); an action that deletes or resets things is red (class="danger"); every other button stays white. */
  :global(button.primary) { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  :global(button.danger) { background: #b3261e; border-color: #b3261e; color: #fff; }
  :global(:where(select)) { font: inherit; font-size: 14px; padding: 4px 8px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; }
  :global(:where(select:disabled)) { opacity: .5; }
  :global(:where(input[type='checkbox'], input[type='radio'])) { width: 16px; height: 16px; vertical-align: -3px; }
  main { display: flex; flex-direction: column; height: 100vh; }
  .browse { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .topbars { border-bottom: 1px solid #eee; }
  .tools { display: flex; gap: 12px; align-items: center; padding: 6px 14px; }
  .tools.second { padding-top: 0; flex-wrap: wrap; row-gap: 4px; align-items: stretch; }
  /* A section of the line: a box with its title at the left. */
  .section { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 3px 10px; min-height: 26px; box-sizing: border-box; border: 1px solid #b8bdca; border-radius: 6px; background: #f6f8fc; }
  .stitle { font-size: 13px; font-weight: 700; color: #333; font-variant-numeric: tabular-nums; }
  .content { flex: 1; min-height: 0; }
  .count { font-size: 13px; font-weight: 600; color: #333; font-variant-numeric: tabular-nums; }
  .filter { display: inline-flex; align-items: center; gap: 6px; padding: 2px 4px 2px 10px; border-radius: 12px; background: #e3ecff; color: #1a3f9a; font-size: 13px; }
  .filter button { border: 0; background: none; color: inherit; font: inherit; cursor: pointer; padding: 0 6px; border-radius: 10px; }
  /* A quiet link, not a button: it sits in the row of pills without looking like one of them. */
  .linkbtn:disabled { opacity: .5; cursor: default; text-decoration: none; }
  .linkbtn { border: 0; background: none; padding: 2px 4px; font: inherit; font-size: 13px; color: #1a3f9a; cursor: pointer; }
  .linkbtn:hover { text-decoration: underline; }
  /* Two independent switches: the map, and whether the grid follows it. */
  .check { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: #333; cursor: pointer; }
  .check.off { color: #999; cursor: default; }
  .check input { margin: 0; }
  .filter button:hover { background: rgba(47, 111, 237, .2); }
  .page-over { position: absolute; inset: 0; z-index: 5; }
  .splash { position: fixed; inset: 0; z-index: 100; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; background: #fff; color: #666; font-size: 14px; }
  .splash p { margin: 0; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
  .mapsplit { display: flex; flex-direction: column; height: 100%; }
  .maptop { flex: 0 0 45%; min-height: 220px; margin-bottom: 8px; display: flex; flex-direction: column; }
  .mapbox { flex: 1; min-height: 0; }
  .reminder { margin: 4px 0 0; font-size: 12px; color: #6a5a1a; }
  /* A line from edge to edge of the panel (past the Shell's 8px side padding), with space either side of it, between the map and the photo list. */
  .mapbottom { flex: 1; min-height: 0; margin: 0 -8px; border-top: 1px solid #b8bdca; padding: 8px 8px 0; box-sizing: border-box; }
  .empty.small { margin-top: 24px; }
</style>
