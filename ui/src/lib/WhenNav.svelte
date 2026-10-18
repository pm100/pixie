<script lang="ts">
  import { untrack } from 'svelte';
  import type { DayGroup } from './api';
  import { monthName } from './format';
  import { activeKeys, buildDateTree, type TreeNode } from './tree';
  import { dayBadge, maxMonthCount, monthCounts, yearEntries } from './whenNav';

  let { groups, activeGroup, ongroup, filter, period, onperiod }: {
    groups: DayGroup[]; activeGroup: number; ongroup: (group: number) => void;
    /** Filter mode (the map): a click chooses a period instead of scrolling the grid. */
    filter: boolean; period: string | null; onperiod: (key: string) => void;
  } = $props();

  const roots = $derived(buildDateTree(groups));
  const entries = $derived(yearEntries(roots, new Date().getFullYear()));
  const max = $derived(maxMonthCount(roots));
  // The keys to highlight: the chosen period and the year/month it sits in, or the place the grid has scrolled to.
  const periodKeys = $derived.by(() => {
    const out = new Set<string>();
    const m = period ? /^[ymd]:(\d{4})(?:-(\d{2}))?/.exec(period) : null;
    if (!period || !m) return out;
    out.add(`y:${m[1]}`);
    if (period[0] !== 'y' && m[2]) out.add(`m:${m[1]}-${m[2]}`);
    if (period[0] === 'd') out.add(period);
    return out;
  });
  const active = $derived(filter ? periodKeys : activeKeys(roots, activeGroup));

  // One year, one month and one decade are open at a time. They follow the grid as it scrolls.
  let openYear = $state<string | null>(null);
  let openMonth = $state<string | null>(null);
  let openDecade = $state<string | null>(null);

  $effect(() => {
    const keys = active;
    const list = entries;
    untrack(() => {
      for (const k of keys) {
        if (k.startsWith('y:')) openYear = k;
        else if (k.startsWith('m:')) openMonth = k;
      }
      for (const e of list) if (e.kind === 'decade' && e.years.some((y) => keys.has(y.key))) openDecade = e.key;
    });
  });

  const heightPct = (n: number) => (n === 0 ? 0 : Math.max(8, Math.round((n / max) * 100)));
  const monthNode = (year: TreeNode, i: number) => year.children.find((m) => Number(m.key.slice(-2)) === i + 1);

  function toggleYear(n: TreeNode) {
    openYear = openYear === n.key ? null : n.key;
    if (filter) onperiod(n.key);
    else ongroup(n.firstGroup);
  }
  function pickMonth(m: TreeNode) {
    openMonth = m.key;
    if (filter) onperiod(m.key);
    else ongroup(m.firstGroup);
  }
</script>

{#snippet yearBlock(n: TreeNode)}
  {@const counts = monthCounts(n)}
  <button class="yr" class:active={active.has(n.key)} onclick={() => toggleYear(n)} aria-expanded={openYear === n.key}>
    <span class="tw">{openYear === n.key ? '▾' : '▸'}</span>
    <span class="y">{n.label}</span>
    <span class="bars" aria-hidden="true">{#each counts as c}<i style:height="{heightPct(c)}%"></i>{/each}</span>
    <span class="n">{n.count.toLocaleString()}</span>
  </button>
  {#if openYear === n.key}
    <div class="months">
      {#each counts as c, i}
        {@const m = monthNode(n, i)}
        {#if m && c > 0}
          <button class="chip" class:on={filter ? active.has(m.key) : openMonth === m.key} onclick={() => pickMonth(m)} title="{monthName(i + 1)} {n.label}: {c}">{monthName(i + 1).slice(0, 3)}</button>
        {:else}
          <span class="chip none">{monthName(i + 1).slice(0, 3)}</span>
        {/if}
      {/each}
    </div>
    {#each n.children.filter((m) => m.key === openMonth) as m (m.key)}
      <div class="days">
        {#each m.children as d (d.key)}
          {@const b = dayBadge(d.key.slice(2))}
          <button class="day" class:active={active.has(d.key)} onclick={() => (filter ? onperiod(d.key) : ongroup(d.firstGroup))}>
            <span class="badge" class:we={b.weekend}><i>{b.weekday}</i><b>{b.dom}</b></span>
            <span class="place"></span>
            <span class="n">{d.count.toLocaleString()}</span>
          </button>
        {/each}
      </div>
    {/each}
  {/if}
{/snippet}

<div class="when">
  {#each entries as e (e.kind === 'decade' ? e.key : e.node.key)}
    {#if e.kind === 'year'}
      {@render yearBlock(e.node)}
    {:else if e.kind === 'decade'}
      <button class="yr" onclick={() => (openDecade = openDecade === e.key ? null : e.key)} aria-expanded={openDecade === e.key}>
        <span class="tw">{openDecade === e.key ? '▾' : '▸'}</span>
        <span class="y">{e.label}</span>
        <span class="n">{e.count.toLocaleString()}</span>
      </button>
      {#if openDecade === e.key}
        <div class="indent">
          {#each e.years as y (y.key)}{@render yearBlock(y)}{/each}
        </div>
      {/if}
    {:else}
      <button class="yr" class:active={active.has('undated')} disabled={filter} title={filter ? 'Items without a date have no place on the map timeline' : ''} onclick={() => ongroup(e.node.firstGroup)}>
        <span class="tw"></span><span class="y">Undated</span><span class="n">{e.node.count.toLocaleString()}</span>
      </button>
    {/if}
  {:else}
    <p class="empty">No photos yet.</p>
  {/each}
</div>

<style>
  .when { display: grid; gap: 1px; }
  .yr, .day { display: flex; align-items: center; gap: 6px; width: 100%; border: 0; background: none; font: inherit; text-align: left; padding: 4px 8px; border-radius: 4px; cursor: pointer; }
  .yr:hover, .day:hover, .chip:not(.none):hover { background: #eef2fb; }
  .yr.active, .day.active { background: #e3ecff; }
  .tw { width: 12px; flex: none; color: #555; }
  .y { font-weight: 600; }
  .n { margin-left: auto; color: #888; font-size: 12px; font-variant-numeric: tabular-nums; }
  .bars { display: flex; align-items: flex-end; gap: 1px; height: 12px; width: 40px; flex: none; }
  .bars i { flex: 1; background: #2f6fed; opacity: .6; border-radius: 1px 1px 0 0; }
  .months { display: grid; grid-template-columns: repeat(4, 1fr); gap: 3px; padding: 2px 8px 6px 22px; }
  .chip { text-align: center; border: 1px solid #cfd5e3; border-radius: 4px; padding: 2px 0; background: #fff; font: inherit; font-size: 12px; cursor: pointer; }
  .chip.none { color: #999; border-style: dashed; background: transparent; cursor: default; }
  .chip.on { background: #2f6fed; color: #fff; border-color: #2f6fed; }
  .days { margin: 0 8px 6px 22px; padding-left: 6px; border-left: 2px solid #2f6fed; display: grid; gap: 1px; }
  .day { padding: 2px 6px; }
  .badge { width: 30px; flex: none; text-align: center; border: 1px solid #d6dbe8; border-radius: 4px; background: #fff; line-height: 1.05; padding: 1px 0 2px; }
  .badge i { display: block; font-style: normal; font-size: 8px; letter-spacing: .06em; text-transform: uppercase; color: #888; }
  .badge b { font-size: 13px; font-variant-numeric: tabular-nums; }
  .badge.we i { color: #2f6fed; }
  .place { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: #555; }
  .indent { padding-left: 10px; }
  .empty { margin: 6px 12px; color: #888; }
</style>
