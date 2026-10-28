<script lang="ts">
  /** The Tools tab: the photos of Picked (or of one of your tags), shown together, with the tools that work on all of them. */
  import { listPicked, listTagged, type Photo, type UserTagCount, type View } from './api';
  import EmailButton from './EmailButton.svelte';
  import TagAs from './TagAs.svelte';
  import { posterUrl, thumbUrl } from './url';
  import Viewer from './Viewer.svelte';

  let { active, picks, onunpick, onclear, emailLimitMb, userTags, onchange }: {
    active: boolean;
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

  /** Which photos the tools work on: `picked`, or the name of one of your tags. */
  let source = $state('picked');
  // A tag that has no photos left is gone from the list: go back to Picked.
  $effect(() => { if (source !== 'picked' && !userTags.some((t) => t.name === source)) source = 'picked'; });
  const isPicked = $derived(source === 'picked');

  let photos = $state<Photo[]>([]);
  let error = $state('');
  let broken = $state<Record<number, boolean>>({});
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
    let live = true;
    (from === 'picked' ? listPicked() : listTagged(from)).then((l) => { if (live) photos = l; }).catch((e) => { if (live) error = String(e); });
    return () => { live = false; };
  });

  const ids = $derived(photos.map((p) => p.id));

</script>

<div class="tools">
  <div class="bar">
    <label for="working-with"><b>Working with</b></label>
    <select id="working-with" bind:value={source}>
      <option value="picked">★ Picked ({picks.size.toLocaleString()})</option>
      {#each userTags as t (t.name)}<option value={t.name}># {t.name} ({t.count.toLocaleString()})</option>{/each}
    </select>
    {#if error}<span class="err" role="alert">{error}</span>{/if}
    {#if isPicked}
      <button class="clear" onclick={onclear} disabled={picks.size === 0} title="Take everything off Picked (the photos stay in the library)">Clear picks</button>
    {/if}
  </div>
  <div class="body">
  {#if photos.length === 0}
    <div class="none">
      {#if isPicked}
        <p><b>Nothing is picked yet.</b></p>
        <p>In Browse, move the mouse over the top-left corner of a photo and click the circle that appears. Shift-click or Ctrl-click to pick several at once.</p>
        <p>The photos you pick are listed here, and can be emailed or tagged together.</p>
      {:else}
        <p><b>No photos have this tag yet.</b></p>
      {/if}
    </div>
  {:else}
  <div class="grid">
    {#each photos as p, i (p.id)}
      <div class="tile">
        <!-- The whole photo is the button that opens it; the unpick marker is a separate button over its corner. -->
        <button class="open" aria-label={p.kind === 'video' ? `Open video ${p.path}` : `Open ${p.path}`} ondblclick={() => (viewing = i)}
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
  <!-- What can be done with these photos; each tool is added here when it has been built. -->
  <aside class="actions" aria-label="Tools">
    <h3>Tools</h3>
    <EmailButton {ids} limitMb={emailLimitMb} />
    <h3>Tags</h3>
    <TagAs {ids} {onchange} floating={false} />
  </aside>
  </div>
</div>

{#if viewing !== null}
  <Viewer index={viewing} total={photos.length} view={noListing} page={pageOfPhotos} onclose={() => (viewing = null)} onnav={(i) => (viewing = i)} />
{/if}

<style>
  /* Fills the page over Browse, as the other tabs do. */
  .tools { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; }
  .bar { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-bottom: 1px solid #e4e4e8; flex-wrap: wrap; }
  .bar select { padding: 4px 6px; }
  .clear { margin-left: auto; }
  .none { flex: 1; min-width: 0; padding: 40px 28px; color: #555; max-width: 520px; line-height: 1.5; }
  .none p { margin: 0 0 12px; }
  .body { flex: 1; min-height: 0; display: flex; }
  .actions { flex: none; width: 200px; box-sizing: border-box; padding: 12px 14px; border-left: 1px solid #e4e4e8; display: flex; flex-direction: column; align-items: stretch; gap: 8px; overflow-y: auto; }
  .actions h3 { margin: 6px 0 2px; font-size: 13px; color: #777; font-weight: 600; }
  .actions h3:first-child { margin-top: 0; }
  .err { color: #b00020; font-size: 13px; }
  .grid { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-wrap: wrap; align-content: flex-start; gap: 6px; padding: 12px 14px; }
  .tile { position: relative; width: 180px; height: 180px; border-radius: 6px; overflow: hidden; background: #e8e8ec; flex: none; }
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
