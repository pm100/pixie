<script lang="ts">
  import type { DayGroup, FolderInfo, MediaKind, Trip } from './api';
  import { untrack } from 'svelte';
  import Check from './Check.svelte';
  import Chevron from './Chevron.svelte';
  import WhenNav from './WhenNav.svelte';
  import { folderName } from './format';
  import { periodLabel } from './periods';
  import { categoryOf, tagLabel, tagTree, type TagCount } from './tagNav';
  import { personLabel, personRows, type PersonInfo } from './people';
  import type { SectionId } from './sections';
  import { nameFromEdit, tripEditText, tripLines, tripSummary, tripTitle, tripYear, tripYears } from './trips';
  import { faceUrl } from './url';

  let { groups, activeGroup, kind, recent, scanLine, filterDates, period, recentText, tags, tag, people, facesFound, person, folders, folderId, trips, tripsReady, trip, destinations, onvisibletrips, collapsed, ongroup, onperiod, onkind, onrecent, ontag, onperson, onfolder, ontrip, onrenametrip, onsettrips, ontoggle }: {
    groups: DayGroup[]; activeGroup: number; kind: MediaKind | null; recent: boolean; scanLine: string | null;
    /** On the map, a date filters the pins and photos; elsewhere it scrolls the grid. */
    filterDates: boolean; period: string | null;
    /** The text of the recent row, e.g. "Last 30 days". */
    recentText: string;
    /** What the photos show, with counts, and the one chosen as a filter (if any). */
    tags: TagCount[]; tag: string | null; ontag: (t: string | null) => void;
    /** The people found in the photos, and the one chosen as a filter (if any). */
    people: PersonInfo[]; person: number | null; onperson: (id: number | null) => void;
    /** Some photos have been checked for faces: with nobody named yet, the section says where to name people. */
    facesFound: boolean;
    /** The photo folders, with the item count of each, and the one chosen (if any). */
    folders: FolderInfo[]; folderId: number | null; onfolder: (id: number | null) => void;
    /** Trips away from home. `tripsReady` is false until the user has confirmed where home is. `trip` is the chosen one's `since`. */
    trips: Trip[]; tripsReady: boolean; trip: number | null; ontrip: (t: Trip | null) => void;
    /** Where each trip went (by its `since`), once known. `onvisibletrips` says which trips are showing, so those are named first. */
    destinations: Record<number, string>; onvisibletrips: (t: Trip[]) => void;
    onrenametrip: (t: Trip, name: string) => void; onsettrips: () => void;
    /** Sections the user has folded away, and the click on a heading. */
    collapsed: SectionId[]; ontoggle: (id: SectionId) => void;
    ongroup: (group: number) => void; onperiod: (key: string) => void;
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
    {#if folded(id) && chosen[id]}<span class="chosen" title="Chosen in this section">· {chosen[id]}</span>{/if}
  </button>
{/snippet}

<nav>
  {@render head('library', 'Media types')}
  {#if !folded('library')}
    <button class="item" class:active={kind === null} onclick={() => onkind(null)}><span class="mark">{#if kind === null}<Check />{/if}</span>All items</button>
    <button class="item" class:active={kind === 'photo'} onclick={() => onkind('photo')}><span class="mark">{#if kind === 'photo'}<Check />{/if}</span>Photos only</button>
    <button class="item" class:active={kind === 'video'} onclick={() => onkind('video')}><span class="mark">{#if kind === 'video'}<Check />{/if}</span>Videos only</button>
  {/if}

  {#if people.length === 0 && facesFound}
    {@render head('people', 'People')}
    {#if !folded('people')}<p class="hint">Nobody is named yet. Name people in the People tab and they will appear here.</p>{/if}
  {:else if people.length > 0}
    {@render head('people', 'People')}
    {#if !folded('people')}
      {#each shownPeople.rows as p (p.id)}
        <button class="item" class:active={person === p.id} aria-pressed={person === p.id} onclick={() => onperson(person === p.id ? null : p.id)}>
          <span class="mark">{#if person === p.id}<Check />{/if}</span>
          {#if p.face_id !== null}<img class="avatar" alt="" src={faceUrl(p.face_id)} />{:else}<span class="avatar"></span>{/if}
          <span class="grow">{personLabel(p)}</span><span class="n">{p.count.toLocaleString()}</span>
        </button>
      {/each}
      {#if shownPeople.hidden > 0 || allPeople}
        <button class="item more" onclick={() => (allPeople = !allPeople)}><span class="mark"></span>{allPeople ? 'Show fewer' : `Show all ${people.length} people`}</button>
      {/if}
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
        <button class="item cat" aria-expanded={openYears.has(g.year)} onclick={() => toggleYear(g.year)}>
          <Chevron open={openYears.has(g.year)} /><span class="grow">{g.year}</span><span class="n">{g.trips.length.toLocaleString()}</span>
        </button>
        {#if openYears.has(g.year)}
          {#each g.trips as t (t.since)}
            {@const lines = tripLines(t, destinations[t.since])}
            {#if renaming === t.since}
              <div class="item sub rename"><span class="mark"></span>
                <input use:focusOnMount bind:value={renameText} aria-label="Trip name" placeholder={tripEditText({ ...t, name: null }, destinations[t.since])} maxlength="120"
                       onkeydown={(e) => { if (e.key === 'Enter') endRename(true, t); else if (e.key === 'Escape') endRename(false, t); }} onblur={() => endRename(true, t)} />
              </div>
            {:else}
              <div class="row">
                <button class="item sub trip" class:active={trip === t.since} aria-pressed={trip === t.since} title={tripSummary(t)} onclick={() => ontrip(trip === t.since ? null : t)}>
                  <span class="mark">{#if trip === t.since}<Check />{/if}</span>
                  <span class="grow"><span class="when">{lines[0]}</span>{#if lines[1]}<span class="dest">{lines[1]}</span>{/if}</span>
                  <span class="n">{(t.photos + t.videos).toLocaleString()}</span>
                </button>
                <button class="edit" aria-label="Rename this trip" title="Rename this trip" onclick={() => startRename(t)}>✎</button>
              </div>
            {/if}
          {/each}
        {/if}
      {/each}
    {/if}
  {/if}

  {#if tags.length > 0}
    {@render head('tags', 'Auto tags')}
    {#if !folded('tags')}
      {#each tree as g (g.category)}
        <button class="item cat" aria-expanded={isOpen(g.category)} onclick={() => toggleCategory(g.category)}>
          <Chevron open={isOpen(g.category)} /><span class="grow">{g.category}</span><span class="n">{g.photos.toLocaleString()}</span>
        </button>
        {#if isOpen(g.category)}
          {#each g.tags as t (t.tag)}
            <button class="item sub" class:active={tag === t.tag} aria-pressed={tag === t.tag} onclick={() => ontag(tag === t.tag ? null : t.tag)}>
              <span class="mark">{#if tag === t.tag}<Check />{/if}</span><span class="grow">{tagLabel(t.tag)}</span><span class="n">{t.count.toLocaleString()}</span>
            </button>
          {/each}
        {/if}
      {/each}
    {/if}
  {/if}

  {@render head('when', 'When')}
  {#if !folded('when')}
    <button class="item" class:active={recent} aria-pressed={recent} onclick={() => onrecent(!recent)}><span class="mark">{#if recent}<Check />{/if}</span>{recentText}</button>
    <WhenNav {groups} {activeGroup} {ongroup} filter={filterDates} {period} {onperiod} {recent} />
  {/if}

  {#if folders.length > 0}
    {@render head('folders', 'Folders')}
    {#if !folded('folders')}
      {#each folders as f (f.id)}
        <button class="item" class:active={folderId === f.id} aria-pressed={folderId === f.id} title={f.path} onclick={() => onfolder(folderId === f.id ? null : f.id)}>
          <span class="mark">{#if folderId === f.id}<Check />{/if}</span><span class="grow">{folderName(f.path)}</span><span class="n">{f.count.toLocaleString()}</span>
        </button>
      {/each}
    {/if}
  {/if}

  {#if scanLine}<p class="scan">{scanLine}</p>{/if}
</nav>

<style>
  nav { padding: 6px 4px 16px; font-size: 14px; }
  .head { display: flex; align-items: center; gap: 4px; width: 100%; text-align: left; margin: 14px 0 4px; padding: 0 8px; border: 0; background: none; cursor: pointer; font: inherit;
          font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: #777; }
  .head:hover { color: #333; }
  .head:first-child { margin-top: 6px; }
  .chosen { text-transform: none; letter-spacing: 0; font-size: 12px; color: #2a5bd7; font-weight: 600; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item { display: flex; align-items: center; gap: 6px; width: 100%; text-align: left; padding: 4px 12px; border: 0; background: none; cursor: pointer; font: inherit; border-radius: 4px; }
  .grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #888; font-size: 12px; font-variant-numeric: tabular-nums; }
  .more { color: #555; }
  .cat { font-weight: 600; color: #333; }
  .sub { padding-left: 28px; }
  .avatar { width: 22px; height: 22px; border-radius: 50%; object-fit: cover; background: #dde2ee; flex: none; margin-right: 6px; }
  .mark { width: 14px; flex: none; display: inline-flex; }
  .item:hover { background: #eef2fb; }
  .item.active { background: #e3ecff; font-weight: 600; }
  .hint { margin: 2px 12px 4px; font-size: 12px; color: #777; }
  .scan { margin: 16px 12px 0; font-size: 12px; color: #777; }
  .row { position: relative; }
  .edit { position: absolute; right: 36px; top: 50%; transform: translateY(-50%); border: 0; background: none; cursor: pointer; color: #777; font-size: 12px; padding: 2px 4px; visibility: hidden; }
  .row:hover .edit, .edit:focus-visible { visibility: visible; }
  .rename { padding-right: 8px; }
  .trip .grow { display: grid; line-height: 1.25; }
  .trip .when { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .trip .dest { font-size: 12px; font-weight: 400; color: #666; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rename input { flex: 1; min-width: 0; font: inherit; padding: 1px 4px; }
</style>
