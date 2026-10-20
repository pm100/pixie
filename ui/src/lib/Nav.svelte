<script lang="ts">
  import type { DayGroup, MediaKind } from './api';
  import { untrack } from 'svelte';
  import Check from './Check.svelte';
  import WhenNav from './WhenNav.svelte';
  import { categoryOf, tagLabel, tagTree, type TagCount } from './tagNav';
  import { personLabel, personRows, type PersonInfo } from './people';
  import { faceUrl } from './url';

  let { groups, activeGroup, kind, recent, scanLine, filterDates, period, recentText, tags, tag, people, facesFound, person, ongroup, onperiod, onkind, onrecent, ontag, onperson }: {
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
    ongroup: (group: number) => void; onperiod: (key: string) => void;
    onkind: (k: MediaKind | null) => void; onrecent: (on: boolean) => void;
  } = $props();

  let allPeople = $state(false);
  /** Auto tags categories the user has opened; the one holding the chosen tag is open as well. */
  let openCategories = $state<Set<string>>(new Set());
  const shownPeople = $derived(personRows(people, person, allPeople, 8));
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
</script>

<nav>
  <h3>Library</h3>
  <button class="item" class:active={kind === null} onclick={() => onkind(null)}><span class="mark">{#if kind === null}<Check />{/if}</span>All items</button>
  <button class="item" class:active={kind === 'photo'} onclick={() => onkind('photo')}><span class="mark">{#if kind === 'photo'}<Check />{/if}</span>Photos only</button>
  <button class="item" class:active={kind === 'video'} onclick={() => onkind('video')}><span class="mark">{#if kind === 'video'}<Check />{/if}</span>Videos only</button>

  {#if people.length === 0 && facesFound}
    <h3>People</h3>
    <p class="hint">Nobody is named yet. Name people in the People tab and they will appear here.</p>
  {:else if people.length > 0}
    <h3>People</h3>
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

  {#if tags.length > 0}
    <h3>Auto tags</h3>
    {#each tree as g (g.category)}
      <button class="item cat" aria-expanded={isOpen(g.category)} onclick={() => toggleCategory(g.category)}>
        <span class="chev" class:open={isOpen(g.category)} aria-hidden="true">▸</span><span class="grow">{g.category}</span><span class="n">{g.photos.toLocaleString()}</span>
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

  <h3>When</h3>
  <button class="item" class:active={recent} aria-pressed={recent} onclick={() => onrecent(!recent)}><span class="mark">{#if recent}<Check />{/if}</span>{recentText}</button>
  <WhenNav {groups} {activeGroup} {ongroup} filter={filterDates} {period} {onperiod} {recent} />

  {#if scanLine}<p class="scan">{scanLine}</p>{/if}
</nav>

<style>
  nav { padding: 6px 4px 16px; font-size: 14px; }
  h3 { margin: 14px 8px 4px; font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: #777; }
  h3:first-child { margin-top: 6px; }
  .item { display: flex; align-items: center; gap: 6px; width: 100%; text-align: left; padding: 4px 12px; border: 0; background: none; cursor: pointer; font: inherit; border-radius: 4px; }
  .grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #888; font-size: 12px; font-variant-numeric: tabular-nums; }
  .more { color: #555; }
  .cat { font-weight: 600; color: #333; }
  .chev { width: 14px; flex: none; text-align: center; font-size: 11px; color: #777; transition: transform .1s; }
  .chev.open { transform: rotate(90deg); }
  .sub { padding-left: 28px; }
  .avatar { width: 22px; height: 22px; border-radius: 50%; object-fit: cover; background: #dde2ee; flex: none; margin-right: 6px; }
  .mark { width: 14px; flex: none; display: inline-flex; }
  .item:hover { background: #eef2fb; }
  .item.active { background: #e3ecff; font-weight: 600; }
  .hint { margin: 2px 12px 4px; font-size: 12px; color: #777; }
  .scan { margin: 16px 12px 0; font-size: 12px; color: #777; }
</style>
