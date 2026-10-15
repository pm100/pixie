<script lang="ts">
  import { listPhotos, type Photo } from './api';
  import { columnsFor, visibleRange } from './gridMath';
  import { thumbUrl } from './url';

  let { total, version, onopen }: { total: number; version: number; onopen: (index: number) => void } = $props();

  const CELL = 180, GAP = 6, PAGE = 200, ROW_H = CELL + GAP;
  let width = $state(800), height = $state(600), scrollTop = $state(0);
  let cache = $state<Record<number, Photo>>({});
  let broken = $state<Record<number, boolean>>({});
  const requested = new Set<number>();

  const cols = $derived(columnsFor(width, CELL, GAP));
  const range = $derived(visibleRange(total, cols, ROW_H, scrollTop, height));
  const indices = $derived(Array.from({ length: range.endIndex - range.firstIndex }, (_, i) => range.firstIndex + i));

  $effect(() => {
    version; // reset when the library changes
    cache = {}; broken = {}; requested.clear();
  });

  $effect(() => {
    if (range.endIndex === 0) return;
    for (let page = Math.floor(range.firstIndex / PAGE); page <= Math.floor((range.endIndex - 1) / PAGE); page++) {
      if (requested.has(page)) continue;
      requested.add(page);
      listPhotos(page * PAGE, PAGE).then((photos) => {
        const next = { ...cache };
        photos.forEach((p, i) => (next[page * PAGE + i] = p));
        cache = next;
      }).catch(() => requested.delete(page));
    }
  });
</script>

<div class="scroller" bind:clientWidth={width} bind:clientHeight={height}
     onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{range.rows * ROW_H}px">
    {#each indices as i (i)}
      {@const p = cache[i]}
      <button class="cell" style:top="{Math.floor(i / cols) * ROW_H}px" style:left="{(i % cols) * (CELL + GAP)}px"
              onclick={() => onopen(i)} aria-label={p ? p.path : 'photo'}>
        {#if p && !broken[i]}
          <img loading="lazy" alt="" src={thumbUrl('grid', p.id, p.mtime)} onerror={() => (broken[i] = true)} />
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
