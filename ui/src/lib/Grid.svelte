<script lang="ts">
  import { untrack } from 'svelte';
  import { listPhotos, type Photo } from './api';
  import { columnsFor, visibleRange } from './gridMath';
  import { PageCache } from './pageCache';
import { retryDelay } from './retry';
  import { thumbUrl } from './url';

  let { total, version, onopen }: { total: number; version: number; onopen: (index: number) => void } = $props();

  const CELL = 180, GAP = 6, PAGE = 200, ROW_H = CELL + GAP;
  let width = $state(800), height = $state(600), scrollTop = $state(0);
  let tick = $state(0); // bumped whenever the page cache changes
  let broken = $state<Record<number, boolean>>({});
  let nonce = $state<Record<number, number>>({}); // bumps the URL to retry a failed thumbnail
  let attempts: Record<number, number> = {};
  const cache = new PageCache<Photo>(listPhotos, PAGE, () => tick++);

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
  const range = $derived(visibleRange(total, cols, ROW_H, scrollTop, height));
  const indices = $derived(Array.from({ length: range.endIndex - range.firstIndex }, (_, i) => range.firstIndex + i));

  // Reset first, then load: both depend on `version`, and effects run in order,
  // so a version bump always refetches the visible pages.
  $effect(() => {
    version;
    untrack(() => { cache.reset(); broken = {}; nonce = {}; attempts = {}; });
  });

  $effect(() => {
    version;
    cache.ensure(range.firstIndex, range.endIndex);
  });
</script>

<div class="scroller" bind:clientWidth={width} bind:clientHeight={height}
     onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{range.rows * ROW_H}px">
    {#each indices as i (i)}
      {@const p = item(i, tick)}
      <button class="cell" style:top="{Math.floor(i / cols) * ROW_H}px" style:left="{(i % cols) * (CELL + GAP)}px"
              onclick={() => onopen(i)} aria-label={p ? p.path : 'photo'}>
        {#if p && !broken[i]}
          <img loading="lazy" alt="" src={thumbUrl('grid', p.id, p.mtime, p.size, undefined, nonce[i] ?? 0)} onerror={() => onImgError(i)} />
        {:else if broken[i]}
          <span class="ph">⚠︎</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .scroller { height: 100%; overflow-y: auto; }
  .inner { position: relative; }
  .cell { position: absolute; width: 180px; height: 180px; padding: 0; border: 0; background: #e8e8ec; border-radius: 6px; overflow: hidden; cursor: pointer; }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
</style>
