<script lang="ts">
  import { untrack } from 'svelte';
  import {
    facesOfPerson, hidePerson, listFacePeople, mergePeople, moveFace, moveFaces, newPersonFromFace, rejectedFaces, rejectFace, rejectFaces, renamePerson, unhidePerson,
  } from './api';
  import { faceCardLine, peopleLine, personLabel, sameNamePerson, type FacePerson, type FaceTile, type PeopleStatus } from './people';
  import { analysisFraction } from './analysis';
  import { createLatest } from './latest';
  import { faceUrl, thumbUrl } from './url';

  let { active, status, edits, onchange, onshow }: {
    /** This tab is the one on screen (it stays built while another tab is shown, so coming back is instant). */
    active: boolean;
    /** How far the face scan has got: shown here, and a change means there is more to see (offered, never forced). */
    status: PeopleStatus | null;
    /** Counts people changes made elsewhere (Browse): the next time this tab is shown it reloads. */
    edits: number;
    /** Something was named, merged, hidden or moved here: the rest of the app should refresh its people. */
    onchange: () => void;
    /** Show this person's photos in Browse. */
    onshow: (id: number) => void;
  } = $props();

  type Selected = { kind: 'person'; id: number } | { kind: 'rejected' } | null;

  let people = $state<FacePerson[]>([]);
  let showHidden = $state(false);
  /** Only people who have no name yet: the ones still to be identified. */
  let onlyUnnamed = $state(false);
  let loaded = $state(false);
  let rejectedCount = $state(0);
  let selected = $state<Selected>(null);
  let tiles = $state<FaceTile[]>([]);
  let error = $state<string | null>(null);
  /** What has been typed in a name box and not saved yet, by person id (cards) or by face id (loose faces). */
  let personDrafts = $state<Record<number, string>>({});
  let faceDrafts = $state<Record<number, string>>({});
  /** Someone was given a name another person already has: waiting for the answer to "merge them?". */
  let pending = $state<{ from: FacePerson; name: string; into: FacePerson } | null>(null);
  /** What the scan had reached, and the outside edits counted, when the list on screen was loaded. */
  let loadedDone = $state<number | null>(null);
  let loadedEdits = $state(0);
  /** Which face (by position in `tiles`) is open in the whole-photo view, if any. */
  let viewing = $state<number | null>(null);
  /** How many of the person's faces are loaded (more on request) and in which order. "Doubtful" puts the faces least
   *  like the rest of the person first: that is where the wrongly grouped ones are. */
  let shown = $state(60);
  let doubtful = $state(true);
  /** Faces ticked for a bulk change. */
  let picked = $state<Set<number>>(new Set());
  const viewed = $derived(viewing === null ? null : tiles[viewing] ?? null);
  const latest = createLatest();
  const scanning = $derived(status !== null && (status.models !== 'ready' || status.done < status.total));
  /** More photos have been scanned since this list was loaded. The list is not changed under the user: they ask. */
  const more = $derived(status !== null && loadedDone !== null && status.done !== loadedDone);
  const fraction = $derived(status ? analysisFraction(status) : null);

  const current = $derived.by(() => {
    const s = selected;
    return s?.kind === 'person' ? people.find((p) => p.id === s.id) ?? null : null;
  });

  async function load() {
    const token = latest.next();
    const sel = selected;
    const doneThen = status?.done ?? null;
    const shownThen = shown;
    const doubtfulThen = doubtful;
    const editsThen = edits;
    try {
      const [list, rejected] = await Promise.all([listFacePeople(showHidden), rejectedFaces(500)]);
      let list2: FaceTile[] = [];
      if (sel?.kind === 'person') list2 = await facesOfPerson(sel.id, shownThen, doubtfulThen);
      else if (sel?.kind === 'rejected') list2 = rejected.slice(0, shownThen);
      if (!latest.isCurrent(token)) return;
      people = list;
      rejectedCount = rejected.length;
      const gone = sel?.kind === 'person' && !list.some((p) => p.id === sel.id); // merged away
      if (gone) selected = null;
      tiles = sel === null || gone ? [] : list2;
      picked = new Set([...picked].filter((id) => tiles.some((t) => t.id === id))); // only faces still on screen stay ticked
      loaded = true;
      loadedDone = doneThen;
      loadedEdits = editsThen;
    } catch (e) {
      if (latest.isCurrent(token)) error = String(e);
    }
  }

  // Load when first shown and when the hidden people are toggled.
  // (`load` reads the scan status and more; only the toggle may trigger it, so the call is untracked.)
  $effect(() => {
    showHidden;
    untrack(load);
  });
  // Edits made in Browse (naming, hiding, merging): reload when the user comes back, not while they are looking elsewhere.
  $effect(() => {
    if (active && loaded && edits !== loadedEdits) untrack(load);
  });

  /** Run a change, then reload this tab and tell the app. */
  async function act(change: () => Promise<unknown>) {
    error = null;
    try { await change(); } catch (e) { error = String(e); return; }
    await load();
    onchange();
  }

  async function open(next: Selected) {
    selected = next;
    tiles = [];
    shown = 60;
    picked = new Set();
    await load();
  }
  const setOrder = (d: boolean) => { doubtful = d; picked = new Set(); load(); };
  const showMore = () => { shown += 120; load(); };
  const toggle = (id: number) => { const n = new Set(picked); if (n.has(id)) n.delete(id); else n.add(id); picked = n; };
  const total = $derived(current ? current.faces : rejectedCount);

  const saveName = (p: FacePerson) => {
    const text = (personDrafts[p.id] ?? p.name ?? '').trim();
    delete personDrafts[p.id];
    if (text === (p.name ?? '')) return;
    const same = sameNamePerson(people, text, p.id);
    if (same) { pending = { from: p, name: text, into: same }; return; } // ask before merging: there is no undo
    return act(() => renamePerson(p.id, text === '' ? null : text));
  };
  const saveFaceName = (t: FaceTile) => {
    const text = (faceDrafts[t.id] ?? '').trim();
    delete faceDrafts[t.id];
    if (text === '') return;
    return act(() => newPersonFromFace(t.id, text));
  };
  async function answer(choice: 'merge' | 'keep' | 'cancel') {
    const q = pending;
    pending = null;
    if (!q || choice === 'cancel') return;
    if (choice === 'merge') await act(() => mergePeople(q.from.id, q.into.id));
    else await act(() => renamePerson(q.from.id, q.name));
    if (choice === 'merge' && selected?.kind === 'person' && selected.id === q.from.id) selected = { kind: 'person', id: q.into.id };
  }
  const close = () => (viewing = null);
  function step(by: number) {
    if (viewing === null || tiles.length === 0) return;
    viewing = (viewing + by + tiles.length) % tiles.length;
  }
  function onkeydown(e: KeyboardEvent) {
    if (viewing === null) return;
    if (e.target instanceof Element && e.target.closest('input, select, textarea')) return; // typing a name
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowLeft') step(-1);
    else if (e.key === 'ArrowRight') step(1);
    else return;
    e.preventDefault();
  }
  // After a face leaves the list (moved or rejected) the one that took its place is shown; past the end, the last; none left, close.
  $effect(() => {
    if (viewing !== null && viewing >= tiles.length) viewing = tiles.length > 0 ? tiles.length - 1 : null;
  });
  const visible = $derived(onlyUnnamed ? people.filter((p) => p.name === null) : people);
  const others = (id: number | null) => people.filter((p) => p.id !== id);
</script>

<svelte:window {onkeydown} />

{#snippet actions(t: FaceTile)}
  {#if current}
    {#if others(current.id).length > 0}
      <select aria-label="Move to another person" onchange={(e) => { const v = Number(e.currentTarget.value); e.currentTarget.value = ''; if (v) act(() => moveFace(t.id, v)); }}>
        <option value="">Move to…</option>
        {#each others(current.id) as o (o.id)}<option value={o.id}>{personLabel(o)}</option>{/each}
      </select>
    {/if}
    <button onclick={() => act(() => rejectFace(t.id))} title="This face is not them">Not this person</button>
  {:else}
    <form onsubmit={(e) => { e.preventDefault(); saveFaceName(t); }}>
      <input value={faceDrafts[t.id] ?? ''} oninput={(e) => (faceDrafts[t.id] = e.currentTarget.value)} placeholder="Name as a new person" aria-label="New person's name" />
    </form>
    {#if people.length > 0}
      <select aria-label="Move to a person" onchange={(e) => { const v = Number(e.currentTarget.value); e.currentTarget.value = ''; if (v) act(() => moveFace(t.id, v)); }}>
        <option value="">Move to…</option>
        {#each people as o (o.id)}<option value={o.id}>{personLabel(o)}</option>{/each}
      </select>
    {/if}
  {/if}
{/snippet}


<div class="faces">
  <!-- The header, status and controls stay put; only the pictures below scroll. -->
  <div class="top">
    <header>
      <h2>People</h2>
      {#if selected !== null}
        <button onclick={() => open(null)}>← All people</button>
      {:else}
        <label class="toggle"><input type="checkbox" bind:checked={onlyUnnamed} /> Only people without a name</label>
        <label class="toggle"><input type="checkbox" bind:checked={showHidden} /> Show hidden people</label>
      {/if}
      {#if error}<span class="err">{error}</span>{/if}
    </header>

    {#if scanning || more}
      <div class="status" role="status">
        <span>{status ? peopleLine(status) : ''}</span>
        {#if fraction !== null && scanning}<progress value={fraction} max="1"></progress>{/if}
        {#if more}<button onclick={load}>Show newly found faces</button>{/if}
      </div>
    {/if}

    {#if pending}
      <div class="notice" role="alert">
        “{pending.name}” is already a person ({faceCardLine(pending.into)}). Are they the same person?
        <button onclick={() => answer('merge')}>Yes, merge them</button>
        <button onclick={() => answer('keep')}>No, keep them separate</button>
        <button onclick={() => answer('cancel')}>Cancel</button>
      </div>
    {/if}

    {#if loaded && selected === null}
      {#if people.length === 0 && rejectedCount === 0}
        <p class="hint">No faces yet. Pixie looks for faces in the background once the face models are downloaded: see Settings, Search.</p>
      {:else}
        <p class="hint">Click a person's faces to see them all. Type a name and press Enter to name them; a name that already exists offers to merge the two.{onlyUnnamed ? ` Showing ${visible.length.toLocaleString()} of ${people.length.toLocaleString()} people.` : ''}</p>
      {/if}
    {:else if loaded}
      <div class="detail-head">
        {#if current}
          <form onsubmit={(e) => { e.preventDefault(); saveName(current); }}>
            <input value={personDrafts[current.id] ?? current.name ?? ''} oninput={(e) => (personDrafts[current.id] = e.currentTarget.value)}
                   onblur={() => saveName(current)} placeholder="Name this person" aria-label="Name" />
          </form>
          <span class="line">{faceCardLine(current)}</span>
        {:else}
          <strong>Faces without a person</strong>
        {/if}
      </div>
      <div class="listbar">
        {#if current}
          <span>Show:</span>
          <button class:on={!doubtful} onclick={() => setOrder(false)} title="The clearest, largest faces first">Best faces first</button>
          <button class:on={doubtful} onclick={() => setOrder(true)} title="The faces least like the rest of this person first: look here for the wrong ones">Most doubtful first</button>
        {/if}
        <span class="sp"></span>
        <span>Showing {tiles.length.toLocaleString()} of {total.toLocaleString()}</span>
        <button onclick={() => (picked = new Set(tiles.map((t) => t.id)))}>Tick all shown</button>
      </div>
      {#if picked.size > 0}
        <div class="notice bulk" role="status">
          <strong>{picked.size} ticked</strong>
          {#if (current ? others(current.id) : people).length > 0}
            <select aria-label="Move the ticked faces to a person" onchange={(e) => { const v = Number(e.currentTarget.value); e.currentTarget.value = ''; const ids = [...picked]; if (v) act(() => moveFaces(ids, v)); }}>
              <option value="">Move them to…</option>
              {#each current ? others(current.id) : people as o (o.id)}<option value={o.id}>{personLabel(o)}</option>{/each}
            </select>
          {/if}
          {#if current}<button onclick={() => { const ids = [...picked]; act(() => rejectFaces(ids)); }} title="None of these is them">Not this person</button>{/if}
          <button onclick={() => (picked = new Set())}>Clear ticks</button>
        </div>
      {/if}
    {/if}
  </div>

  <div class="scroll">
    {#if !loaded}
      <p class="hint">Loading…</p>
    {:else if selected === null}
      <div class="cards">
        {#each visible as p (p.id)}
          <section class="card" class:dim={p.hidden}>
            <button class="samples" onclick={() => open({ kind: 'person', id: p.id })} aria-label={`See all faces of ${personLabel(p)}`}>
              {#each p.samples as f (f)}<img alt="" loading="lazy" decoding="async" src={faceUrl(f, undefined, 160)} />{/each}
            </button>
            <form onsubmit={(e) => { e.preventDefault(); saveName(p); }}>
              <input value={personDrafts[p.id] ?? p.name ?? ''} oninput={(e) => (personDrafts[p.id] = e.currentTarget.value)} onblur={() => saveName(p)}
                     placeholder="Name this person" aria-label="Name" />
            </form>
            <div class="line">{faceCardLine(p)}{p.hidden ? ' · hidden' : ''}</div>
            <div class="actions">
              <button disabled={!p.listed} title={p.listed ? 'Show their photos in Browse' : 'Browse lists named people, and people in two or more photos'} onclick={() => onshow(p.id)}>Show photos</button>
              {#if p.hidden}
                <button onclick={() => act(() => unhidePerson(p.id))}>Unhide</button>
              {:else}
                <button onclick={() => act(() => hidePerson(p.id))}>Hide</button>
              {/if}
            </div>
          </section>
        {/each}
        {#if rejectedCount > 0}
          <section class="card loose">
            <button class="samples text" onclick={() => open({ kind: 'rejected' })}>Faces without a person ({rejectedCount.toLocaleString()})</button>
            <div class="line">Faces you marked “not this person”. Give them a name or move them to someone.</div>
          </section>
        {/if}
      </div>
      {#if onlyUnnamed && visible.length === 0}<p class="hint">Everyone has a name.</p>{/if}
    {:else}
      <div class="tiles">
        {#each tiles as t, i (t.id)}
          <figure class:ticked={picked.has(t.id)}>
            <label class="tick" title="Tick to change several at once"><input type="checkbox" checked={picked.has(t.id)} onchange={() => toggle(t.id)} aria-label="Tick this face" /></label>
            <button class="zoom" onclick={() => (viewing = i)} title="See the whole photo this face is in" aria-label="See the whole photo">
              <img alt="" loading="lazy" decoding="async" src={faceUrl(t.id, undefined, 320)} />
            </button>
            <figcaption>{@render actions(t)}</figcaption>
          </figure>
        {/each}
        {#if tiles.length === 0}<p class="hint">No faces here.</p>{/if}
      </div>
      {#if tiles.length < total}
        <p><button onclick={showMore}>Show more faces</button></p>
      {/if}
    {/if}
  </div>
</div>

{#if viewed}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="lightbox" onclick={(e) => { if (e.target === e.currentTarget) close(); }} role="dialog" tabindex="-1" aria-modal="true" aria-label="The whole photo">
    <div class="lb-bar">
      <button onclick={() => step(-1)} aria-label="Previous face" title="Previous (←)">←</button>
      <span>{(viewing ?? 0) + 1} of {tiles.length}{current ? ` · ${personLabel(current)}` : ''}</span>
      <button onclick={() => step(1)} aria-label="Next face" title="Next (→)">→</button>
      <span class="sp"></span>
      <div class="lb-actions">{@render actions(viewed)}</div>
      <button onclick={close} aria-label="Close" title="Close (Esc)">✕</button>
    </div>
    <div class="lb-stage">
      <div class="lb-frame">
        {#key viewed.photo_id}
          <img alt="Where this face was found" src={thumbUrl('view', viewed.photo_id, viewed.mtime, viewed.size)} />
        {/key}
        <div class="lb-box" style={`left: ${viewed.x * 100}%; top: ${viewed.y * 100}%; width: ${viewed.w * 100}%; height: ${viewed.h * 100}%`}></div>
      </div>
    </div>
  </div>
{/if}

<style>
  .faces { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; font-size: 14px; }
  .top { flex: none; padding: 14px 18px 0; border-bottom: 1px solid #e3e6ee; }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 14px 18px 24px; }
  header { display: flex; align-items: center; gap: 16px; margin-bottom: 4px; }
  h2 { margin: 0; font-size: 18px; }
  .toggle { color: #555; font-size: 13px; }
  .hint { color: #777; margin: 6px 0 12px; }
  .err { color: #b3261e; }
  .status { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 6px 0 10px; padding: 6px 12px; border-radius: 8px; background: #eef2fb; color: #2a3f7a; font-size: 13px; }
  .status progress { width: 160px; }
  .notice { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 8px 0 12px; padding: 8px 12px; border-radius: 8px; background: #fff4d6; color: #5b4300; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(420px, 1fr)); gap: 14px; }
  .card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1px solid #d3d8e4; border-radius: 10px; background: #fafbfe; min-width: 0; }
  .card.dim { opacity: .6; }
  .samples { display: flex; gap: 6px; padding: 0; border: 0; background: none; cursor: pointer; }
  .samples img { width: 128px; height: 128px; object-fit: cover; border-radius: 8px; background: #dde2ee; }
  .samples.text { padding: 18px 10px; border-radius: 8px; background: #eef1f8; font: inherit; font-weight: 600; color: #333; }
  input, select { font: inherit; font-size: 13px; padding: 3px 6px; border: 1px solid #c3c8d4; border-radius: 6px; background: #fff; color: inherit; max-width: 100%; }
  form input { width: 100%; box-sizing: border-box; }
  .line { color: #666; font-size: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  button { font: inherit; font-size: 13px; }
  .detail-head { display: flex; align-items: center; gap: 12px; margin: 8px 0 14px; }
  .detail-head form { width: 260px; }
  .listbar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0 0 10px; font-size: 13px; color: #555; }
  .listbar .sp { flex: 1; }
  .listbar button.on { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  figure { position: relative; }
  figure.ticked img { outline: 3px solid #2f6fed; outline-offset: -3px; }
  .tick { position: absolute; top: 6px; left: 6px; z-index: 1; padding: 2px; border-radius: 4px; background: rgba(255, 255, 255, .85); line-height: 0; }
  .zoom { padding: 0; border: 0; background: none; cursor: zoom-in; display: block; }
  .lightbox { position: fixed; inset: 0; z-index: 50; display: flex; flex-direction: column; background: rgba(12, 14, 20, .94); color: #eee; }
  .lb-bar { display: flex; align-items: center; gap: 10px; padding: 8px 14px; background: rgba(0, 0, 0, .35); font-size: 13px; }
  .lb-bar .sp { flex: 1; }
  .lb-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .lb-actions input, .lb-actions select { background: #fff; }
  .lb-stage { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; padding: 12px; }
  /* The frame shrink-wraps the picture, so the box's percentages are percentages of the photo itself. */
  .lb-frame { position: relative; display: inline-block; line-height: 0; }
  .lb-frame img { max-width: calc(100vw - 24px); max-height: calc(100vh - 90px); background: #222; }
  .lb-box { position: absolute; box-sizing: border-box; border: 3px solid #ffd54a; border-radius: 4px; box-shadow: 0 0 0 1px rgba(0, 0, 0, .6), 0 0 0 9999px rgba(0, 0, 0, .25); pointer-events: none; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 14px; }
  figure { margin: 0; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  figure img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 8px; background: #dde2ee; }
  figcaption { display: flex; flex-wrap: wrap; gap: 6px; }
  figcaption form { width: 100%; }
</style>
