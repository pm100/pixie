<script lang="ts">
  /** One of the tool tabs (Send, Create, Organize, Clean up), laid out like Settings: the tab's tools down the left, the options of the chosen tool in the middle, and the photos
   *  the tools work on on the right. Clean up looks after the whole library, so it has no "Working on" and no photos. */
  import { untrack } from 'svelte';
  import { listPicked, listTagged, type Photo, type UserTagCount, type View } from './api';
  import DuplicatesTab from './DuplicatesTab.svelte';
  import MissingDatesTool from './MissingDatesTool.svelte';
  import type { DateRules } from './dateRules';
  import EmailForm from './EmailForm.svelte';
  import GroupTags from './GroupTags.svelte';
  import PlaceholderTool from './PlaceholderTool.svelte';
  import PrintTool from './PrintTool.svelte';
  import Resizer from './Resizer.svelte';
  import Viewer from './Viewer.svelte';
  import WorkingOn from './WorkingOn.svelte';
  import { plural } from './format';
  import { loadPref, savePref } from './prefs';
  import { firstTool, GROUPS, toolsIn, worksOnWholeLibrary, type ToolGroup } from './tools';
  import { posterUrl, thumbUrl } from './url';

  let { group, tool = null, active, source = $bindable(), photo, picks, onunpick, onclear, emailLimitMb, userTags, onchange, dupes, onduplicates, dateRules, ondaterules, onshowundated }: {
    group: ToolGroup;
    /** The tool to open at; otherwise the one used last in this tab, or the first. */
    tool?: string | null;
    active: boolean;
    /** Which photos the tools work on: `picked`, `current` (the photo chosen in Browse) or `tag:<name>`. Shared by the tabs. */
    source: string;
    /** The photo chosen in Browse, if any. */
    photo: Photo | null;
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
    /** The duplicate check is on, so Duplicates is listed in Clean up. */
    dupes: boolean;
    /** Copies were moved to the Recycle Bin: the rest of the app should refresh its counts. */
    onduplicates: () => void;
    /** Which dates count as missing and when the file's date stands in (Missing dates), a change to that, and "show the items with no date" in Browse. */
    dateRules: DateRules; ondaterules: (r: DateRules) => void; onshowundated: () => void;
  } = $props();

  const features = $derived({ dupes });
  const list = $derived(toolsIn(group, features));
  const whole = $derived(worksOnWholeLibrary(group));
  const groupLabel = $derived(GROUPS.find((g) => g.id === group)?.label ?? group);

  // The widths of the tools list and of the options column; the photos take the rest. Remembered, as Browse's panes are.
  let navW = $state(loadPref('tools.nav', 230));
  let sideW = $state(loadPref('tools.side', 300));

  // Each tab remembers the tool it was left at.
  let chosenId = $state(untrack(() => (tool && toolsIn(group, { dupes }).some((t) => t.id === tool) ? tool : loadPref<string>(`tools.tool.${group}`, firstTool(group, { dupes }).id))));
  const current = $derived(list.find((t) => t.id === chosenId) ?? firstTool(group, features));
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

  let photos = $state<Photo[]>([]);
  /** The source `photos` was read for. */
  let listed = '';
  let error = $state('');
  let broken = $state<Record<number, boolean>>({});
  /** The photo clicked here (blue outline); double-click or Enter opens the viewer. */
  let chosen = $state<number | null>(null);
  /** The photo open in the viewer (its position among these photos), if any. */
  let viewing = $state<number | null>(null);
  // The viewer pages through these photos as they are listed here (it does not use Browse's listing).
  const noListing: View = { order: 'newest', folderId: null, kind: null, bounds: null, maxId: null, since: null, until: null, search: false };
  const pageOfPhotos = (offset: number, limit: number) => Promise.resolve(photos.slice(offset, offset + limit));
  // A photo that leaves the list while it is open: close the viewer rather than show a different one.
  $effect(() => { if (viewing !== null && viewing >= photos.length) viewing = null; });

  $effect(() => {
    picks; userTags; // read again whenever the picks or the tags change
    const from = source;
    const one = here;
    if (!active || whole) return;
    // Another set of photos: the old list must not stay on show (and be acted on) while the new one is read, or if it cannot be.
    if (from !== listed) { listed = from; photos = []; }
    if (from === 'current') { photos = one ? [one] : []; error = ''; return; }
    let live = true;
    (from === 'picked' ? listPicked() : listTagged(from.slice(4)))
      .then((l) => { if (live) { photos = l; error = ''; } })
      .catch((e) => { if (live) { photos = []; error = String(e); } });
    return () => { live = false; };
  });

  const ids = $derived(photos.map((p) => p.id));
  // Videos cannot be printed: Print works on the pictures among them, and is greyed out when there are none.
  const printable = $derived(photos.filter((p) => p.kind === 'photo'));
  const videosLeft = $derived(photos.length - printable.length);
  const noPrint = (id: string) => id === 'print' && photos.length > 0 && printable.length === 0;
  const isPicked = $derived(source === 'picked');
</script>

<div class="page">
  {#if whole}
    <div class="scope"><b>Whole library.</b> {current.label} works on every photo, not on the ones you picked.</div>
  {:else}
    <WorkingOn bind:source pickedCount={picks.size} hasPhoto={here !== null} {userTags} count={photos.length} {onclear} {error} />
  {/if}
  <div class="body" style:grid-template-columns={whole ? `${navW}px 5px minmax(0, 1fr)` : `${navW}px 5px ${sideW}px 5px minmax(0, 1fr)`}>
    <nav aria-label={`${groupLabel} tools`}>
      {#each list as t (t.id)}
        <button class:on={current.id === t.id} class:soon={!t.built || (noPrint(t.id) && current.id !== t.id)} aria-current={current.id === t.id ? 'page' : undefined}
                disabled={noPrint(t.id) && current.id !== t.id} title={noPrint(t.id) ? "Videos can't be printed" : undefined} onclick={() => choose(t.id)}>
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
        {:else}
          <PlaceholderTool tool={current} />
        {/if}
      </div>
    {:else}
      <aside class="side" aria-label={current.label}>
        {#if current.id === 'email'}
          <div class="tool wide"><EmailForm {ids} limitMb={emailLimitMb} /></div>
        {:else if current.id === 'print'}
          {#if videosLeft > 0}
            <p class="note">{plural(videosLeft, 'video')} can't be printed{printable.length > 0 ? ' and left out' : ''}.</p>
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
        {#if photos.length === 0}
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
          <div class="grid">
            {#each photos as p, i (p.id)}
              <div class="tile" class:chosen={chosen === p.id}>
                <!-- The whole photo is the button: one click chooses it, a double click or Enter opens it. The unpick marker is a separate button over its corner. -->
                <button class="open" aria-label={p.kind === 'video' ? `Video ${p.path}` : `Photo ${p.path}`} aria-pressed={chosen === p.id}
                        onclick={() => (chosen = p.id)} ondblclick={() => (viewing = i)}
                        onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); viewing = i; } }}></button>
                {#if p.kind === 'video' && !broken[p.id]}
                  <img alt="" loading="lazy" src={posterUrl(p.id, p.mtime, p.size)} onerror={() => (broken[p.id] = true)} />
                {:else if p.kind === 'photo' && !broken[p.id]}
                  <img alt="" loading="lazy" src={thumbUrl('grid', p.id, p.mtime, p.size)} onerror={() => (broken[p.id] = true)} />
                {:else}
                  <span class="ph" aria-hidden="true">{p.kind === 'video' ? '▶' : '⚠︎'}</span>
                {/if}
                {#if isPicked}
                  <button class="pick on" aria-label="Take off Picked" title="Take off Picked" onclick={() => onunpick(p.id)}>
                    <span class="tk" aria-hidden="true"><svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2.5l2.9 6.2 6.8.8-5 4.7 1.3 6.7L12 17.5 6 20.9l1.3-6.7-5-4.7 6.8-.8z" /></svg></span>
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if viewing !== null}
  <Viewer index={viewing} total={photos.length} view={noListing} page={pageOfPhotos} onclose={() => (viewing = null)} onnav={(i) => (viewing = i)} />
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
  .grid { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-wrap: wrap; align-content: flex-start; gap: 6px; padding: 12px 14px; }
  .tile { position: relative; width: 180px; height: 180px; border-radius: 6px; overflow: hidden; background: #e8e8ec; flex: none; }
  .tile.chosen { outline: 3px solid #2f6fed; outline-offset: -3px; }
  .tile img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
  .open { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: none; cursor: pointer; }
  .open:focus-visible { outline: 3px solid #2f6fed; outline-offset: -3px; }
  .pick { position: absolute; z-index: 1; left: 0; top: 0; width: 46px; height: 46px; padding: 0; border: 0; background: none; cursor: pointer; }
  .tk { position: absolute; left: 8px; top: 8px; width: 22px; height: 22px; box-sizing: border-box; display: grid; place-items: center; border-radius: 50%; border: 2px solid #fff; background: #2f6fed; box-shadow: 0 1px 3px rgba(0, 0, 0, .5); color: #fff; opacity: 0; }
  .tk svg { width: 14px; height: 14px; }
  /* Like the grid: the marker shows while the mouse is over the corner (here it takes the photo off, so it turns red). */
  .pick:hover .tk, .pick:focus-visible .tk { opacity: 1; }
  .pick:hover .tk { background: #b3261e; }
</style>
