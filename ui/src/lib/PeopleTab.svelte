<script lang="ts">
  import { tick, untrack } from 'svelte';
  import {
    assignGroup, facesOfPerson, faceOverview, listFacePeople, moveFaces, newPersonFromFaces, rejectFaces, renamePerson,
    hidePerson, ignoredFaces, ignoreFaces, unignoreFaces, suggestForFaces, suggestForGroup, unhidePerson, unsortedFaces,
  } from './api';
  import { analysisFraction } from './analysis';
  import { createLatest } from './latest';
  import {
    clickSelection, sectionCount, toCandidates, faceCardLine, firstSection, lookingForPeople, peopleLine, personLabel, pickable, refreshOffered, sectionTitle,
    type FaceOrder, type FaceOverview, type FacePerson, type FaceTile, type PeopleStatus, type Section, type UnsortedFace,
  } from './people';
  import FaceGrid from './FaceGrid.svelte';
  import FaceLightbox from './FaceLightbox.svelte';
  import LooseCard from './LooseCard.svelte';
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
  /** The faces ignored one by one (the Ignored section also lists groups that were ignored whole). */
  let ignored = $state<FaceTile[]>([]);
  /** One person (or group) opened, with how their faces are shown: all of them in an order, or only those to check. */
  let selected = $state<number | null>(null);
  let order = $state<FaceOrder>('doubtful');
  /** How the Named screen is ordered. */
  let peopleSort = $state<'name' | 'photos'>('name');
  let tiles = $state<FaceTile[]>([]);
  let shown = $state(PAGE);
  let picked = $state<ReadonlySet<number>>(new Set());
  let anchor = $state<number | null>(null);
  let viewing = $state<number | null>(null);
  /** The faces being walked through in the whole-photo view: a copy taken when it was opened, so naming a face (which can
   *  empty the group it was in) does not change what is being walked through. */
  let walk = $state<FaceTile[]>([]);
  /** The opened card went away while the photo view was open (its faces all found a person): go back when that closes. */
  let pendingBack = false;
  let loaded = $state(false);
  let loadedRev = $state<number | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  const latest = createLatest();

  const current = $derived(selected === null ? null : (cards.find((c) => c.id === selected) ?? named.find((c) => c.id === selected) ?? null));
  const pickers = $derived(pickable(named, selected));
  const scanning = $derived(status !== null && (status.models !== 'ready' || status.done < status.total));
  const fraction = $derived(status ? analysisFraction(status) : null);
  const stale = $derived(refreshOffered(status?.rev, loadedRev));
  const total = $derived(section === 'groups' && selected === null ? (overview?.unsorted ?? 0) : (current?.faces ?? 0));
  const shownCards = $derived(
    section === 'people'
      ? [...cards].sort((a, b) => (peopleSort === 'photos' ? b.photos - a.photos : 0) || (a.name ?? '').localeCompare(b.name ?? '', undefined, { sensitivity: 'base' }) || a.id - b.id)
      : cards,
  );
  const faceList = $derived(selected === null && section === 'groups' ? unsorted.map((u) => u.tile) : selected === null && section === 'hidden' ? ignored : tiles);

  async function load() {
    const token = latest.next();
    const sel = selected;
    const ord = order;
    const count = shown;
    try {
      const ov = await faceOverview();
      const sec = section ?? firstSection(ov);
      const [list, people, loose, faces, gone1] = await Promise.all([
        listFacePeople(sec),
        sec === 'people' ? Promise.resolve<FacePerson[] | null>(null) : listFacePeople('people'),
        sec === 'groups' && sel === null ? unsortedFaces(0, count) : Promise.resolve<UnsortedFace[]>([]),
        sel === null ? Promise.resolve<FaceTile[]>([]) : facesOfPerson(sel, 0, count, ord),
        sec === 'hidden' && sel === null ? ignoredFaces(0, count) : Promise.resolve<FaceTile[]>([]),
      ]);
      if (!latest.isCurrent(token)) return;
      overview = ov;
      section = sec;
      cards = list;
      named = people ?? list;
      unsorted = loose;
      ignored = gone1;
      // the person was given away (their group joined someone, or every face was moved): back to the list, not an empty screen
      const gone = sel !== null && ![...list, ...named].some((c) => c.id === sel);
      tiles = gone ? [] : faces;
      loaded = true;
      loadedRev = ov.rev;
      // what was picked may have moved on
      const here = new Set(faces.map((t) => t.id).concat(loose.map((u) => u.tile.id)));
      picked = new Set([...picked].filter((id) => here.has(id)));
      if (gone) {
        // the person was given away: back to the list where it was, not an empty screen (once the photo view, if open, closes)
        if (viewing === null) back();
        else pendingBack = true;
      }
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

  // The scan finished while this tab was showing: show the final grouping (the page is only looked at while it works, not
  // changed, so nothing moves under the user; there is no button to ask for it).
  let wasScanning = false;
  $effect(() => {
    const now = scanning;
    const finished = wasScanning && !now;
    wasScanning = now;
    if (finished) untrack(() => { if (active && loaded && stale && selected === null) void load(); });
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
    load();
  }
  /** Where the list was (its section, scroll position and how much of it was showing) when a card was opened, so coming
   *  back puts the user in the same place. */
  let listPos: { section: Section; top: number; shown: number } | null = null;
  let scrollEl: HTMLElement | null = null;

  function open(id: number, mode: FaceOrder = 'doubtful') {
    if (selected === null && section) listPos = { section, top: scrollEl?.scrollTop ?? 0, shown };
    selected = id;
    order = mode;
    tiles = [];
    shown = PAGE;
    picked = new Set();
    anchor = null;
    load();
  }
  /** Back to the list from an opened card: the same section, with as much showing as before, scrolled to where it was. */
  async function back() {
    const pos = listPos?.section === section ? listPos : null;
    selected = null;
    tiles = [];
    shown = pos?.shown ?? PAGE;
    picked = new Set();
    await load();
    if (pos) {
      await tick();
      if (scrollEl) scrollEl.scrollTop = pos.top;
    }
  }
  /** Escape goes back from an opened card (not while the whole-photo view or a name box has it: they handle their own). */
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || e.defaultPrevented || !active || selected === null || viewing !== null) return;
    e.preventDefault();
    back();
  }
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

  const ignoreSelected = () => act(() => ignoreFaces(ids()));
  const unignoreSelected = () => act(() => unignoreFaces(ids()));
  /** Looking at the faces ignored one by one (not a person's faces, not Unsure). */
  const inIgnored = $derived(selected === null && section === 'hidden');
  /** Who the faces on screen look like (for the top of "This is…"): the selection, else the whole group. */
  const candidatesFor = () => (picked.size > 0 ? suggestForFaces(ids()).then(toCandidates) : current ? suggestForGroup(current.id).then(toCandidates) : Promise.resolve([]));
  const hide = (p: FacePerson) => act(() => hidePerson(p.id));
  const unhide = (p: FacePerson) => act(() => unhidePerson(p.id));

  // ---- faces
  const give = (faceIds: number[], person: number) => act(() => moveFaces(faceIds, person));
  const giveNew = (faceIds: number[], text: string) => act(() => newPersonFromFaces(faceIds, text));
  const notThem = (faceIds: number[]) => act(() => rejectFaces(faceIds));

  /** Open the whole-photo view on face `i` of the faces on screen. */
  function view(i: number) {
    walk = faceList.slice();
    viewing = i;
  }
  function closeView() {
    viewing = null;
    if (pendingBack) {
      pendingBack = false;
      back();
    }
  }
  function step(by: number) {
    if (viewing === null || walk.length === 0) return;
    const to = viewing + by;
    if (to < 0 || to >= walk.length) return; // like the photo viewer: no wrapping round
    (document.activeElement as HTMLElement | null)?.blur();
    viewing = to;
  }
  /** After something was done with the face on screen, stay in the walk-through: on to the next face. */
  const advance = () => { if (viewing !== null && viewing < walk.length - 1) viewing += 1; };

  /** Opened an unnamed group: with nothing selected, "This is…" names the whole group. */
  const wholeGroup = $derived(selected !== null && current !== null && current.name === null);
  const sections: Section[] = ['people', 'groups', 'hidden'];
</script>

<svelte:window {onkeydown} />

{#snippet lightActions(t: FaceTile)}
  <ThisIs people={pickers} small alignRight onpick={(p) => give([t.id], p).then(advance)} onnew={(n) => giveNew([t.id], n).then(advance)} onignore={inIgnored ? undefined : () => act(() => ignoreFaces([t.id])).then(advance)}
          candidates={() => suggestForFaces([t.id]).then(toCandidates)} />
{/snippet}

<div class="people" aria-busy={busy}>
  {#if busy}
    <div class="busy" role="status"><span class="spin" aria-hidden="true"></span>Updating face data…</div>
  {/if}
  <div class="top">
    <header>
      {#if selected !== null}
        <button onclick={back} title="Go back">← {section ? sectionTitle[section] : 'Back'}</button>
      {:else if loaded && overview}
        <nav class="sections" aria-label="People">
          {#each sections as s (s)}
            <button class:on={section === s} aria-pressed={section === s} onclick={() => goto(s)}>
              {sectionTitle[s]}<span class="n">{sectionCount(overview, s).toLocaleString()}</span>
            </button>
          {/each}
        </nav>
      {/if}
      {#if error}<span class="err">{error}</span>{/if}
    </header>

    {#if scanning}
      <div class="status" role="status">
        <span>{status ? peopleLine(status) : ''}</span>
        {#if fraction !== null && scanning}<progress value={fraction} max="1"></progress>{/if}
        {#if status && lookingForPeople(status)}<span class="advice">It's best to let Pixie finish before naming anyone.</span>{/if}
      </div>
    {/if}
    {#if notice}<div class="notice" role="status">{notice} <button onclick={() => (notice = null)} title="Dismiss this message">OK</button></div>{/if}

    {#if loaded && selected === null && section === 'groups' && (overview?.groups ?? 0) + (overview?.unsorted ?? 0) > 0}
      <p class="hint">Each card is one person as far as Pixie can tell, or a single face it could not place. Click <b>This is…</b> and type a name, or pick someone you have already named. Click a picture to see the faces behind it.</p>
    {/if}

    {#if loaded && selected !== null && current}
      <div class="detail-head">
        {#if current.name !== null}<strong class="title">{current.name}</strong>{/if}
        <span class="line">{faceCardLine(current)}</span>
        {#if !current.hidden && current.name !== null}<button onclick={() => hide(current)} title="Leave this out of the lists">Ignore</button>{/if}
      </div>
      <div class="listbar">
        <span>Sort:</span>
        <button class:on={order === 'doubtful'} onclick={() => setOrder('doubtful')} title="The faces least like the rest first: look here for the wrong ones">Least confident</button>
        <button class:on={order === 'best'} onclick={() => setOrder('best')} title="The clearest, largest faces first">Most confident</button>
        <span class="sp"></span>
      </div>
    {/if}

    {#if loaded && (selected !== null || (inIgnored && ignored.length > 0))}
      <div class="bar" class:live={picked.size > 0}>
        <!-- One "This is…", always in the same place: with faces selected it names those faces; with none, the whole group
             (in a named person or among the loose faces there is nothing to name until something is selected). -->
        <ThisIs
          people={pickers}
          disabled={picked.size === 0 && !wholeGroup}
          candidates={candidatesFor}
          onpick={(p) => (picked.size > 0 ? give(ids(), p) : current && assign(current, p))}
          onnew={(n) => (picked.size > 0 ? giveNew(ids(), n) : current && name(current, n))}
          onignore={inIgnored || (picked.size === 0 && !wholeGroup) ? undefined : () => (picked.size > 0 ? ignoreSelected() : current && hide(current))}
        />
        {#if picked.size > 0}
          <b>{picked.size.toLocaleString()} selected</b>
        {:else if wholeGroup}
          <span class="muted">names this whole group, or select faces with the circles (shift-click selects a run)</span>
        {:else}
          <span class="muted">Click a circle to select a face (shift-click selects a run), then say who they are.</span>
        {/if}
        <span class="sp"></span>
        {#if picked.size > 0}
          {#if inIgnored}
            <button onclick={unignoreSelected} title="Stop ignoring the selected faces, so they are offered for naming again">Unignore</button>
          {:else}
            {#if selected !== null}<button onclick={() => notThem(ids())} title="Say the selected faces are not this person, and take them out of the group">Move out of group</button>{/if}
          {/if}
          <button onclick={() => (picked = new Set())} title="Clear the selection">Clear</button>
        {:else}
          {#if current?.hidden}<button onclick={() => unhide(current)} title="Bring this group back">Unignore this group</button>{/if}
          {#if faceList.length > 0}<button onclick={pickAll} title="Select all the faces shown">Select all shown</button>{/if}
        {/if}
      </div>
    {/if}
  </div>

  <div class="scroll" bind:this={scrollEl}>
    {#if !loaded}
      <p class="hint">Loading…</p>
    {:else if selected === null}
      {#if cards.length === 0 && !(inIgnored && ignored.length > 0) && !(section === 'groups' && unsorted.length > 0)}
        <p class="hint">
          {#if section === 'groups'}{(overview?.people ?? 0) > 0 ? 'Nothing left to name.' : 'No groups yet. Pixie looks for faces in the background once the face models are downloaded: see Settings, Picture analysis.'}
          {:else if section === 'people'}Nobody is named yet. Name a group in “To name”.
          {:else}Nothing is ignored.{/if}
        </p>
      {:else}
        {#if section === 'people'}
          <div class="listbar">
            <span>Sort:</span>
            <button class:on={peopleSort === 'name'} title="List people by name" onclick={() => (peopleSort = 'name')}>Name</button>
            <button class:on={peopleSort === 'photos'} title="List people by how many photos they are in" onclick={() => (peopleSort = 'photos')}>Photos</button>
          </div>
        {/if}
        {#if cards.length > 0 || (section === 'groups' && unsorted.length > 0)}
          <!-- On To name every card works the same, whether it holds many faces or one: the difference is only what opening it
               shows (a group's faces, or the one photo). -->
          <div class="cards">
            {#each shownCards as p (p.id)}
              <PersonCard person={p} people={pickable(named, p.id)} onopen={() => open(p.id)}
                          onassign={(into) => assign(p, into)} onname={(text) => name(p, text)} onhide={() => hide(p)} />
            {/each}
            {#if section === 'groups'}
              {#each unsorted as u, i (u.tile.id)}
                <LooseCard tile={u.tile} people={pickable(named, null)} onopen={() => view(i)} onassign={(pid) => give([u.tile.id], pid)}
                           onname={(text) => giveNew([u.tile.id], text)} onignore={() => act(() => ignoreFaces([u.tile.id]))} />
              {/each}
            {/if}
          </div>
          {#if section === 'groups' && unsorted.length < total}<p><button onclick={showMore} title="Show more groups of faces">Show more</button></p>{/if}
        {/if}
        {#if inIgnored && ignored.length > 0}
          <FaceGrid tiles={ignored} {picked} onclick={click} onzoom={view} />
          {#if ignored.length >= shown}<p><button onclick={showMore} title="Show more faces">Show more faces</button></p>{/if}
        {/if}
      {/if}
    {:else}
      {#if tiles.length === 0}
        <p class="hint">No faces here.</p>
      {:else}
        <FaceGrid tiles={faceList} {picked} onclick={click} onzoom={view} />
        {#if tiles.length < total}<p><button onclick={showMore} title="Show more faces">Show more faces</button></p>{/if}
      {/if}
    {/if}
  </div>
</div>

{#if viewing !== null}
  <FaceLightbox tiles={walk} index={viewing} title={current ? personLabel(current) : undefined} onclose={closeView} onstep={step} actions={lightActions} />
{/if}

<style>
  .people { position: absolute; inset: 0; display: flex; flex-direction: column; background: #fff; font-size: 14px; }
  /* Shown only if the work takes a moment, so quick changes do not flash it. */
  .busy { position: absolute; inset: 0; z-index: 30; display: flex; align-items: center; justify-content: center; gap: 12px; background: rgba(255, 255, 255, .8); font-size: 16px; color: #2a3f7a; opacity: 0; animation: busyin .2s .25s forwards; }
  .spin { width: 22px; height: 22px; border: 3px solid #c3d1f5; border-top-color: #2f6fed; border-radius: 50%; animation: spin .8s linear infinite; }
  @keyframes busyin { to { opacity: 1; } }
  @keyframes spin { to { transform: rotate(360deg); } }
  .top { flex: none; padding: var(--page-pad-y) var(--page-pad-x) 8px; border-bottom: 1px solid #e3e6ee; }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: var(--page-pad-y) var(--page-pad-x) 24px; }
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
  .status .advice { color: #555; }
  .notice { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 6px 0 8px; padding: 6px 12px; border-radius: 8px; background: #fff4d6; color: #5b4300; font-size: 13px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 14px; }
  button { font: inherit; font-size: 13px; }
  .detail-head { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 8px 0 8px; }
  .title { font-size: 16px; }
  .line { color: #666; font-size: 12px; }
  .listbar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0 0 8px; font-size: 13px; color: #555; }
  .listbar button.on { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 30px; padding: 4px 8px; border-radius: 8px; background: #f4f6fb; }
  .bar.live { background: #e3ecff; }
</style>
