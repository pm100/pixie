<script lang="ts">
  import { untrack } from 'svelte';
  import type { DayGroup } from './api';
  import { monthName } from './format';
  import { buildDateTree, type TreeNode } from './tree';
  import Check from './Check.svelte';
  import Chevron from './Chevron.svelte';
  import { dayBadge, monthCounts, yearEntries } from './whenNav';

  let { groups, period, onperiod }: {
    /** The day groups to list: the whole library under the other filters (the date filter itself is left out, so every
     *  other date stays there to choose). */
    groups: DayGroup[];
    /** The chosen period (a filter, like any other), if any: `y:2025`, `m:2025-06`, `d:2025-06-14` or `undated`. */
    period: string | null;
    /** A click on a year, month, day or Undated chooses it as the date filter (the same one again clears it). */
    onperiod: (key: string) => void;
  } = $props();

  const roots = $derived(buildDateTree(groups));
  const entries = $derived(yearEntries(roots));
  // The keys to mark: the chosen period and the year and month it sits in.
  const marked = $derived.by(() => {
    const out = new Set<string>();
    if (period === 'undated') return out.add('undated');
    const m = period ? /^[ymd]:(\d{4})(?:-(\d{2}))?/.exec(period) : null;
    if (!period || !m) return out;
    out.add(`y:${m[1]}`);
    if (period[0] !== 'y' && m[2]) out.add(`m:${m[1]}-${m[2]}`);
    if (period[0] === 'd') out.add(period);
    return out;
  });

  // One year and one month are open at a time; the year and month of the chosen period are opened.
  let openYear = $state<string | null>(null);
  let openMonth = $state<string | null>(null);

  $effect(() => {
    const keys = marked;
    untrack(() => {
      for (const k of keys) {
        if (k.startsWith('y:')) openYear = k;
        else if (k.startsWith('m:')) openMonth = k;
      }
    });
  });

  const monthNode = (year: TreeNode, i: number) => year.children.find((m) => Number(m.key.slice(-2)) === i + 1);

  function toggleYear(n: TreeNode) {
    openYear = openYear === n.key ? null : n.key;
    onperiod(n.key);
  }
  function pickMonth(m: TreeNode) {
    openMonth = m.key;
    onperiod(m.key);
  }
</script>

{#snippet yearBlock(n: TreeNode)}
  {@const counts = monthCounts(n)}
  <button class="yr" class:active={marked.has(n.key)} onclick={() => toggleYear(n)} aria-expanded={openYear === n.key} title="Show only this date, and open it to see the days inside">
    <Chevron open={openYear === n.key} />
    <span class="y">{n.label}</span>
    {#if marked.has(n.key)}<Check />{/if}
    <span class="n">{n.count.toLocaleString()}</span>
  </button>
  {#if openYear === n.key}
    <div class="months">
      {#each counts as c, i}
        {@const m = monthNode(n, i)}
        {#if m && c > 0}
          <button class="chip" class:on={marked.has(m.key)} onclick={() => pickMonth(m)} title="{monthName(i + 1)} {n.label}: {c}">{monthName(i + 1).slice(0, 3)}{#if marked.has(m.key)}<Check />{/if}</button>
        {:else}
          <span class="chip none">{monthName(i + 1).slice(0, 3)}</span>
        {/if}
      {/each}
    </div>
    {#each n.children.filter((m) => m.key === openMonth) as m (m.key)}
      <div class="days">
        {#each m.children as d (d.key)}
          {@const b = dayBadge(d.key.slice(2))}
          <button class="day" class:active={marked.has(d.key)} title="Show only the photos from this day" onclick={() => onperiod(d.key)}>
            <span class="badge" class:we={b.weekend}><i>{b.weekday}</i><b>{b.dom}</b></span>
            <span class="place"></span>
            {#if marked.has(d.key)}<Check />{/if}
            <span class="n">{d.count.toLocaleString()}</span>
          </button>
        {/each}
      </div>
    {/each}
  {/if}
{/snippet}

<div class="when">
  {#each entries as e (e.node.key)}
    {#if e.kind === 'year'}
      {@render yearBlock(e.node)}
    {:else}
      <button class="yr" class:active={marked.has('undated')} aria-pressed={marked.has('undated')} title="Show only the photos that have no date" onclick={() => onperiod('undated')}>
        <span class="tw"></span><span class="y">Undated</span>{#if marked.has('undated')}<Check />{/if}<span class="n">{e.node.count.toLocaleString()}</span>
      </button>
    {/if}
  {:else}
    <p class="empty">No photos yet.</p>
  {/each}
</div>

<style>
  .when { display: grid; gap: 1px; }
  /* The same rows and indents as the other trees in the list (Auto tags categories, trip years). */
  .yr, .day { display: flex; align-items: center; gap: 4px; width: 100%; border: 0; background: none; font: inherit; text-align: left; padding: 4px 8px 4px 20px; border-radius: 4px; cursor: pointer; }
  .yr:hover, .day:hover, .chip:not(.none):hover { background: #eef2fb; }
  .yr.active, .day.active { background: #e3ecff; }
  .tw { width: 16px; flex: none; }
  .y { font-weight: 600; color: #333; }
  .n { margin-left: auto; color: #888; font-size: 12px; font-variant-numeric: tabular-nums; }
  /* Month tiles keep one size whatever the width of the pane: always four to a row, so three rows. */
  .months { display: grid; grid-template-columns: repeat(4, 42px); justify-content: start; gap: 3px; padding: 2px 8px 6px 32px; }
  .chip { text-align: center; border: 1px solid #cfd5e3; border-radius: 4px; padding: 2px 0; background: #fff; font: inherit; font-size: 12px; cursor: pointer; }
  .chip.none { color: #999; border-style: dashed; background: transparent; cursor: default; }
  /* Chosen: the same light blue and bold as every other chosen row in the list. */
  .chip.on { background: #e3ecff; border-color: #bcd0f7; font-weight: 600; }
  .chip { display: inline-flex; align-items: center; justify-content: center; gap: 2px; }
  .chip :global(.check) { width: 11px; height: 11px; }
  .days { margin: 0 8px 6px 32px; padding-left: 6px; border-left: 2px solid #2f6fed; display: grid; gap: 1px; }
  .day { padding: 2px 6px; }
  .badge { width: 30px; flex: none; text-align: center; border: 1px solid #d6dbe8; border-radius: 4px; background: #fff; line-height: 1.05; padding: 1px 0 2px; }
  .badge i { display: block; font-style: normal; font-size: 8px; letter-spacing: .06em; text-transform: uppercase; color: #888; }
  .badge b { font-size: 13px; font-variant-numeric: tabular-nums; }
  .badge.we i { color: #2f6fed; }
  .place { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: #555; }
  .empty { margin: 6px 8px 6px 20px; color: #888; }
</style>
