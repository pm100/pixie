<script lang="ts">
  import type { DayGroup, MediaKind } from './api';
  import Check from './Check.svelte';
  import WhenNav from './WhenNav.svelte';

  let { groups, activeGroup, kind, recent, scanLine, filterDates, period, recentText, ongroup, onperiod, onkind, onrecent }: {
    groups: DayGroup[]; activeGroup: number; kind: MediaKind | null; recent: boolean; scanLine: string | null;
    /** On the map, a date filters the pins and photos; elsewhere it scrolls the grid. */
    filterDates: boolean; period: string | null;
    /** The text of the recent row, e.g. "Last 30 days". */
    recentText: string;
    ongroup: (group: number) => void; onperiod: (key: string) => void;
    onkind: (k: MediaKind | null) => void; onrecent: (on: boolean) => void;
  } = $props();
</script>

<nav>
  <h3>Library</h3>
  <button class="item" class:active={kind === null} onclick={() => onkind(null)}><span class="mark">{#if kind === null}<Check />{/if}</span>All items</button>
  <button class="item" class:active={kind === 'photo'} onclick={() => onkind('photo')}><span class="mark">{#if kind === 'photo'}<Check />{/if}</span>Photos only</button>
  <button class="item" class:active={kind === 'video'} onclick={() => onkind('video')}><span class="mark">{#if kind === 'video'}<Check />{/if}</span>Videos only</button>

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
  .mark { width: 14px; flex: none; display: inline-flex; }
  .item:hover { background: #eef2fb; }
  .item.active { background: #e3ecff; font-weight: 600; }
  .scan { margin: 16px 12px 0; font-size: 12px; color: #777; }
</style>
