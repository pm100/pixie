<script lang="ts">
  import { untrack } from 'svelte';
  import {
    assignGroup, facesOfPerson, faceOverview, hidePerson, listFacePeople, moveFaces, newPersonFromFaces, rejectFaces, renamePerson,
    tidyFaces, unhidePerson, unsortedFaces,
  } from './api';
  import { analysisFraction } from './analysis';
  import { createLatest } from './latest';
  import {
    clickSelection, faceCardLine, firstSection, peopleLine, personLabel, pickable, refreshOffered, sectionTitle,
    type FaceOrder, type FaceOverview, type FacePerson, type FaceTile, type PeopleStatus, type Section, type UnsortedFace,
  } from './people';
  import FaceGrid from './FaceGrid.svelte';
  import FaceLightbox from './FaceLightbox.svelte';
  import PersonCard from './PersonCard.svelte';
  import ThisIs from './ThisIs.svelte';

  let { active, status, onchange }: {
    /** This tab is the one on screen (it stays built while another tab is shown, so coming back is instant). */
    active: boolean;
    /** How far the face scan has got, and `rev`, which changes when faces are regrouped in the background. */
    status: PeopleStatus | null;
    /** Somebody was named, merged, hidden or moved here: the rest of the app should refresh its people. */
    onchange: () => void;
  } = $props();

  const PAGE = 60;

  let overview = $state<FaceOverview | null>(null);
  let section = $state<Section | null>(null);
  /** The cards of the section (groups, people or hidden), and the named people a face can be given to. */
  let cards = $state<FacePerson[]>([]);
  let named = $state<FacePerson[]>([]);
  let unsorted = $state<UnsortedFace[]>([]);
  /** One person (or group) opened, with how their faces are shown: all of them in an order, or only those to check. */
  let selected = $state<number | null>(null);
  let order = $state<FaceOrder>('best');
  let tiles = $state<FaceTile[]>([]);
  let shown = $state(PAGE);
  let picked = $state<ReadonlySet<number>>(new Set());
  let anchor = $state<number | null>(null);
  let viewing = $state<number | null>(null);
  let loaded = $state(false);
  let loadedRev = $state<number | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let nameDraft = $state<string | null>(null);
  const latest = createLatest();

  const current = $derived(selected === null ? null : (cards.find((c) => c.id === selected) ?? named.find((c) => c.id === selected) ?? null));
  const pickers = $derived(pickable(named, selected));
  const scanning = $derived(status !== null && (status.models !== 'ready' || status.done < status.total));
  const fraction = $derived(status ? analysisFraction(status) : null);
  const stale = $derived(refreshOffered(status?.rev, loadedRev));
  const total = $derived(section === 'unsorted' && selected === null ? (overview?.unsorted ?? 0) : (current?.faces ?? 0));
  const faceList = $derived(selected === null && section === 'unsorted' ? unsorted.map((u) => u.tile) : tiles);

  async function load() {
    const token = latest.next();
    const sel = selected;
    const ord = order;
    const count = shown;
    try {
      const ov = await faceOverview();
      const sec = section ?? firstSection(ov);
      const [list, people, loose, faces] = await Promise.all([
        sec === 'unsorted' ? Promise.resolve<FacePerson[]>([]) : listFacePeople(sec),
        sec === 'people' ? Promise.resolve<FacePerson[] | null>(null) : listFacePeople('people'),
        sec === 'unsorted' && sel === null ? unsortedFaces(0, count) : Promise.resolve<UnsortedFace[]>([]),
        sel === null ? Promise.resolve<FaceTile[]>([]) : facesOfPerson(sel, 0, count, ord),
      ]);
      if (!latest.isCurrent(token)) return;
      overview = ov;
      section = sec;
      cards = list;
      named = people ?? list;
      unsorted = loose;
      tiles = faces;
      loaded = true;
      loadedRev = ov.rev;
      // what was picked may have moved on
      const here = new Set(faces.map((t) => t.id).concat(loose.map((u) => u.tile.id)));
      picked = new Set([...picked].filter((id) => here.has(id)));
    } catch (e) {
      if (latest.isCurrent(token)) error = String(e);
    }
  }

  // First shown: load. (`load` reads other state; only becoming active may trigger it.)
  let wasActive = false;
  $effect(() => {
    const now = active;
    const first = now && !wasActive && !loaded;
    // coming back to the tab: show what was regrouped meanwhile, unless the user is partway through a person's faces
    const back = now && !wasActive && loaded && stale && selected === null;
    wasActive = now;
    if (first || back) untrack(load);
  });

  /** Run a change, then reload this tab and tell the app. */
  async function act(change: () => Promise<unknown>) {
    error = null;
    notice = null;
    busy = true;
    try { await change(); } catch (e) { error = String(e); busy = false; return; }
    picked = new Set();
    await load();
    busy = false;
    onchange();
  }

  function goto(next: Section) {
    section = next;
    selected = null;
    tiles = [];
    shown = PAGE;
    picked = new Set();
    nameDraft = null;
    load();
  }
  function open(id: number, mode: FaceOrder = 'best') {
    selected = id;
    order = mode;
    tiles = [];
    shown = PAGE;
    picked = new Set();
    anchor = null;
    nameDraft = null;
    load();
  }
  const back = () => {
    selected = null;
    tiles = [];
    shown = PAGE;
    picked = new Set();
    nameDraft = null;
    load();
  };
  const setOrder = (o: FaceOrder) => { order = o; shown = PAGE; picked = new Set(); load(); };
  const showMore = () => { shown += 120; load(); };

  function click(id: number, shift: boolean) {
    const r = clickSelection(faceList.map((t) => t.id), picked, anchor, id, shift);
    picked = r.picked;
    anchor = r.anchor;
  }
  const pickAll = () => (picked = new Set(faceList.map((t) => t.id)));
  const ids = () => [...picked];

  // ---- groups and people
  const assign = (p: FacePerson, into: number) => act(async () => { await assignGroup(p.id, into); });
  const name = (p: FacePerson, text: string) => act(async () => { await renamePerson(p.id, text); });
  const hide = (p: FacePerson) => act(async () => { await hidePerson(p.id); if (selected === p.id) selected = null; });
  const unhide = (p: FacePerson) => act(() => unhidePerson(p.id));
  async function saveName() {
    const cur = current;
    const text = (nameDraft ?? '').trim();
    nameDraft = null;
    if (!cur || text === (cur.name ?? '')) return;
    await act(async () => {
      const kept = await renamePerson(cur.id, text === '' ? null : text);
      if (selected === cur.id) selected = kept; // a name somebody has: the two were joined
    });
  }

  // ---- faces
  const give = (faceIds: number[], person: number) => act(() => moveFaces(faceIds, person));
  const giveNew = (faceIds: number[], text: string) => act(() => newPersonFromFaces(faceIds, text));
  const notThem = (faceIds: number[]) => act(() => rejectFaces(faceIds));
  const splitOff = (faceIds: number[]) => act(() => newPersonFromFaces(faceIds, null));
  async function tidy() {
    await act(async () => {
      const done = await tidyFaces();
      if (!done) notice = 'The faces were changing while this ran. Try again in a moment.';
    });
  }

  function step(by: number) {
    if (viewing === null || faceList.length === 0) return;
    const to = viewing + by;
    if (to < 0 || to >= faceList.length) return; // like the photo viewer: no wrapping round
    (document.activeElement as HTMLElement | null)?.blur();
    viewing = to;
  }
  // After a face leaves the list the one that took its place is shown; past the end, the last; none left, close.
  $effect(() => {
    if (viewing !== null && viewing >= faceList.length) viewing = faceList.length > 0 ? faceList.length - 1 : null;
  });

  const sections: Section[] = ['groups', 'people', 'unsorted', 'hidden'];
</script>

{#snippet lightActions(t: FaceTile)}
  <ThisIs people={pickers} small onpick={(p) => give([t.id], p)} onnew={(n) => giveNew([t.id], n)} />
  {#if selected !== null && current?.confirmed}<button onclick={() => notThem([t.id])}>Not this person</button>{/if}
{/snippet}

<div class="people">
  <div class="top">
    <header>
      {#if selected !== null}
        <button onclick={back}>← {section ? sectionTitle[section] : 'Back'}</button>
      {:else if loaded && overview}
        <nav class="sections" aria-label="People">
          {#each sections as s (s)}
            <button class:on={section === s} aria-pressed={section === s} onclick={() => goto(s)}>
              {sectionTitle[s]}<span class="n">{overview[s].toLocaleString()}</span>
            </button>
          {/each}
        </nav>
        <span class="sp"></span>
        <button onclick={tidy} disabled={busy} title="Regroup the faces you have not settled yet. This also happens by itself as photos are scanned.">Tidy up</button>
      {/if}
      {#if error}<span class="err">{error}</span>{/if}
    </header>

    {#if scanning || stale}
      <div class="status" role="status">
        <span>{status ? peopleLine(status) : ''}</span>
        {#if fraction !== null && scanning}<progress value={fraction} max="1"></progress>{/if}
        {#if stale}<button onclick={load}>Show the new grouping</button>{/if}
      </div>
    {/if}
    {#if notice}<div class="notice" role="status">{notice} <button onclick={() => (notice = null)}>OK</button></div>{/if}

    {#if loaded && selected === null && section === 'groups' && (overview?.groups ?? 0) > 0}
      <p class="hint">Pixie put faces that look alike together. Say who each group is: <b>This is…</b> picks someone you have already named, <b>Somebody else…</b> names a new person.</p>
    {/if}

    {#if loaded && selected !== null && current}
      <div class="detail-head">
        <form onsubmit={(e) => { e.preventDefault(); saveName(); }}>
          <input value={nameDraft ?? current.name ?? ''} oninput={(e) => (nameDraft = e.currentTarget.value)} onblur={saveName} placeholder="Name this person" aria-label="Name" />
        </form>
        <span class="line">{faceCardLine(current)}</span>
        {#if current.name === null && !current.hidden}
          <ThisIs people={pickers} label="This group is…" onpick={(p) => assign(current, p)} onnew={(n) => name(current, n)} />
        {/if}
        {#if !current.hidden}<button onclick={() => hide(current)}>Hide</button>{:else}<button onclick={() => unhide(current)}>Unhide</button>{/if}
      </div>
      <div class="listbar">
        <span>Show:</span>
        <button class:on={order === 'best'} onclick={() => setOrder('best')} title="The clearest, largest faces first">Best faces first</button>
        <button class:on={order === 'doubtful'} onclick={() => setOrder('doubtful')} title="The faces least like the rest first: look here for the wrong ones">Least typical first</button>
        <span class="sp"></span>
      </div>
    {/if}

    {#if loaded && (selected !== null || section === 'unsorted')}
      <div class="bar" class:live={picked.size > 0}>
        {#if picked.size > 0}
          <b>{picked.size.toLocaleString()} picked</b>
          <ThisIs people={pickers} onpick={(p) => give(ids(), p)} onnew={(n) => giveNew(ids(), n)} />
          {#if selected !== null && current?.confirmed}<button onclick={() => notThem(ids())}>Not this person</button>{/if}
          {#if selected !== null}<button onclick={() => splitOff(ids())} title="Make these faces a person of their own">Split off as a new person</button>{/if}
          <button onclick={() => (picked = new Set())}>Clear</button>
        {:else}
          <span class="muted">Click faces to pick them (shift-click picks a run), then say who they are.</span>
          <span class="sp"></span>
          {#if faceList.length > 0}<button onclick={pickAll}>Pick all shown</button>{/if}
        {/if}
      </div>
    {/if}
  </div>

  <div class="scroll">
    {#if !loaded}
      <p class="hint">Loading…</p>
    {:else if selected === null && section === 'unsorted'}
      {#if unsorted.length === 0}
        <p class="hint">No loose faces. Everything is in a group.</p>
      {:else}
        <FaceGrid tiles={faceList} {picked} onclick={click} onzoom={(i) => (viewing = i)} />
        {#if unsorted.length < total}<p><button onclick={showMore}>Show more faces</button></p>{/if}
      {/if}
    {:else if selected === null}
      {#if cards.length === 0}
        <p class="hint">
          {#if section === 'groups'}{(overview?.people ?? 0) > 0 ? 'Nothing left to name.' : 'No groups yet. Pixie looks for faces in the background once the face models are downloaded: see Settings, Picture analysis.'}
          {:else if section === 'people'}Nobody is named yet. Name a group in “Grouped”.
          {:else}Nobody is hidden.{/if}
        </p>
      {:else}
        <div class="cards">
          {#each cards as p (p.id)}
            <PersonCard person={p} people={pickable(named, p.id)} onopen={() => open(p.id)}
                        onassign={(into) => assign(p, into)} onname={(text) => name(p, text)} onhide={() => hide(p)} onunhide={() => unhide(p)} />
          {/each}
        </div>
      {/if}
    {:else}
      {#if tiles.length === 0}
        <p class="hint">No faces here.</p>
      {:else}
        <FaceGrid tiles={faceList} {picked} onclick={click} onzoom={(i) => (viewing = i)} />
        {#if tiles.length < total}<p><button onclick={showMore}>Show more faces</button></p>{/if}
      {/if}
    {/if}
  </div>
</div>

{#if viewing !== null}
  <FaceLightbox tiles={faceList} index={viewing} title={current ? personLabel(current) : undefined} onclose={() => (viewing = null)} onstep={step} actions={lightActions} />
{/if}

<style>
  .people { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; font-size: 14px; }
  .top { flex: none; padding: 14px 18px 8px; border-bottom: 1px solid #e3e6ee; }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 14px 18px 24px; }
  header { display: flex; align-items: center; gap: 12px; margin-bottom: 4px; }
  .sp { flex: 1; }
  .sections { display: flex; gap: 6px; }
  .sections button { font: inherit; font-size: 14px; border: 1px solid #c3c8d4; background: #f4f6fb; border-radius: 16px; padding: 3px 12px; cursor: pointer; color: #333; }
  .sections button.on { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .sections .n { margin-left: 6px; opacity: .75; font-size: 12px; }
  .hint { color: #666; margin: 6px 0 8px; max-width: 80ch; }
  .muted { color: #777; font-size: 13px; }
  .err { color: #b3261e; }
  .status { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 6px 0 8px; padding: 6px 12px; border-radius: 8px; background: #eef2fb; color: #2a3f7a; font-size: 13px; }
  .status progress { width: 160px; }
  .notice { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 6px 0 8px; padding: 6px 12px; border-radius: 8px; background: #fff4d6; color: #5b4300; font-size: 13px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 14px; }
  button { font: inherit; font-size: 13px; }
  input { font: inherit; font-size: 14px; padding: 3px 6px; border: 1px solid #c3c8d4; border-radius: 6px; background: #fff; color: inherit; }
  .detail-head { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 8px 0 8px; }
  .detail-head form { width: 240px; }
  .detail-head input { width: 100%; box-sizing: border-box; }
  .line { color: #666; font-size: 12px; }
  .listbar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0 0 8px; font-size: 13px; color: #555; }
  .listbar button.on { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 30px; padding: 4px 8px; border-radius: 8px; background: #f4f6fb; }
  .bar.live { background: #e3ecff; }
</style>
