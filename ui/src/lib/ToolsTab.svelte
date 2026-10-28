<script lang="ts">
  /** The Tools tab, laid out like Settings: the tools down the left, the options of the chosen tool (by default the details of the photo chosen on the right) in the middle, and
   *  the photos of Picked (or of one of your tags) on the right. */
  import { untrack } from 'svelte';
  import { listPicked, listTagged, type Photo, type UserTagCount, type View } from './api';
  import Details from './Details.svelte';
  import EmailForm from './EmailForm.svelte';
  import { loadPref, savePref } from './prefs';
  import Resizer from './Resizer.svelte';
  import { plural } from './format';
  import GroupTags from './GroupTags.svelte';
  import PrintTool from './PrintTool.svelte';
  import { posterUrl, thumbUrl } from './url';
  import Viewer from './Viewer.svelte';

  let { active, picks, onunpick, onclear, emailLimitMb, userTags, onchange, start = null }: {
    active: boolean;
    /** Given when the tab is opened to work on one photo (the Email and Print buttons in Browse): that photo, and the tool. */
    start?: { photo: Photo; page: 'email' | 'print' } | null;
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
  } = $props();

  // The widths of the tools list and of the options column; the photos take the rest. Remembered, as Browse's panes are.
  let navW = $state(loadPref('tools.nav', 230));
  let sideW = $state(loadPref('tools.side', 300));

  // What the middle column shows. Each tool is added to this list when it has been built.
  type Page = 'details' | 'email' | 'print' | 'tags';
  const pages: { id: Page; label: string }[] = [{ id: 'details', label: 'Photo details' }, { id: 'email', label: 'Email' }, { id: 'print', label: 'Print' }, { id: 'tags', label: 'Tags' }];
  let page = $state<Page>(untrack(() => start?.page ?? 'details'));

  /** One photo the tools can work on by itself (the Current photo choice): the one the tab was opened for, or one whose Email
   *  or Print button was pressed here. */
  let current = $state<Photo | null>(untrack(() => start?.photo ?? null));
  /** Which photos the tools work on: `picked`, `current`, or `tag:<name>` for one of your tags. */
  let source = $state(untrack(() => (start ? 'current' : 'picked')));
  // A tag that has no photos left is gone from the list: go back to Picked.
  $effect(() => { if (source.startsWith('tag:') && !userTags.some((t) => `tag:${t.name}` === source)) source = 'picked'; });
  const isPicked = $derived(source === 'picked');

  let photos = $state<Photo[]>([]);
  let error = $state('');
  let broken = $state<Record<number, boolean>>({});
  /** The Email or Print button of the photo chosen here: work on that one photo with that tool. */
  function workOn(tool: 'email' | 'print') {
    if (!chosenPhoto) return;
    current = chosenPhoto;
    source = 'current';
    page = tool;
  }
  /** The photo chosen in the middle (one click); its details are what the right pane shows by default. */
  let chosen = $state<number | null>(null);
  const chosenPhoto = $derived(photos.find((p) => p.id === chosen) ?? null);
  /** The photo open in the viewer (its position among these photos), if any. Double-click or Enter opens it. */
  let viewing = $state<number | null>(null);
  // The viewer pages through these photos as they are listed here (it does not use Browse's listing).
  const noListing: View = { order: 'newest', folderId: null, kind: null, bounds: null, maxId: null, since: null, until: null, search: false };
  const pageOfPhotos = (offset: number, limit: number) => Promise.resolve(photos.slice(offset, offset + limit));
  // A photo that leaves the list while it is open: close the viewer rather than show a different one.
  $effect(() => { if (viewing !== null && viewing >= photos.length) viewing = null; });

  $effect(() => {
    picks; userTags; // read again whenever the picks or the tags change
    const from = source;
    if (!active) return;
    if (from === 'current') { photos = current ? [current] : []; return; }
    let live = true;
    (from === 'picked' ? listPicked() : listTagged(from.slice(4))).then((l) => { if (live) photos = l; }).catch((e) => { if (live) error = String(e); });
    return () => { live = false; };
  });

  const ids = $derived(photos.map((p) => p.id));
</script>

<div class="page">
  <div class="bar">
    <label for="working-with">Working on:</label>
    <select id="working-with" bind:value={source}>
      <option value="picked">★ Picked ({picks.size.toLocaleString()})</option>
      {#if current}<option value="current">Current photo</option>{/if}
      {#each userTags as t (t.name)}<option value="tag:{t.name}"># {t.name} ({t.count.toLocaleString()})</option>{/each}
    </select>
    <span class="count">{plural(photos.length, 'photo')}</span>
    {#if isPicked}
      <button class="clear" onclick={onclear} disabled={picks.size === 0} title="Take everything off Picked (the photos stay in the library)">Clear picks</button>
    {/if}
    {#if error}<span class="err" role="alert">{error}</span>{/if}
  </div>
  <div class="body" style:grid-template-columns="{navW}px 5px {sideW}px 5px minmax(0, 1fr)">
  <nav aria-label="Tools">
    {#each pages as p (p.id)}
      <button class:on={page === p.id} aria-current={page === p.id ? 'page' : undefined} onclick={() => (page = p.id)}>{p.label}</button>
    {/each}
  </nav>
  <Resizer bind:width={navW} min={160} max={360} side="left" onchange={(w) => savePref('tools.nav', w)} />

  <aside class="side" aria-label={page === 'details' ? 'Photo details' : pages.find((x) => x.id === page)?.label}>
    {#if page === 'details'}
      <Details photo={chosenPhoto} onemail={() => workOn('email')} onprint={() => workOn('print')} ontagschange={onchange} />
    {:else if page === 'email'}
      <div class="tool wide">
        <EmailForm {ids} limitMb={emailLimitMb} />
      </div>
    {:else if page === 'print'}
      <PrintTool {photos} />
    {:else}
      <div class="tool"><GroupTags {ids} {onchange} /></div>
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
        {:else}
          <p><b>No photos have this tag yet.</b></p>
        {/if}
      </div>
    {:else}
      <div class="grid">
        {#each photos as p, i (p.id)}
          <div class="tile" class:chosen={chosen === p.id}>
            <!-- The whole photo is the button: one click chooses it (its details show), a double click or Enter opens it. The unpick marker is a separate button over its corner. -->
            <button class="open" aria-label={p.kind === 'video' ? `Video ${p.path}` : `Photo ${p.path}`} aria-pressed={chosen === p.id}
                    onclick={() => { chosen = p.id; if (page !== 'details') page = 'details'; }} ondblclick={() => (viewing = i)}
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
                <span class="tk" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7" /></svg></span>
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
  </div>
</div>

{#if viewing !== null}
  <Viewer index={viewing} total={photos.length} view={noListing} page={pageOfPhotos} onclose={() => (viewing = null)} onnav={(i) => (viewing = i)} />
{/if}

<style>
  /* Fills the page over Browse, as the other tabs do; the three columns are laid out like Settings (menu, then the page). */
  .page { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; }
  /* The line across the top, like Browse's: which photos the tools work on. */
  .bar { display: flex; gap: 12px; align-items: center; padding: 6px 14px; border-bottom: 1px solid #eee; font-size: 14px; }
  .bar select { padding: 3px 6px; }
  .count { font-size: 13px; font-weight: 600; color: #333; font-variant-numeric: tabular-nums; }
  .clear { border: 1px solid #c9ceda; background: #fff; border-radius: 4px; padding: 3px 10px; font: inherit; cursor: pointer; }
  .clear:disabled { opacity: .5; cursor: default; }
  .body { flex: 1; min-height: 0; display: grid; }
  nav { background: #fafafb; border-right: 1px solid #e4e4e8; padding: 6px 4px 16px; font-size: 14px; display: grid; align-content: start; gap: 1px; overflow-y: auto; }
  nav button { text-align: left; border: 0; background: none; font: inherit; padding: 4px 12px; border-radius: 4px; cursor: pointer; }
  nav button:hover:not(.on) { background: #eef2fb; }
  nav button.on { background: #e3ecff; font-weight: 600; }
  .mid { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .none { padding: 40px 28px; color: #555; max-width: 520px; line-height: 1.5; }
  .none p { margin: 0 0 12px; }
  .side { min-width: 0; overflow-y: auto; overflow-x: hidden; border-right: 1px solid #e4e4e8; background: #fafafb; }
  .tool { padding: 12px 14px; font-size: 14px; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
  .tool.wide { align-items: stretch; }
  .err { color: #b00020; font-size: 13px; }
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
