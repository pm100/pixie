<script lang="ts">
  import { untrack } from 'svelte';
  import { listPhotos, type DayGroup, type Photo, type View } from './api';
  import { formatDay } from './format';
  import { columnsFor } from './gridMath';
  import { buildLayout, groupAt, groupTop, visibleRows } from './layout';
  import { PageCache } from './pageCache';
  import { retryDelay } from './retry';
  import { thumbUrl } from './url';

  let { groups, version, view, selected, scrollTo, onopen, onselect, onvisiblegroup }: {
    groups: DayGroup[];
    version: number;
    view: View;
    selected: ReadonlySet<number>;
    scrollTo: { group: number; n: number } | null;
    onopen: (index: number) => void;
    onselect: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
    onvisiblegroup: (group: number) => void;
  } = $props();

  const CELL = 180, GAP = 6, PAGE = 200, ROW_H = CELL + GAP, HEAD_H = 40;
  let width = $state(800), height = $state(600), scrollTop = $state(0);
  let scroller: HTMLDivElement;
  let tick = $state(0); // bumped whenever the page cache changes
  let broken = $state<Record<number, boolean>>({});
  let nonce = $state<Record<number, number>>({}); // bumps the URL to retry a failed thumbnail
  let attempts: Record<number, number> = {};
  // `view` is read at call time, so a fetch always uses the current order and folder.
  const cache = new PageCache<Photo>((o, l) => listPhotos(o, l, view.order, view.folderId), PAGE, () => tick++);

  // `_` is read only so the template re-renders when the cache changes.
  // A failed load is often just a busy queue: retry with backoff before giving up.
  function onImgError(i: number) {
    const n = attempts[i] ?? 0;
    attempts[i] = n + 1;
    const delay = retryDelay(n);
    if (delay === null) broken[i] = true;
    else setTimeout(() => (nonce[i] = (nonce[i] ?? 0) + 1), delay);
  }

  const item = (i: number, _: number) => cache.items[i];

  const cols = $derived(columnsFor(width, CELL, GAP));
  const layout = $derived(buildLayout(groups, cols, ROW_H, HEAD_H));
  const span = $derived(visibleRows(layout, scrollTop, height));
  const shown = $derived(layout.rows.slice(span[0], span[1]));
  const current = $derived(groupAt(layout, scrollTop));

  // Reset first, then load: both depend on `version`, and effects run in order,
  // so a version bump always refetches the visible pages.
  $effect(() => {
    version;
    untrack(() => { cache.reset(); broken = {}; nonce = {}; attempts = {}; });
  });

  $effect(() => {
    version;
    let lo = Infinity, hi = -1;
    for (const r of shown) {
      if (r.kind !== 'cells') continue;
      lo = Math.min(lo, r.first);
      hi = Math.max(hi, r.first + r.n);
    }
    if (hi > lo) cache.ensure(lo, hi);
  });

  // Tree clicks ask to jump to a group's heading.
  $effect(() => {
    const t = scrollTo;
    if (!t) return;
    untrack(() => { scroller.scrollTop = groupTop(layout, t.group); });
  });

  // Tell the tree which group is at the top, so it can highlight it.
  $effect(() => {
    if (current >= 0) onvisiblegroup(current);
  });

  function key(e: KeyboardEvent, i: number) {
    if (e.key === 'Enter') { e.preventDefault(); onopen(i); }
  }
</script>

<div class="scroller" bind:this={scroller} bind:clientWidth={width} bind:clientHeight={height}
     onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{layout.height}px">
    {#each shown as row (row.kind === 'heading' ? `h${row.group}` : `c${row.first}`)}
      {#if row.kind === 'heading'}
        <div class="head" style:top="{row.top}px" style:height="{row.h}px">
          <strong>{formatDay(groups[row.group].day)}</strong>
          <span>{groups[row.group].count} {groups[row.group].count === 1 ? 'photo' : 'photos'}</span>
        </div>
      {:else}
        {#each { length: row.n } as _, k (row.first + k)}
          {@const i = row.first + k}
          {@const p = item(i, tick)}
          <button class="cell" class:sel={selected.has(i)}
                  style:top="{row.top}px" style:left="{k * (CELL + GAP)}px"
                  onclick={(e) => onselect(i, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })}
                  ondblclick={() => onopen(i)} onkeydown={(e) => key(e, i)}
                  aria-label={p ? p.path : 'photo'} aria-pressed={selected.has(i)}>
            {#if p && !broken[i]}
              <img loading="lazy" alt="" src={thumbUrl('grid', p.id, p.mtime, p.size, undefined, nonce[i] ?? 0)} onerror={() => onImgError(i)} />
            {:else if broken[i]}
              <span class="ph">⚠︎</span>
            {/if}
          </button>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<style>
  .scroller { height: 100%; overflow-y: auto; }
  .inner { position: relative; }
  .head { position: absolute; left: 0; right: 0; display: flex; align-items: baseline; gap: 10px; padding: 14px 4px 4px; box-sizing: border-box; border-bottom: 1px solid #e4e4e8; }
  .head span { color: #777; font-size: 13px; }
  .cell { position: absolute; width: 180px; height: 180px; padding: 0; border: 0; background: #e8e8ec; border-radius: 6px; overflow: hidden; cursor: pointer; outline-offset: 0; }
  .cell.sel { outline: 3px solid #2f6fed; }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
</style>
