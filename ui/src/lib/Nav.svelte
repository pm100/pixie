<script lang="ts">
  import type { DayGroup, FolderInfo, MediaKind, Trip } from './api';
  import { untrack } from 'svelte';
  import Check from './Check.svelte';
  import { plural } from './format';
  import { clipTitle } from './clipTitle';
  import { folderNote, type FolderStates } from './folderState';
  import Chevron from './Chevron.svelte';
  import WhenNav from './WhenNav.svelte';
  import { folderName } from './format';
  import { periodLabel } from './periods';
  import { categoryOf, tagLabel, tagTree, type TagCount } from './tagNav';
  import { personLabel, personRows, type PersonInfo } from './people';
  import type { SectionId } from './sections';
  import { nameFromEdit, tripEditText, tripLines, tripSummary, tripTitle, tripYear, tripYears } from './trips';
  import { faceUrl } from './url';
  import { countOf, tripsHeld, type Facets } from './facets';

  let { facets = null, groups, kind, recent, period, recentText, tags, tag, people, facesFound, person, cameras = [], camera = null, oncamera = () => {}, analysisOn = { tags: true, faces: true }, onopenanalysis = () => {}, folders, folderStates = {}, folderId, trips, tripsReady, trip, destinations, onvisibletrips, collapsed, onperiod, onkind, onrecent, ontag, onperson, onfolder, ontrip, onrenametrip, onsettrips, onnamepeople, ontoggle }: {
    /** The counts under the filters in force (each section counted without its own filter); null until known, when the
     *  counts the entries came with are shown. An entry that comes to 0 is dimmed, not hidden, so the list does not move. */
    facets?: Facets | null;
    /** The days to list under When: the library under the other filters, without the date filter itself. */
    groups: DayGroup[]; kind: MediaKind | null; recent: boolean;
    /** The chosen date (a filter like the others): `y:2025`, `m:2025-06`, `d:2025-06-14` or `undated`. */
    period: string | null;
    /** The text of the recent row, e.g. "Last 30 days". */
    recentText: string;
    /** What the photos show, with counts, and the one chosen as a filter (if any). */
    tags: TagCount[]; tag: string | null; ontag: (t: string | null) => void;
    /** The people found in the photos, and the one chosen as a filter (if any). */
    people: PersonInfo[]; person: number | null; onperson: (id: number | null) => void;
    /** Some photos have been checked for faces: with nobody named yet, the section says where to name people. */
    facesFound: boolean;
    /** The photo folders, with the item count of each, and the one chosen (if any). */
    /** Which analyses are on (People, Auto tags); one that is off says so here, and how to turn it on. */
    analysisOn?: { tags: boolean; faces: boolean }; onopenanalysis?: () => void;
    /** The cameras (tidied names) with their counts under the filters in force, and the one chosen. */
    cameras?: [string, number][]; camera?: string | null; oncamera?: (name: string | null) => void;
    folders: FolderInfo[]; folderId: number | null; onfolder: (id: number | null) => void;
    /** Which folders cannot be reached right now (their drive is unplugged, say). */
    folderStates?: FolderStates;
    /** Trips away from home. `tripsReady` is false until the user has confirmed where home is. `trip` is the chosen one's `since`. */
    trips: Trip[]; tripsReady: boolean; trip: number | null; ontrip: (t: Trip | null) => void;
    /** Where each trip went (by its `since`), once known. `onvisibletrips` says which trips are showing, so those are named first. */
    destinations: Record<number, string>; onvisibletrips: (t: Trip[]) => void;
    onrenametrip: (t: Trip, name: string) => void; onsettrips: () => void;
    /** Go to the People tab, where people are named (the link under "Nobody is named yet"). */
    onnamepeople: () => void;
    /** Sections the user has folded away, and the click on a heading. */
    collapsed: SectionId[]; ontoggle: (id: SectionId) => void;
    onperiod: (key: string) => void;
    onkind: (k: MediaKind | null) => void; onrecent: (on: boolean) => void;
  } = $props();

  let allPeople = $state(false);
  /** Auto tags categories the user has opened; the one holding the chosen tag is open as well. */
  let openCategories = $state<Set<string>>(new Set());
  const shownPeople = $derived(personRows(people, person, allPeople, 8));
  // Trips are listed under the year they started in. The newest year starts open, and so does the year of a chosen trip.
  const years = $derived(tripYears(trips));
  let openYears = $state<Set<number>>(new Set());
  let yearsSeeded = false;
  const toggleYear = (y: number) => {
    const next = new Set(openYears);
    if (next.has(y)) next.delete(y); else next.add(y);
    openYears = next;
  };
  $effect(() => {
    const first = years[0]?.year;
    if (first !== undefined && !yearsSeeded) { yearsSeeded = true; untrack(() => (openYears = new Set([first]))); }
  });
  $effect(() => {
    const t = trips.find((x) => x.since === trip);
    if (t) untrack(() => { const y = tripYear(t); if (!openYears.has(y)) openYears = new Set([...openYears, y]); });
  });
  // Tell the app which trips are on show (to look up where they went, those first).
  $effect(() => {
    const showing = folded('trips') ? [] : years.filter((g) => openYears.has(g.year)).flatMap((g) => g.trips);
    untrack(() => onvisibletrips(showing));
  });
  const tree = $derived(tagTree(tags));
  const chosenCategory = $derived(categoryOf(tags, tag));
  const isOpen = (category: string) => openCategories.has(category);
  const toggleCategory = (category: string) => {
    const next = new Set(openCategories);
    if (isOpen(category)) next.delete(category); else next.add(category);
    openCategories = next;
  };
  // Choosing a tag (here or elsewhere) opens its category once; the user can close it again afterwards.
  $effect(() => {
    const c = chosenCategory;
    if (c !== null) untrack(() => { if (!openCategories.has(c)) openCategories = new Set([...openCategories, c]); });
  });

  const folded = (id: SectionId) => collapsed.includes(id);
  /** What is chosen inside a section, shown on its heading while it is folded so a filter is never hidden. */
  const chosen = $derived<Record<SectionId, string>>({
    library: kind === 'photo' ? 'Photos only' : kind === 'video' ? 'Videos only' : '',
    folders: folders.find((f) => f.id === folderId) ? folderName(folders.find((f) => f.id === folderId)!.path) : '',
    people: people.find((p) => p.id === person) ? personLabel(people.find((p) => p.id === person)!) : '',
    trips: trips.find((t) => t.since === trip) ? tripTitle(trips.find((t) => t.since === trip)!) : '',
    tags: tag !== null ? tagLabel(tag) : '',
    cameras: camera ?? '',
    when: period !== null ? periodLabel(period) : recent ? recentText : '',
  });

  let renaming = $state<number | null>(null), renameText = $state('');
  // The box starts with exactly what the node says, on one line with ": " between its two lines; what is typed
  // replaces all of it. Leaving it as it was keeps the node automatic.
  function startRename(t: Trip) { renaming = t.since; renameText = tripEditText(t, destinations[t.since]); }
  function endRename(commit: boolean, t: Trip) {
    if (renaming === null) return;
    renaming = null;
    if (!commit) return;
    const name = nameFromEdit(renameText, t, destinations[t.since]);
    if (name !== (t.name ?? '')) onrenametrip(t, name);
  }
  const focusOnMount = (el: HTMLInputElement) => { el.focus(); el.select(); };
</script>

{#snippet head(id: SectionId, title: string)}
  <button class="head" aria-expanded={!folded(id)} onclick={() => ontoggle(id)}>
    <Chevron open={!folded(id)} />{title}
    {#if folded(id) && chosen[id]}<span class="chosen" use:clipTitle>· {chosen[id]}</span>{/if}
  </button>
{/snippet}

<nav>
  {@render head('when', 'When')}
  {#if !folded('when')}
    <button class="item" class:active={recent} class:zero={facets && facets.recent === 0 && !recent} aria-pressed={recent} onclick={() => onrecent(!recent)}><span class="mark">{#if recent}<Check />{/if}</span><span class="grow" use:clipTitle>{recentText}</span>{#if facets}<span class="n">{facets.recent.toLocaleString()}</span>{/if}</button>
    <WhenNav {groups} {period} {onperiod} />
  {/if}

  {#if !analysisOn.faces}
    {@render head('people', 'People')}
    {#if !folded('people')}
      <p class="hint">People is turned off, so no faces are being looked for. Turn it on in Settings → Picture analysis.</p>
      <button class="item more" onclick={onopenanalysis}><span class="mark"></span>Open Settings…</button>
    {/if}
  {:else if people.length === 0 && facesFound}
    {@render head('people', 'People')}
    {#if !folded('people')}
      <p class="hint">Nobody is named yet. Name people in the People tab and they will appear here.</p>
      <button class="item more" onclick={onnamepeople}><span class="mark"></span>Name people…</button>
    {/if}
  {:else if people.length > 0}
    {@render head('people', 'People')}
    {#if !folded('people')}
      {#each shownPeople.rows as p (p.id)}
        {@const n = countOf(facets?.people, p.id, p.count)}
        <button class="item" class:active={person === p.id} class:zero={facets && n === 0 && person !== p.id} aria-pressed={person === p.id} onclick={() => onperson(person === p.id ? null : p.id)}>
          <span class="mark">{#if person === p.id}<Check />{/if}</span>
          {#if p.face_id !== null}<img class="avatar" alt="" src={faceUrl(p.face_id)} />{:else}<span class="avatar"></span>{/if}
          <span class="grow" use:clipTitle>{personLabel(p)}</span><span class="n">{n.toLocaleString()}</span>
        </button>
      {/each}
      {#if shownPeople.hidden > 0 || allPeople}
        <button class="item more" onclick={() => (allPeople = !allPeople)}><span class="mark"></span>{allPeople ? 'Show fewer' : `Show all ${plural(people.length, 'person', 'people')}`}</button>
      {/if}
    {/if}
  {/if}

  {#if !analysisOn.tags}
    {@render head('tags', 'Auto tags')}
    {#if !folded('tags')}
      <p class="hint">Auto tags are turned off, so photos are not being labelled. Turn them on in Settings → Picture analysis.</p>
      <button class="item more" onclick={onopenanalysis}><span class="mark"></span>Open Settings…</button>
    {/if}
  {:else if tags.length > 0}
    {@render head('tags', 'Auto tags')}
    {#if !folded('tags')}
      {#each tree as g (g.category)}
        {@const held = countOf(facets?.categories, g.category, g.photos)}
        <button class="item cat" class:zero={facets && held === 0} aria-expanded={isOpen(g.category)} onclick={() => toggleCategory(g.category)}>
          <Chevron open={isOpen(g.category)} /><span class="grow" use:clipTitle>{g.category}</span><span class="n">{held.toLocaleString()}</span>
        </button>
        {#if isOpen(g.category)}
          {#each g.tags as t (t.tag)}
            {@const n = countOf(facets?.tags, t.tag, t.count)}
            <button class="item sub" class:active={tag === t.tag} class:zero={facets && n === 0 && tag !== t.tag} aria-pressed={tag === t.tag} onclick={() => ontag(tag === t.tag ? null : t.tag)}>
              <span class="mark">{#if tag === t.tag}<Check />{/if}</span><span class="grow" use:clipTitle>{tagLabel(t.tag)}</span><span class="n">{n.toLocaleString()}</span>
            </button>
          {/each}
        {/if}
      {/each}
    {/if}
  {/if}

  {@render head('trips', 'Trips')}
  {#if !folded('trips')}
    {#if !tripsReady}
      <p class="hint">Trips are runs of days spent away from home. Pixie needs to know where home is first.</p>
      <button class="item more" onclick={onsettrips}><span class="mark"></span>Set up trips…</button>
    {:else if trips.length === 0}
      <p class="hint">No trips found. A smaller home radius or a shorter minimum length in Settings may find some.</p>
    {:else}
      {#each years as g (g.year)}
        {@const held = tripsHeld(g.trips, facets ?? null)}
        <button class="item cat" class:zero={facets && held === 0} aria-expanded={openYears.has(g.year)} onclick={() => toggleYear(g.year)}>
          <Chevron open={openYears.has(g.year)} /><span class="grow" use:clipTitle>{g.year}</span>
        </button>
        {#if openYears.has(g.year)}
          {#each g.trips as t (t.since)}
            {@const lines = tripLines(t, destinations[t.since])}
            {@const n = countOf(facets?.trips, t.since, t.photos + t.videos)}
            {#if renaming === t.since}
              <div class="item sub rename"><span class="mark"></span>
                <input use:focusOnMount bind:value={renameText} aria-label="Trip name" placeholder={tripEditText({ ...t, name: null }, destinations[t.since])} maxlength="120"
                       onkeydown={(e) => { if (e.key === 'Enter') endRename(true, t); else if (e.key === 'Escape') endRename(false, t); }} onblur={() => endRename(true, t)} />
              </div>
            {:else}
              <div class="row">
                <button class="item sub trip" class:active={trip === t.since} class:zero={facets && n === 0 && trip !== t.since} aria-pressed={trip === t.since} title={tripSummary(t)} onclick={() => ontrip(trip === t.since ? null : t)}>
                  <span class="mark">{#if trip === t.since}<Check />{/if}</span>
                  <span class="grow"><span class="when" use:clipTitle>{lines[0]}</span>{#if lines[1]}<span class="dest" use:clipTitle>{lines[1]}</span>{/if}</span>
                  <span class="n">{n.toLocaleString()}</span>
                </button>
                <button class="edit" aria-label="Rename this trip" title="Rename this trip" onclick={() => startRename(t)}>✎</button>
              </div>
            {/if}
          {/each}
        {/if}
      {/each}
    {/if}
  {/if}

  {@render head('library', 'Media types')}
  {#if !folded('library')}
    <!-- Like every other filter: click to choose, click again to clear. Nothing chosen shows everything. -->
    <button class="item" class:active={kind === 'photo'} class:zero={facets && facets.photos === 0 && kind !== 'photo'} aria-pressed={kind === 'photo'} onclick={() => onkind(kind === 'photo' ? null : 'photo')}><span class="mark">{#if kind === 'photo'}<Check />{/if}</span><span class="grow">Photos</span>{#if facets}<span class="n">{facets.photos.toLocaleString()}</span>{/if}</button>
    <button class="item" class:active={kind === 'video'} class:zero={facets && facets.videos === 0 && kind !== 'video'} aria-pressed={kind === 'video'} onclick={() => onkind(kind === 'video' ? null : 'video')}><span class="mark">{#if kind === 'video'}<Check />{/if}</span><span class="grow">Videos</span>{#if facets}<span class="n">{facets.videos.toLocaleString()}</span>{/if}</button>
  {/if}

  <!-- A list of one camera is no choice: the section appears with two or more (or while one is chosen, to clear it). -->
  {#if cameras.length >= 2 || camera !== null}
    {@render head('cameras', 'Cameras')}
    {#if !folded('cameras')}
      {#each cameras as [name, n] (name)}
        <button class="item" class:active={camera === name} aria-pressed={camera === name} onclick={() => oncamera(camera === name ? null : name)}>
          <span class="mark">{#if camera === name}<Check />{/if}</span><span class="grow" use:clipTitle>{name}</span><span class="n">{n.toLocaleString()}</span>
        </button>
      {/each}
    {/if}
  {/if}

  {#if folders.length > 0}
    {@render head('folders', 'Folders')}
    {#if !folded('folders')}
      {#each folders as f (f.id)}
        {@const n = countOf(facets?.folders, f.id, f.count)}
        {@const note = folderNote(folderStates[f.id])}
        <button class="item" class:active={folderId === f.id} class:zero={facets && n === 0 && folderId !== f.id} class:offline={note !== null} aria-pressed={folderId === f.id} title={note ? `${f.path} — ${note}` : f.path} onclick={() => onfolder(folderId === f.id ? null : f.id)}>
          <span class="mark">{#if folderId === f.id}<Check />{/if}</span><span class="grow">{folderName(f.path)}</span>{#if note}<span class="offnote">{note}</span>{/if}<span class="n">{n.toLocaleString()}</span>
        </button>
      {/each}
    {/if}
  {/if}

</nav>

<style>
  nav { padding: 6px 4px 16px; font-size: 14px; }
  .head { display: flex; align-items: center; gap: 4px; width: 100%; text-align: left; margin: 2px 0 0; padding: 2px 8px; border: 0; border-radius: 4px; background: none; cursor: pointer; font: inherit;
          font-weight: 400; color: #222; }
  .head:hover { background: #eef2fb; }
  .head:first-child { margin-top: 6px; }
  .chosen { font-size: 12px; color: #2a5bd7; font-weight: 600; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item { display: flex; align-items: center; gap: 6px; width: 100%; text-align: left; padding: 4px 12px; border: 0; background: none; cursor: pointer; font: inherit; border-radius: 4px; }
  .grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #888; font-size: 12px; font-variant-numeric: tabular-nums; }
  /* Nothing here under the filters in force: shown, but quiet. */
  .zero { opacity: .45; }
  .more { color: #555; }
  .cat { font-weight: 600; color: #333; }
  .sub { padding-left: 28px; }
  .avatar { width: 22px; height: 22px; border-radius: 50%; object-fit: cover; background: #dde2ee; flex: none; margin-right: 6px; }
  .mark { width: 14px; flex: none; display: inline-flex; }
  .item:hover { background: #eef2fb; }
  .item.offline .grow, .item.offline .n { opacity: .55; }
  .offnote { font-size: 11px; color: #8a5a00; background: #fff1d6; border-radius: 8px; padding: 0 6px; white-space: nowrap; }
  .item.active { background: #e3ecff; font-weight: 600; }
  .hint { margin: 2px 12px 4px; font-size: 12px; color: #777; }
  .row { position: relative; }
  .edit { position: absolute; right: 36px; top: 50%; transform: translateY(-50%); border: 0; background: none; cursor: pointer; color: #777; font-size: 12px; padding: 2px 4px; visibility: hidden; }
  .row:hover .edit, .edit:focus-visible { visibility: visible; }
  .rename { padding-right: 8px; }
  .trip .grow { display: grid; line-height: 1.25; }
  .trip .when { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .trip .dest { font-size: 12px; font-weight: 400; color: #666; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rename input { flex: 1; min-width: 0; font: inherit; padding: 1px 4px; }
</style>
