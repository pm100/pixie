<script lang="ts">
  /** The Collage tool (Create > Collage): the page in the middle, the Working-on photos on the right, and the settings behind the gear. The
   *  collage is kept as you work and is there when you come back; Save writes the page as a PNG or JPEG. */
  import { save } from '@tauri-apps/plugin-dialog';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { collageTemplates, getCollage, getPhoto, saveCollage, setCollage, type CollageTemplate, type Photo } from './api';
  import { defaultSavePath, pagePx, sizeProblem } from './collage';
  import CollageList from './CollageList.svelte';
  import CollagePage from './CollagePage.svelte';
  import CollageSettings from './CollageSettings.svelte';
  import { docDpi, newDoc, pageMm, paperLabel, parseDoc, serializeDoc, toSaveArgs, type CollageDoc, type CollageFrame } from './collageDoc';
  import { bringToFront, firstFreeSpot, newFrameRect, replacePhoto, sendToBack } from './collageFrames';
  import { AUTO_LAYOUT_MAX } from './collageLayout';
  import { framesFromTemplate, makeFrame, relayoutFrames } from './collageStart';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { plural } from './format';
  import { loadPref, savePref } from './prefs';
  import Resizer from './Resizer.svelte';

  let { photos, active, skippedVideos = 0, defaultDir = null }: {
    /** The photos the tools work on (videos already taken out). */
    photos: Photo[];
    active: boolean;
    /** How many videos were left out of `photos`, for the note. */
    skippedVideos?: number;
    /** Where the save dialog starts: the sync folder, or else the first watched folder (null when there is none). */
    defaultDir?: string | null;
  } = $props();

  const locale = typeof navigator === 'undefined' ? undefined : navigator.language;

  let doc = $state<CollageDoc>(newDoc(locale));
  let loaded = $state(false);
  let templates = $state<CollageTemplate[]>([]);
  let selected = $state<string[]>([]);
  let settingsOpen = $state(false);
  let confirm = $state<{ kind: 'clear' } | { kind: 'start'; how: 'template' | 'blank'; id: string | null } | null>(null);
  /** The frames as they were before the last Layout or template, so that it can be undone; and how many times Layout has been pressed since the photos last changed. */
  let beforeLayout = $state<CollageFrame[] | null>(null);
  let layoutPress = 0;
  let saving = $state(false);
  let saved = $state(false);
  let error = $state('');
  let note = $state('');
  /** The width of the photo list on the right, remembered. About the size of the photos in Print. */
  let listW = $state(loadPref('collage.list', 400));
  /** Photos in frames that are not among the Working-on photos, fetched by id (null: no longer in the library). */
  let extra = $state<Record<number, Photo | null>>({});

  onMount(async () => {
    try {
      const [text, t] = await Promise.all([getCollage(), collageTemplates()]);
      doc = parseDoc(text, locale);
      templates = t;
    } catch (e) {
      error = String(e);
    }
    loaded = true;
    if (doc.frames.length === 0) settingsOpen = true;
  });

  const byId = $derived.by(() => {
    const m = new Map<number, Photo>();
    for (const [id, p] of Object.entries(extra)) if (p) m.set(Number(id), p);
    for (const p of photos) m.set(p.id, p);
    return m;
  });

  // A frame may show a photo that is not in the Working-on set: ask for it once (a photo gone from the library stays missing).
  $effect(() => {
    for (const f of doc.frames) {
      if (byId.has(f.photo) || untrack(() => f.photo in extra)) continue;
      extra[f.photo] = null;
      getPhoto(f.photo).then((p) => { extra[f.photo] = p && !p.missing ? p : null; }).catch(() => {});
    }
  });

  // The collage is kept a moment after the last change, and straight away when the tool goes.
  let pending: string | null = null;
  $effect(() => {
    if (!loaded) return;
    const text = serializeDoc(doc);
    pending = text;
    const timer = setTimeout(() => { pending = null; setCollage(text).catch((e) => (error = String(e))); }, 400);
    return () => clearTimeout(timer);
  });
  onDestroy(() => { if (pending) void setCollage(pending).catch(() => {}); });

  const mm = $derived(pageMm(doc));
  const dpi = $derived(docDpi(doc));
  const px = $derived(pagePx(mm.w, mm.h, dpi));
  const problem = $derived(sizeProblem(mm.w, mm.h, dpi));
  const summary = $derived(`${paperLabel(doc)} · ${doc.landscape ? 'landscape' : 'portrait'} · ${px.w.toLocaleString()} × ${px.h.toLocaleString()} px · ${dpi} dpi`);
  const onPage = $derived(new Set(doc.frames.map((f) => f.photo)));
  /** What a template arranges: the photos on the page, or all the Working-on photos while the page is empty. */
  const templatePhotos = $derived(doc.frames.length > 0 ? doc.frames.map((f) => byId.get(f.photo)).filter((p): p is Photo => !!p) : photos);

  const touch = () => { saved = false; };
  const setFrames = (frames: CollageFrame[]) => {
    // (other photos on the page: the old arrangements no longer apply)
    if (frames.length !== doc.frames.length) { layoutPress = 0; beforeLayout = null; }
    doc.frames = frames;
    touch();
  };
  const setDoc = (d: CollageDoc) => { doc = d; touch(); };

  function addPhoto(photoId: number, at?: { x: number; y: number }) {
    const p = byId.get(photoId);
    if (!p) return;
    const rect = newFrameRect(p.width ?? 0, p.height ?? 0, mm, at ?? { x: 0.5, y: 0.5 });
    if (!at) Object.assign(rect, firstFreeSpot(doc.frames, rect));
    const frame = makeFrame(p, rect);
    setFrames([...doc.frames, frame]);
    selected = [frame.id];
  }

  /** A photo from the list was dropped on the page: it is added where it was dropped, on top of any photo there (photos may overlap); with Ctrl
   *  it takes the place of the photo it was dropped on instead. */
  function dropped(photoId: number, at: { x: number; y: number }, onFrame: string | null, replace: boolean) {
    if (onFrame && replace) setFrames(replacePhoto(doc.frames, onFrame, photoId));
    else addPhoto(photoId, at);
  }

  /** Arrange the photos on the page; each press gives another arrangement (see `layoutSuggestion`). */
  function layout() {
    if (doc.frames.length === 0 || doc.frames.length > AUTO_LAYOUT_MAX) return;
    note = '';
    const before = $state.snapshot(doc.frames) as CollageFrame[];
    const next = relayoutFrames(doc.frames, byId, doc, layoutPress);
    layoutPress++;
    setFrames(next);
    beforeLayout = before;
  }

  function undoLayout() {
    if (!beforeLayout) return;
    const back = beforeLayout;
    setFrames(back);
    beforeLayout = null;
  }

  function runStart(how: 'template' | 'blank', id: string | null) {
    note = '';
    error = '';
    if (how === 'blank') {
      setDoc({ ...doc, frames: [], template: null });
      beforeLayout = null;
    } else {
      const made = framesFromTemplate(templates.find((t) => t.id === id)?.cells ?? [], doc, templatePhotos);
      if (made.frames.length === 0) return;
      const before = $state.snapshot(doc.frames) as CollageFrame[]; // an empty page too: Undo layout then empties the page again
      setDoc({ ...doc, frames: made.frames, template: id });
      beforeLayout = before;
      if (made.leftOut > 0) note = `Pixie used the first ${made.frames.length === 1 ? 'photo' : plural(made.frames.length, 'photo')}.`;
    }
    layoutPress = 0;
    selected = [];
    settingsOpen = false;
  }

  function start(how: 'template' | 'blank', id: string | null) {
    if (how === 'blank' && doc.frames.length > 0) confirm = { kind: 'start', how, id };
    else runStart(how, id);
  }

  function confirmed() {
    const c = confirm;
    confirm = null;
    if (c?.kind === 'clear') { setDoc({ ...doc, frames: [], template: null }); selected = []; note = ''; }
    else if (c?.kind === 'start') runStart(c.how, c.id);
  }

  const removeSelected = () => { setFrames(doc.frames.filter((f) => !selected.includes(f.id))); selected = []; };

  function keys(e: KeyboardEvent) {
    if (!active || e.defaultPrevented || settingsOpen || confirm) return;
    const t = e.target as HTMLElement | null;
    if (t && /^(INPUT|SELECT|TEXTAREA)$/.test(t.tagName)) return;
    if ((e.key === 'Delete' || e.key === 'Backspace') && selected.length > 0) { e.preventDefault(); removeSelected(); }
    else if (e.key === ']' && selected.length > 0) setFrames(bringToFront(doc.frames, selected));
    else if (e.key === '[' && selected.length > 0) setFrames(sendToBack(doc.frames, selected));
    else if (e.key === 'Escape' && selected.length > 0) selected = [];
  }

  async function saveIt() {
    if (saving || problem || doc.frames.length === 0) return;
    error = '';
    saved = false;
    const gone = doc.frames.filter((f) => !byId.has(f.photo)).length;
    if (gone > 0) { error = `${plural(gone, 'frame')} show${gone === 1 ? 's' : ''} a photo that is no longer in the library. Remove ${gone === 1 ? 'it' : 'them'} first.`; return; }
    const dst = await save({ defaultPath: defaultSavePath(defaultDir, new Date()), filters: [{ name: 'PNG picture', extensions: ['png'] }, { name: 'JPEG picture', extensions: ['jpg', 'jpeg'] }] });
    if (!dst) return; // cancelled: nothing to say
    saving = true;
    try {
      await saveCollage(toSaveArgs(doc), dst);
      saved = true;
    } catch (e) {
      error = String(e);
    }
    saving = false;
  }
</script>

<svelte:window onkeydown={keys} />

<div class="tool">
  <div class="mid">
    <div class="bar">
      <button class="gear" aria-label="Settings" title="Open the collage settings: how to start, the page size, and how the photos look" onclick={() => (settingsOpen = true)}>⚙</button>
      <span class="summary">{summary}</span>
      <label class="check"><input type="checkbox" checked={doc.snap} onchange={(e) => setDoc({ ...doc, snap: e.currentTarget.checked })} title="Snapping: a photo moved into a free space fills it, and edges line up with the page and with other photos; hold Alt to skip it for one move" /> Snap</label>
      <span class="grow"></span>
      <button onclick={layout} disabled={doc.frames.length === 0 || doc.frames.length > AUTO_LAYOUT_MAX}
              title={doc.frames.length > AUTO_LAYOUT_MAX ? `Layout arranges up to ${AUTO_LAYOUT_MAX} photos: take some off the page first` : 'Arrange the photos on the page for you; press it again for another arrangement'}>Layout</button>
      <button onclick={undoLayout} disabled={!beforeLayout} title="Put the photos back as they were before the last Layout">Undo layout</button>
      <button class="danger" onclick={() => (confirm = { kind: 'clear' })} disabled={doc.frames.length === 0} title="Take every photo off the page and start again">Clear</button>
      <button class="primary" onclick={saveIt} disabled={saving || !!problem || doc.frames.length === 0} title="Choose where to save the collage as a PNG or JPEG picture">{saving ? 'Saving…' : 'Save'}</button>
    </div>

    <div class="pickbar">
      <span class="what">{selected.length === 0 ? 'Click a photo on the page to work on it' : selected.length === 1 ? 'Selected photo' : `${selected.length} photos selected`}</span>
      <button disabled={selected.length === 0} onclick={() => setFrames(bringToFront(doc.frames, selected))} title="Put the chosen photos in front of the others (the ] key)">To front</button>
      <button disabled={selected.length === 0} onclick={() => setFrames(sendToBack(doc.frames, selected))} title="Put the chosen photos behind the others (the [ key)">To back</button>
      <button class="danger" disabled={selected.length === 0} onclick={removeSelected} title="Take the chosen photos off the page; they stay in your library (the Delete key)">Remove</button>
    </div>

    <CollagePage {doc} photos={byId} {selected} {active} onselect={(ids) => (selected = ids)} onchange={setFrames} ondrop={dropped} />

    <div class="status">
      {#if problem}<span class="err" role="alert">{problem}</span>{/if}
      {#if error}<span class="err" role="alert">{error}</span>{/if}
      {#if saved}<span class="ok" role="status">Saved.</span>{/if}
      {#if note}<span class="note">{note}</span>{/if}
      {#if skippedVideos > 0}<span class="note">{plural(skippedVideos, 'video')} can't go in a collage and {skippedVideos === 1 ? 'was' : 'were'} left out.</span>{/if}
      <span class="hint">Drag to move (it fills the space you put it in) · drag a handle to resize · double-click a photo to crop it · Ctrl-drop onto another photo to swap (from the list: to replace it) · Alt skips snapping</span>
    </div>
  </div>

  <div class="split"><Resizer bind:width={listW} min={200} max={800} side="right" onchange={(w) => savePref('collage.list', w)} /></div>

  <div class="side" style:width="{listW}px">
    <CollageList {photos} {onPage} onadd={(id) => addPhoto(id)} />
  </div>
</div>

{#if settingsOpen && loaded}
  <CollageSettings {doc} {templates} locale={locale ?? ''} photoCount={templatePhotos.length} onchange={setDoc} onstart={start} ondone={() => (settingsOpen = false)} />
{/if}

{#if confirm}
  <ConfirmDialog title={confirm.kind === 'clear' ? 'Clear the collage?' : 'Replace the page?'}
                 message={confirm.kind === 'clear' ? 'The photos stay in your library.' : 'The photos on the page now will be taken off it. They stay in your library.'}
                 confirmLabel={confirm.kind === 'clear' ? 'Clear' : 'Replace'} danger onconfirm={confirmed} oncancel={() => (confirm = null)} />
{/if}

<style>
  /* (overflow: hidden, or anything wider than the area makes the box around the tool grow a scrollbar of its own) */
  .tool { position: absolute; inset: 0; display: flex; min-width: 0; overflow: hidden; }
  .mid { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .side { flex: none; min-height: 0; }
  .split { flex: none; width: 5px; display: grid; }
  .bar { display: flex; align-items: center; gap: 8px; padding: 6px 12px; background: #fff; border-bottom: 1px solid #e4e4e8; }
  .bar > button, .bar > label { flex: none; }
  .gear { font-size: 18px; line-height: 1; padding: 3px 8px; }
  .pickbar { display: flex; align-items: center; gap: 8px; padding: 5px 12px; background: #fafafb; border-bottom: 1px solid #e4e4e8; }
  .what { color: #555; font-size: 13px; margin-right: 4px; }
  /* (the summary gives way, so the buttons never move to another line) */
  .summary { color: #555; font-size: 13px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .check { display: inline-flex; align-items: center; gap: 4px; font-size: 13px; color: #333; }
  .grow { flex: 1; }
  .status { display: flex; flex-wrap: wrap; gap: 4px 14px; padding: 5px 12px; background: #fff; border-top: 1px solid #e4e4e8; font-size: 12px; color: #777; }
  .err { color: #b00020; }
  .ok { color: #2a7a2a; }
  .note { color: #8a6d00; }
</style>
