<script lang="ts">
  /** One of the tool tabs (Send, Create, Organize, Clean up, Import/Export), laid out like Settings: the tab's tools down the left, the options of the chosen tool in the middle, and the photos
   *  the tools work on on the right. Clean up and Import/Export look after the whole library, so they have no "Working on" and no photos. */
  import { untrack } from 'svelte';
  import { kindCounts, listPickedIds, listTaggedIds, photosAmong, type FolderInfo, type Photo, type UserTagCount, type View } from './api';
  import DuplicatesTab from './DuplicatesTab.svelte';
  import MissingDatesTool from './MissingDatesTool.svelte';
  import type { DateRules } from './dateRules';
  import EmailForm from './EmailForm.svelte';
  import GroupTags from './GroupTags.svelte';
  import type { CopyStatus } from './master';
  import PlaceholderTool from './PlaceholderTool.svelte';
  import CollageTool from './CollageTool.svelte';
  import MosaicTool from './MosaicTool.svelte';
  import PrintTool from './PrintTool.svelte';
  import Resizer from './Resizer.svelte';
  import ImportTool from './ImportTool.svelte';
  import SyncTool from './SyncTool.svelte';
  import ToolsGrid from './ToolsGrid.svelte';
  import Viewer from './Viewer.svelte';
  import WorkingOn from './WorkingOn.svelte';
  import { plural } from './format';
  import { loadPref, savePref } from './prefs';
  import { firstTool, GROUPS, toolsIn, worksOnWholeLibrary, type ToolGroup } from './tools';
  import { COLLAGE_MAX, PRINT_MAX } from './toolLimits';

  let { group, tool = null, active, source = $bindable(), photo, browse, shownCount, picks, onunpick, onclear, emailLimitMb, userTags, onchange, onduplicates, dateRules, ondaterules, onshowundated, folders, master, onmaster, subfolder, onsubfolder, copyStatus, oncopy, onstopcopy, onfeeds, remind, onremind, onimportbrowse = () => {} }: {
    group: ToolGroup;
    /** The tool to open at; otherwise the one used last in this tab, or the first. */
    tool?: string | null;
    active: boolean;
    /** Which photos the tools work on: `picked`, `current` (the photo chosen in Browse) or `tag:<name>`. Shared by the tabs. */
    source: string;
    /** The photo chosen in Browse, if any. */
    photo: Photo | null;
    /** The listing Browse shows now (the Photomosaic can take its tiles from it). */
    browse: View;
    /** How many photos Browse shows now. */
    shownCount: number;
    /** The email size limit (Settings, Email). */
    emailLimitMb: number;
    /** The picked photos' ids: the list is read again when they change. */
    picks: ReadonlySet<number>;
    onunpick: (id: number) => void;
    onclear: () => void;
    /** Your tags with their counts (read again when they change). */
    userTags: UserTagCount[];
    /** Something was tagged (with the tag named): the tag lists need reading again. */
    onchange: (tag: string) => void;
    /** Copies were moved to the Recycle Bin: the rest of the app should refresh its counts. */
    onduplicates: () => void;
    /** Which dates count as missing and when the file's date stands in (Missing dates), a change to that, and "show the items with no date" in Browse. */
    dateRules: DateRules; ondaterules: (r: DateRules) => void; onshowundated: () => void;
    /** Import: look at a source (a card, a stick or a folder that was opened) in Browse. */
    onimportbrowse?: (source: number) => void;
    /** Sync: the watched folders, the sync folder (null while there is none) and choosing it, what is waiting to be copied, and the controls to copy. */
    folders: FolderInfo[]; master: string | null; onmaster: (path: string | null) => void;
    subfolder: string; onsubfolder: (name: string) => Promise<void>;
    copyStatus: CopyStatus | null; oncopy: () => void; onstopcopy: () => void; onfeeds: (id: number, feeds: boolean) => void;
    /** Whether Pixie reminds at start about new items to sync, and a change to that. */
    remind: boolean; onremind: (on: boolean) => void;
  } = $props();

  const list = $derived(toolsIn(group));
  const groupLabel = $derived(GROUPS.find((g) => g.id === group)?.label ?? group);

  // The widths of the tools list and of the options column; the photos take the rest. Remembered, as Browse's panes are.
  let navW = $state(loadPref('tools.nav', 230));
  let sideW = $state(loadPref('tools.side', 300));

  // Each tab remembers the tool it was left at.
  let chosenId = $state(untrack(() => (tool && toolsIn(group).some((t) => t.id === tool) ? tool : loadPref<string>(`tools.tool.${group}`, firstTool(group).id))));
  const current = $derived(list.find((t) => t.id === chosenId) ?? firstTool(group));
  // Clean up and Import/Export look after the whole library; so does the Photomosaic, which takes its tiles from it (or from what Browse shows, or the picks) by itself.
  const whole = $derived(worksOnWholeLibrary(group) || current.id === 'mosaic');
  function choose(id: string) {
    chosenId = id;
    savePref(`tools.tool.${group}`, id);
  }

  // The photo chosen in Browse, kept while this tab is open: Browse lets go of it when it refreshes (a rescan, a tag change), which must not change what the tools work on.
  let held = $state<Photo | null>(null);
  $effect(() => { if (photo) held = photo; });
  const here = $derived(photo ?? held);

  // With nothing picked, start on the photo chosen in Browse (once, when the tab opens); and never work on the current photo when there is none.
  let started = false;
  $effect(() => {
    if (started) return;
    started = true;
    untrack(() => { if (!whole && source === 'picked' && picks.size === 0 && photo) source = 'current'; });
  });
  $effect(() => { if (source === 'current' && !here) source = 'picked'; });
  // A tag that has no photos left is gone from the list: go back to Picked.
  $effect(() => { if (source.startsWith('tag:') && !userTags.some((t) => `tag:${t.name}` === source)) source = 'picked'; });

  /** The ids of the photos the tools work on, newest first. Only the ids are held: the photos of the rows on show are read as they are needed
   *  (see ToolsGrid), and the tools that need the photos themselves ask for the ones they can use (see `usable`). */
  let ids = $state<number[]>([]);
  /** How many of them are pictures and how many videos. */
  let kinds = $state({ pictures: 0, videos: 0 });
  /** The source `ids` was read for. */
  let listed = '';
  let error = $state('');
  /** The photo clicked here (blue outline); double-click or Enter opens the viewer. */
  let chosen = $state<number | null>(null);
  /** The photo open in the viewer (its position among these photos), if any. */
  let viewing = $state<number | null>(null);
  // The viewer pages through these photos as they are listed here (it does not use Browse's listing).
  const noListing: View = { order: 'newest', folderId: null, kind: null, bounds: null, maxId: null, since: null, until: null, search: false };
  const pageOfPhotos = (offset: number, limit: number) => photosAmong(shownIds.slice(offset, offset + limit), false, limit);
  // A photo that leaves the list while it is open: close the viewer rather than show a different one.
  $effect(() => { if (viewing !== null && viewing >= shownIds.length) viewing = null; });

  $effect(() => {
    picks; userTags; // read again whenever the picks or the tags change
    const from = source;
    const one = here;
    if (!active || whole) return;
    // Another set of photos: the old list must not stay on show (and be acted on) while the new one is read, or if it cannot be.
    if (from !== listed) { listed = from; ids = []; kinds = { pictures: 0, videos: 0 }; }
    if (from === 'current') {
      ids = one ? [one.id] : [];
      kinds = { pictures: one && one.kind === 'photo' ? 1 : 0, videos: one && one.kind === 'video' ? 1 : 0 };
      error = '';
      return;
    }
    let live = true;
    (from === 'picked' ? listPickedIds() : listTaggedIds(from.slice(4)))
      .then(async (l) => {
        const [pictures, videos] = l.length === 0 ? [0, 0] : await kindCounts(l);
        if (live) { ids = l; kinds = { pictures, videos }; error = ''; }
      })
      .catch((e) => { if (live) { ids = []; kinds = { pictures: 0, videos: 0 }; error = String(e); } });
    return () => { live = false; };
  });

  // Print and Collage work on the pictures among them (videos cannot be printed), as many as they can take: the photos themselves are read for those two only.
  const usable = $derived(current.id === 'print' ? PRINT_MAX : current.id === 'collage' ? COLLAGE_MAX : 0);
  let printable = $state<Photo[]>([]);
  $effect(() => {
    const list = ids;
    const cap = usable;
    if (cap === 0 || !active || whole) return;
    if (list.length === 0) { printable = []; return; }
    let live = true;
    photosAmong(list, true, cap).then((l) => { if (live) printable = l; }).catch(() => { if (live) printable = []; });
    return () => { live = false; };
  });
  const videosLeft = $derived(kinds.videos);
  /** The photos listed on the right: Print leaves the videos out of the list too, since it cannot print them. */
  const shownIds = $derived(current.id === 'print' && kinds.videos > 0 ? printable.map((p) => p.id) : ids);
  /** Pictures the tool has no room for. */
  const beyond = $derived(usable === 0 ? 0 : Math.max(0, kinds.pictures - usable));
  const noPrint = (id: string) => (id === 'print' || id === 'collage') && ids.length > 0 && kinds.pictures === 0;
  const isPicked = $derived(source === 'picked');
  // The Collage has its own page in the middle and its own photo list on the right, so it takes the whole area beside the tools list.
  const wide = $derived(current.id === 'collage');
</script>

<div class="page">
  {#if whole && current.id !== 'mosaic' && current.id !== 'import'}
    <div class="scope"><b>Whole library.</b> {current.label} works on every photo, not on the ones you picked.</div>
  {:else if !whole}
    <WorkingOn bind:source pickedCount={picks.size} hasPhoto={here !== null} {userTags} count={ids.length} {onclear} {error} />
  {/if}
  <div class="body" style:grid-template-columns={whole || wide ? `${navW}px 5px minmax(0, 1fr)` : `${navW}px 5px ${sideW}px 5px minmax(0, 1fr)`}>
    <nav aria-label={`${groupLabel} tools`}>
      {#each list as t (t.id)}
        <button class:on={current.id === t.id} class:soon={!t.built || (noPrint(t.id) && current.id !== t.id)} aria-current={current.id === t.id ? 'page' : undefined}
                disabled={noPrint(t.id) && current.id !== t.id} title={noPrint(t.id) ? (t.id === 'print' ? "Videos can't be printed" : "Videos can't go in a collage") : undefined} onclick={() => choose(t.id)}>
          {t.label}{#if !t.built}<small>soon</small>{/if}
        </button>
      {/each}
    </nav>
    <Resizer bind:width={navW} min={160} max={360} side="left" onchange={(w) => savePref('tools.nav', w)} />

    {#if whole}
      <div class="whole" role="region" aria-label={current.label}>
        {#if current.id === 'duplicates'}
          <DuplicatesTab {active} onchange={onduplicates} />
        {:else if current.id === 'missing-dates'}
          <MissingDatesTool rules={dateRules} onrules={ondaterules} {active} onshow={onshowundated} />
        {:else if current.id === 'mosaic'}
          <MosaicTool photo={here} {browse} filteredCount={shownCount} pickedCount={picks.size} {userTags} {onclear} {active} defaultDir={master ?? folders[0]?.path ?? null} />
        {:else if current.id === 'import'}
          <ImportTool {folders} {master} {picks} onbrowse={onimportbrowse} />
        {:else if current.id === 'sync'}
          <SyncTool {folders} {master} {onmaster} {subfolder} {onsubfolder} {copyStatus} {oncopy} {onstopcopy} {onfeeds} {remind} {onremind} />
        {:else}
          <PlaceholderTool tool={current} />
        {/if}
      </div>
    {:else if wide}
      <div class="whole" role="region" aria-label={current.label}>
        <CollageTool photos={printable} {active} skippedVideos={videosLeft} beyond={beyond} defaultDir={master ?? folders[0]?.path ?? null} />
      </div>
    {:else}
      <aside class="side" aria-label={current.label}>
        {#if current.id === 'email'}
          <div class="tool wide"><EmailForm {ids} limitMb={emailLimitMb} /></div>
        {:else if current.id === 'print'}
          {#if videosLeft > 0}
            <p class="note">{plural(videosLeft, 'video')} can't be printed{kinds.pictures > 0 ? `, so ${videosLeft === 1 ? 'it is' : 'they are'} left out` : ''}.</p>
          {/if}
          {#if beyond > 0}
            <p class="note">Print takes {PRINT_MAX.toLocaleString()} photos at a time, the newest first: {plural(beyond, 'photo')} left out. Pick fewer, or use a tag, to print the rest.</p>
          {/if}
          <PrintTool photos={printable} />
        {:else if current.id === 'tags'}
          <div class="tool"><GroupTags {ids} {onchange} /></div>
        {:else}
          <PlaceholderTool tool={current} />
        {/if}
      </aside>
      <Resizer bind:width={sideW} min={240} max={560} side="left" onchange={(w) => savePref('tools.side', w)} />

      <div class="mid">
        {#if ids.length === 0}
          <div class="none">
            {#if isPicked}
              <p><b>Nothing is picked yet.</b></p>
              <p>In Browse, move the mouse over the top-left corner of a photo and click the circle that appears. Ctrl-click a photo to pick it, or Shift-click to pick everything from the photo you chose last.</p>
              <p>The photos you pick are listed here, and can be emailed or tagged together.</p>
            {:else if source === 'current'}
              <p><b>No photo is chosen.</b></p>
            {:else}
              <p><b>No photos have this tag yet.</b></p>
            {/if}
          </div>
        {:else}
          <ToolsGrid ids={shownIds} {chosen} load={(l) => photosAmong(l, false, l.length)} onchoose={(id) => (chosen = id)} onopen={(i) => (viewing = i)} picked={isPicked} {onunpick} />
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if viewing !== null}
  <Viewer index={viewing} total={shownIds.length} view={noListing} page={pageOfPhotos} onclose={() => (viewing = null)} onnav={(i) => (viewing = i)} />
{/if}

<style>
  /* Fills the page over Browse, as the other tabs do; the columns are laid out like Settings (menu, then the page). */
  .page { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; }
  .scope { padding: 6px 14px; background: #fff7d6; border-bottom: 1px solid #e0c75a; color: #5a4a00; font-size: 14px; }
  .body { flex: 1; min-height: 0; display: grid; }
  nav { background: #fafafb; border-right: 1px solid #e4e4e8; padding: 6px 4px 16px; font-size: 14px; display: grid; align-content: start; gap: 1px; overflow-y: auto; }
  nav button { text-align: left; border: 0; background: none; font: inherit; padding: 4px 12px; border-radius: 4px; cursor: pointer; }
  nav button:hover:not(.on) { background: #eef2fb; }
  nav button.on { background: #e3ecff; font-weight: 600; }
  nav button.soon { color: #9a9aa4; }
  nav small { margin-left: 6px; font-size: 11px; color: #b0b0b8; }
  .whole { position: relative; min-width: 0; min-height: 0; overflow: auto; }
  .mid { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .none { padding: 40px 28px; color: #555; max-width: 520px; line-height: 1.5; }
  .none p { margin: 0 0 12px; }
  .side { min-width: 0; overflow-y: auto; overflow-x: hidden; border-right: 1px solid #e4e4e8; background: #fafafb; }
  .note { margin: 0; padding: 10px 14px 0; font-size: 13px; color: #8a6d00; }
  .tool { padding: 12px 14px; font-size: 14px; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
  .tool.wide { align-items: stretch; }
</style>
