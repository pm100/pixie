<script lang="ts">
  /** The Photomosaic tool (Create > Photomosaic): rebuild the photo chosen in Browse, or part of it, out of many small photos. "Tiles from" at the
   *  top says which photos the tiles come from; the settings are on the left, and the middle shows a live draft. Build makes the real picture from
   *  the original photos, which can then be looked at close up and saved. */
  import { save } from '@tauri-apps/plugin-dialog';
  import { onDestroy, onMount } from 'svelte';
  import {
    getMosaic, getMosaicBuilt, getPhoto, mosaicBuild, mosaicCancel, mosaicClear, mosaicDraft, mosaicIndex, mosaicSave, mosaicStatus, onMosaicBuild, onMosaicIndex, onMosaicPreview, setMosaic,
    type MosaicArgs, type MosaicBuilt, type MosaicDraft, type MosaicProgress, type MosaicSource, type Photo, type UserTagCount, type View,
  } from './api';
  import { focusFromDrag, overflow, pagePx, sizeProblem } from './collage';
  import { paperGroups } from './collagePapers';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { plural } from './format';
  import { isStale, mosaicDpi, mosaicPageMm, mosaicSavePath, MAX_GAP, MAX_TILE_MM, MIN_TILE_MM, parseMosaicDoc, serializeMosaicDoc, sourceLine, tileCount, type MosaicDoc } from './mosaic';
  import MosaicViewer from './MosaicViewer.svelte';
  import { thumbUrl } from './url';
  import WorkingOn from './WorkingOn.svelte';

  let { photo, browse, pickedCount, userTags, filteredCount, onclear, defaultDir = null, active }: {
    /** The photo chosen in Browse, if any: the photo the mosaic rebuilds. */
    photo: Photo | null;
    /** The listing Browse shows now (for "Filtered"). */
    browse: View;
    /** How many photos are picked, and take them all off Picked. */
    pickedCount: number;
    onclear: () => void;
    /** Your tags with their counts. */
    userTags: UserTagCount[];
    /** How many photos Browse shows now. */
    filteredCount: number;
    /** Where the save dialog starts: the sync folder, or else the first watched folder (null when there is none). */
    defaultDir?: string | null;
    active: boolean;
  } = $props();

  const locale = typeof navigator === 'undefined' ? undefined : navigator.language;

  let doc = $state<MosaicDoc>(parseMosaicDoc(null, locale));
  let loaded = $state(false);
  let draft = $state<MosaicDraft | null>(null);
  let built = $state<MosaicBuilt | null>(null);
  let view = $state<'draft' | 'built'>('draft');
  let indexing = $state<MosaicProgress | null>(null);
  let building = $state<MosaicProgress | null>(null);
  let drafting = $state(false);
  /** Something started before the window was last away is still running: the window only shows how far it is, and waits. */
  let external = $state(false);
  /** The mosaic so far, while it is being built (a small JPEG, base64). */
  let preview = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state('');
  let note = $state('');
  let confirmNew = $state(false);
  /** The size of the target's picture, for placing it in the crop box (only its shape matters). */
  let shape = $state({ w: 4, h: 3 });

  const groups = paperGroups(locale);
  const mm = $derived(mosaicPageMm(doc));
  const dpi = $derived(mosaicDpi(doc));
  const px = $derived(pagePx(mm.w, mm.h, dpi));
  const problem = $derived(sizeProblem(mm.w, mm.h, dpi));
  const count = $derived(tileCount(mm.w, mm.h, doc.tileMm, dpi));
  const stale = $derived(isStale(built, draft?.key ?? null));
  const busy = $derived(indexing !== null || building !== null);

  /** The photo to rebuild is the one chosen in Browse (it must be a picture). Browse lets go of its choice when you change a filter, so the photo is
   *  remembered (here, and in the settings) until another is chosen. */
  const chosen = $derived(photo && photo.kind === 'photo' ? photo : null);
  let kept = $state<Photo | null>(null);
  const target = $derived(chosen ?? kept);
  $effect(() => { if (chosen) kept = chosen; });
  // After a restart, or when Browse has nothing chosen: the photo the settings were made for.
  $effect(() => {
    const id = doc.target;
    if (!loaded || chosen || id === null || kept?.id === id) return;
    getPhoto(id).then((p) => { if (p && !p.missing && p.kind === 'photo' && doc.target === p.id) kept = p; }).catch(() => {});
  });
  /** How many photos "Tiles from" gives. */
  const poolCount = $derived(doc.source === 'picked' ? pickedCount : doc.source.startsWith('tag:') ? (userTags.find((t) => `tag:${t.name}` === doc.source)?.count ?? 0) : filteredCount);

  function sourceArg(source: string): MosaicSource {
    if (source === 'picked') return { which: 'picked' };
    if (source.startsWith('tag:')) return { which: 'tag', tag: source.slice(4) };
    return { which: 'browse', order: browse.order, folder_id: browse.folderId, kind: browse.kind, bounds: browse.bounds, max_id: browse.maxId, since: browse.since, until: browse.until, search: browse.search };
  }
  const args = $derived<MosaicArgs | null>(
    target === null || problem
      ? null
      : { source: sourceArg(doc.source), target: target.id, focus_x: doc.focusX, focus_y: doc.focusY, zoom: doc.zoom, paper_w_mm: mm.w, paper_h_mm: mm.h, dpi, tile_mm: doc.tileMm, gap: doc.gap, blend: doc.blend },
  );

  // ---- keeping the settings ----

  let unlisten: (() => void)[] = [];
  onMount(async () => {
    try {
      doc = parseMosaicDoc(await getMosaic(), locale);
      built = await getMosaicBuilt();
      if (built) view = 'built';
    } catch (e) {
      error = String(e);
    }
    unlisten = [await onMosaicIndex((p) => { if (indexing) indexing = p; }), await onMosaicBuild((p) => { if (building) building = p; }), await onMosaicPreview((j) => { if (building) preview = j; })];
    await resume();
    loaded = true;
  });

  // The tool is closed when you go to another tab, but a build goes on: when it opens again, pick up where things are and wait for the end.
  let poll: ReturnType<typeof setInterval> | undefined;
  async function resume() {
    const s = await mosaicStatus().catch(() => null);
    if (!s || s.phase === 'idle') return;
    external = true;
    const was = s.phase;
    const show = (p: MosaicProgress) => { if (was === 'build') building = p; else indexing = p; };
    show({ done: s.done, total: s.total });
    poll = setInterval(async () => {
      const now = await mosaicStatus().catch(() => null);
      if (now && now.phase === was) { show({ done: now.done, total: now.total }); return; }
      clearInterval(poll);
      poll = undefined;
      building = null;
      indexing = null;
      preview = null;
      if (was === 'build') {
        built = await getMosaicBuilt().catch(() => null);
        if (built) view = 'built';
      }
      external = false;
    }, 700);
  }

  let pending: string | null = null;
  $effect(() => {
    if (!loaded) return;
    const text = serializeMosaicDoc(doc);
    pending = text;
    const timer = setTimeout(() => { pending = null; setMosaic(text).catch((e) => (error = String(e))); }, 400);
    return () => clearTimeout(timer);
  });
  onDestroy(() => {
    if (pending) void setMosaic(pending).catch(() => {});
    unlisten.forEach((f) => f());
    if (poll) clearInterval(poll);
  });

  // The crop belongs to the photo it was made for: another photo chosen in Browse starts again from the middle.
  $effect(() => {
    const id = chosen?.id ?? null;
    if (!loaded || id === null || id === doc.target) return;
    doc = { ...doc, target: id, focusX: 0.5, focusY: 0.5, zoom: 1 };
  });
  // A tag that has no photos left is gone from the list: go back to what Browse shows.
  $effect(() => {
    if (loaded && doc.source.startsWith('tag:') && !userTags.some((t) => `tag:${t.name}` === doc.source)) doc = { ...doc, source: 'filtered' };
  });

  // ---- the draft ----

  let gen = 0;
  let prepared = '';
  let preparing: Promise<void> | null = null;

  /** Make the small pictures for the photos the tiles come from, once for each source. */
  async function prepare(source: MosaicSource) {
    const key = JSON.stringify(source);
    if (prepared === key) return;
    while (preparing) await preparing.catch(() => {});
    if (prepared === key) return;
    indexing = { done: 0, total: 0 };
    preparing = mosaicIndex(source).then(() => { prepared = key; }).finally(() => { preparing = null; indexing = null; });
    await preparing;
  }

  async function redraw(a: MosaicArgs, mine: number) {
    drafting = true;
    try {
      await prepare(a.source);
      if (mine !== gen) return;
      const d = await mosaicDraft(a);
      if (mine !== gen) return;
      draft = d;
      error = '';
    } catch (e) {
      if (mine !== gen) return;
      const text = String(e);
      if (text.includes('Cancelled.')) note = 'Preparing the photos was stopped. Change something to start again.';
      else { error = text; draft = null; }
    }
    if (mine === gen) drafting = false;
  }

  $effect(() => {
    if (!loaded || !active || args === null || external) return;
    const a = args;
    const mine = ++gen;
    const timer = setTimeout(() => void redraw(a, mine), 250);
    return () => clearTimeout(timer);
  });

  // ---- build and save ----

  async function build() {
    if (!args || busy) return;
    const a = args;
    error = ''; note = ''; saved = false;
    building = { done: 0, total: 0 };
    try {
      await prepare(a.source);
      building = { done: 0, total: 0 };
      built = await mosaicBuild(a);
      view = 'built';
    } catch (e) {
      const text = String(e);
      if (text.includes('Cancelled.')) note = built ? 'Stopped. The picture built before is still there.' : 'Stopped.';
      else error = text;
    }
    building = null;
    preview = null;
  }

  const stop = () => { void mosaicCancel(); };

  /** What is going on, in words: a build first chooses a photo for every tile, then opens each chosen photo once and places it, then writes the picture. */
  function stepLine(building: boolean, p: MosaicProgress | null): string {
    if (!p || p.total === 0) return building ? 'Choosing a photo for every tile…' : 'Looking for photos that have not been prepared…';
    if (!building) return `${p.done.toLocaleString()} of ${p.total.toLocaleString()} photos prepared`;
    if (p.done >= p.total) return 'Writing the picture and the views for zooming…';
    return `Placing photos: ${p.done.toLocaleString()} of ${p.total.toLocaleString()} (each photo is opened once, and used in every tile it fills)`;
  }

  async function saveIt() {
    if (!built || stale || saving) return;
    error = ''; saved = false;
    const dst = await save({ defaultPath: mosaicSavePath(defaultDir, new Date()), filters: [{ name: 'PNG picture', extensions: ['png'] }, { name: 'JPEG picture', extensions: ['jpg', 'jpeg'] }] });
    if (!dst) return; // cancelled: nothing to say
    saving = true;
    try {
      await mosaicSave(dst);
      saved = true;
    } catch (e) {
      error = String(e);
    }
    saving = false;
  }

  // ---- choosing the part of the photo ----

  const BOX = 260;
  const box = $derived.by(() => {
    const k = Math.min(BOX / mm.w, BOX / mm.h);
    return { w: Math.max(40, Math.round(mm.w * k)), h: Math.max(40, Math.round(mm.h * k)) };
  });
  const shown = $derived.by(() => {
    const s = Math.max(box.w / shape.w, box.h / shape.h) * doc.zoom;
    const [w, h] = [shape.w * s, shape.h * s];
    return { w, h, left: -(w - box.w) * doc.focusX, top: -(h - box.h) * doc.focusY };
  });

  let drag: { x: number; y: number; fx: number; fy: number } | null = null;
  function down(e: PointerEvent) {
    drag = { x: e.clientX, y: e.clientY, fx: doc.focusX, fy: doc.focusY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const f = focusFromDrag({ fx: drag.fx, fy: drag.fy }, { dx: e.clientX - drag.x, dy: e.clientY - drag.y }, overflow(shape.w, shape.h, box.w, box.h, doc.zoom));
    doc = { ...doc, focusX: f.fx, focusY: f.fy };
  }
  const up = () => { drag = null; };

  /** A new mosaic: the settings go back to how they start (the choice of tiles stays) and the picture that was built is thrown away. */
  async function startNew() {
    confirmNew = false;
    error = ''; note = ''; saved = false;
    try {
      await mosaicClear();
      doc = { ...parseMosaicDoc(null, locale), source: doc.source, target: target?.id ?? null };
      built = null; draft = null; preview = null; view = 'draft';
    } catch (e) {
      error = String(e);
    }
  }
  const setNum = (e: Event, lo: number, hi: number, dflt: number): number => {
    const n = Number((e.currentTarget as HTMLInputElement).value);
    return Number.isFinite(n) ? Math.min(hi, Math.max(lo, n)) : dflt;
  };
  const summary = $derived(`${px.w.toLocaleString()} × ${px.h.toLocaleString()} px · ${dpi} dpi · ${plural(count.tiles, 'tile')} (${count.cols} × ${count.rows})`);
  const saveTitle = $derived(!built ? 'Build the picture first' : stale ? 'Build again first: the settings have changed since the picture was built' : 'Choose where to save the mosaic as a PNG or JPEG picture');
</script>

<div class="tool">
  <WorkingOn tiles bind:source={doc.source} {pickedCount} hasPhoto={photo !== null} {userTags} count={poolCount} {onclear} />
  <div class="body">
  <div class="opts">
    <section>
      <h4>Photo to rebuild</h4>
      <div class="crop" style:width="{box.w}px" style:height="{box.h}px" role="img" aria-label="The part of the photo the mosaic will show; drag to move it"
           title="Drag to choose which part of the photo the mosaic shows"
           onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}>
        {#if target}
          <img alt="" draggable="false" src={thumbUrl('view', target.id, target.mtime, target.size)}
               onload={(e) => { const i = e.currentTarget as HTMLImageElement; shape = { w: i.naturalWidth || 4, h: i.naturalHeight || 3 }; }}
               style:width="{shown.w}px" style:height="{shown.h}px" style:left="{shown.left}px" style:top="{shown.top}px" />
        {:else}
          <span class="none">No photo yet</span>
        {/if}
      </div>
      <label>Zoom
        <input type="range" min="1" max="4" step="0.05" value={doc.zoom} oninput={(e) => (doc = { ...doc, zoom: setNum(e, 1, 4, doc.zoom) })} title="Zoom in on the photo to rebuild only part of it" />
      </label>
      <p class="hint">The mosaic rebuilds the photo chosen in Browse, and keeps it while you change what Browse shows.</p>
    </section>

    <section>
      <h4>Picture</h4>
      <label>Paper
        <select value={doc.paper} onchange={(e) => (doc = { ...doc, paper: e.currentTarget.value })} title="The size of the picture the mosaic is made for">
          {#each groups as g (g.label)}
            <optgroup label={g.label}>{#each g.sizes as p (p.id)}<option value={p.id}>{p.label}</option>{/each}</optgroup>
          {/each}
        </select>
      </label>
      <label class="check"><input type="checkbox" checked={doc.landscape} onchange={(e) => (doc = { ...doc, landscape: e.currentTarget.checked })} title="Make the picture wide instead of tall" /> Landscape</label>
      <label>Tile size (mm)
        <input type="number" min={MIN_TILE_MM} max={MAX_TILE_MM} step="1" value={doc.tileMm} onchange={(e) => (doc = { ...doc, tileMm: setNum(e, MIN_TILE_MM, MAX_TILE_MM, doc.tileMm) })} title="The width of each small photo in millimetres: smaller tiles give a finer picture but you cannot make out the photos" />
      </label>
    </section>

    <section>
      <h4>Look</h4>
      <label>Keep repeats apart (tiles)
        <input type="number" min="1" max={MAX_GAP} step="1" value={doc.gap} onchange={(e) => (doc = { ...doc, gap: Math.round(setNum(e, 1, MAX_GAP, doc.gap)) })} title="A photo is not used again within this many tiles of itself, as far as there are enough photos" />
      </label>
      <label>Blend with target ({Math.round(doc.blend * 100)}%)
        <input type="range" min="0" max="1" step="0.05" value={doc.blend} oninput={(e) => (doc = { ...doc, blend: setNum(e, 0, 1, doc.blend) })} title="Tint each tile towards the colour of the picture where it sits: 0 leaves the tiles as they are, more gives a clearer picture" />
      </label>
    </section>
  </div>

  <div class="mid">
    <div class="bar">
      <span class="summary">{summary}</span>
      <span class="grow"></span>
      <button class="danger" onclick={() => (confirmNew = true)} disabled={busy || (!built && !draft)} title="Start a new mosaic: the settings go back to how they start and the built picture is thrown away. Your photos are not touched">New mosaic</button>
      <button class:on={view === 'draft'} onclick={() => (view = 'draft')} title="Show the quick draft that follows your settings">Draft</button>
      <button class:on={view === 'built'} disabled={!built} onclick={() => (view = 'built')} title={built ? 'Look at the picture that was built, and zoom in on it' : 'Nothing has been built yet'}>Built picture</button>
      {#if busy}
        <button class="danger" onclick={stop} title="Stop after the photo being worked on; the picture built before stays">Cancel</button>
      {/if}
      <button class:primary={!built || stale} onclick={build} disabled={busy || !args || !draft} title={!args ? 'Choose a photo in Browse first: it is the photo the mosaic rebuilds' : 'Make the full-size picture from the original photos; this can take a while'}>{building ? 'Building…' : 'Build'}</button>
      <button class:primary={!!built && !stale} onclick={saveIt} disabled={!built || stale || saving || busy} title={saveTitle}>{saving ? 'Saving…' : 'Save'}</button>
    </div>

    <div class="stage">
      {#if busy}
        {@const p = building ?? indexing}
        <div class="working" role="status">
          <div class="card">
            <b>{building ? 'Building the picture' : 'Preparing photos'}</b>
            <progress max={p && p.total > 0 ? p.total : undefined} value={p && p.total > 0 ? p.done : undefined} aria-label="How far it has got"></progress>
            <span>{stepLine(!!building, p)}</span>
            <span class="hint">{building ? 'You can leave this tab: it carries on.' : 'The first time takes a while; later mosaics start straight away.'}</span>
          </div>
        </div>
      {/if}
      {#if building && preview}
        <img class="draft" alt="The mosaic as it is being built" src={`data:image/jpeg;base64,${preview}`} />
      {:else if view === 'built' && built}
        <MosaicViewer {built} />
      {:else if draft}
        <img class="draft" alt="A quick draft of the mosaic" src={`data:image/jpeg;base64,${draft.jpeg_base64}`} />
      {:else if drafting}
        <p class="msg" role="status">Making the draft…</p>
      {:else}
        <p class="msg">{args === null ? (problem ?? (photo && !chosen ? 'The photo chosen in Browse is a video. Choose a picture: the mosaic rebuilds it.' : 'Choose a photo in Browse: the mosaic rebuilds it.')) : (error || '')}</p>
      {/if}
    </div>

    <div class="status">
      {#if problem}<span class="err" role="alert">{problem}</span>{/if}
      {#if error}<span class="err" role="alert">{error}</span>{/if}
      {#if saved}<span class="ok" role="status">Saved.</span>{/if}
      {#if note}<span class="note">{note}</span>{/if}
      {#if draft}
        <span>{sourceLine(draft.tiles, draft.photos)}</span>
        {#if draft.gap_used === 0}<span class="note">There are too few photos to keep repeats apart.</span>
        {:else if draft.gap_used < doc.gap}<span class="note">There are not enough photos to keep repeats {doc.gap} tiles apart: they are at least {plural(draft.gap_used, 'tile')} apart.</span>{/if}
        {#if draft.skipped > 0}<span class="note">{plural(draft.skipped, 'photo')} could not be read and {draft.skipped === 1 ? 'was' : 'were'} left out.</span>{/if}
        {#if built && stale}<span class="note">The settings have changed since the picture was built: build it again to save.</span>{/if}
      {/if}
    </div>
  </div>
  </div>
</div>

{#if confirmNew}
  <ConfirmDialog title="Start a new mosaic?" message="The settings go back to how they start and the picture that was built is thrown away. Your photos are not touched."
                 confirmLabel="New mosaic" danger onconfirm={() => void startNew()} oncancel={() => (confirmNew = false)} />
{/if}

<style>
  .tool { position: absolute; inset: 0; display: flex; flex-direction: column; min-width: 0; overflow: hidden; }
  .body { flex: 1; min-height: 0; display: flex; min-width: 0; }
  .opts { flex: none; width: 310px; overflow-y: auto; padding: var(--page-pad); border-right: 1px solid #e4e4e8; background: #fff; box-sizing: border-box; }
  .mid { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  section { margin-bottom: 14px; }
  h4 { margin: 0 0 6px; font-size: 13px; color: #444; }
  label { display: flex; flex-direction: column; gap: 3px; font-size: 13px; color: #333; margin-bottom: 8px; }
  label.check { flex-direction: row; align-items: center; gap: 6px; }
  select, input[type='number'] { padding: 3px 6px; }
  .crop { position: relative; overflow: hidden; background: #e8e8ec; border-radius: 4px; margin-bottom: 8px; cursor: grab; touch-action: none; }
  .crop img { position: absolute; max-width: none; user-select: none; }
  .none { position: absolute; inset: 0; display: grid; place-items: center; color: #777; font-size: 13px; }
  .bar { display: flex; align-items: center; gap: 8px; padding: 6px 12px; background: #fff; border-bottom: 1px solid #e4e4e8; }
  .bar > button { flex: none; }
  .bar button.on { background: #e8f0fe; border-color: #2f6fed; }
  .summary { color: #555; font-size: 13px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grow { flex: 1; }
  .working { position: absolute; left: 0; right: 0; bottom: 14px; z-index: 5; display: grid; place-items: center; pointer-events: none; }
  .card { display: grid; gap: 8px; justify-items: center; text-align: center; padding: 18px 24px; background: #fff; border: 1px solid #cfdcf8; border-radius: 8px; box-shadow: 0 2px 10px rgba(0, 0, 0, .12); font-size: 13px; color: #234; max-width: 46ch; }
  .card progress { width: 320px; max-width: 100%; }
  .stage { position: relative; flex: 1; min-height: 0; background: #f4f4f6; display: grid; place-items: center; overflow: hidden; }
  .draft { max-width: 100%; max-height: 100%; object-fit: contain; image-rendering: auto; }
  .msg { color: #666; font-size: 14px; text-align: center; max-width: 46ch; margin: 0; }
  .hint { color: #888; font-size: 12px; }
  .status { display: flex; flex-wrap: wrap; gap: 4px 14px; padding: 5px 12px; background: #fff; border-top: 1px solid #e4e4e8; font-size: 12px; color: #777; }
  .err { color: #b00020; }
  .ok { color: #2a7a2a; }
  .note { color: #8a6d00; }
</style>
