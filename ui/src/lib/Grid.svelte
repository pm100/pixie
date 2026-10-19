<script lang="ts">
  import { untrack } from 'svelte';
  import { listPhotos, type DayGroup, type Photo, type View } from './api';
  import { formatDay, formatDuration } from './format';
  import { columnsFor } from './gridMath';
  import { buildLayout, groupAt, groupTop, visibleRows } from './layout';
  import { PageCache } from './pageCache';
  import { makePoster } from './posterFrame';
  import { createPosterQueue } from './posterQueue';
  import { retryDelay } from './retry';
  import { posterUrl, thumbUrl } from './url';

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
  const cache = new PageCache<Photo>((o, l) => listPhotos(o, l, view.order, view.folderId, view.kind, view.bounds, view.maxId, view.since, view.until, view.search), PAGE, () => tick++);

  // Video tiles show a poster frame that the webview makes itself, one video at a time.
  // All of this is keyed by photo id: a grid index points at a different photo after a filter or sort change.
  let noPoster = $state<Record<number, boolean>>({}); // id -> the player cannot decode it
  let posterNonce = $state<Record<number, number>>({}); // id -> bumped to reload a poster that now exists
  const posterTries: Record<number, number> = {};
  const known: Record<number, Photo> = {};
  const posters = createPosterQueue((id) => makePoster(known[id]));
  function onPosterMissing(p: Photo) {
    known[p.id] = p;
    posterTries[p.id] = (posterTries[p.id] ?? 0) + 1;
    if (posterTries[p.id] > 2) { noPoster[p.id] = true; return; } // saved but still not served: stop
    posters.request(p.id).then((ok) => {
      if (ok) posterNonce[p.id] = (posterNonce[p.id] ?? 0) + 1; // reload the poster that now exists
      else if (stillShown(p.id)) noPoster[p.id] = true; // scrolled away: it may be asked again later
    });
  }

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

  const visibleVideoIds = $derived(new Set(
    shown.flatMap((r) => r.kind === 'cells'
      ? Array.from({ length: r.n }, (_, k) => item(r.first + k, tick)).filter((p): p is Photo => !!p && p.kind === 'video').map((p) => p.id)
      : []),
  ));
  const stillShown = (id: number) => visibleVideoIds.has(id);
  // A tile that scrolled away no longer needs its poster made (a job already running finishes).
  $effect(() => {
    const keep = visibleVideoIds;
    untrack(() => { for (const id of Object.keys(known).map(Number)) if (!keep.has(id)) posters.cancel(id); });
  });

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

  function unit(n: number): string {
    const [one, many] = view.kind === 'photo' ? ['photo', 'photos'] : view.kind === 'video' ? ['video', 'videos'] : ['item', 'items'];
    return n === 1 ? one : many;
  }

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
          <span>{groups[row.group].count.toLocaleString()} {unit(groups[row.group].count)}</span>
        </div>
      {:else}
        {#each { length: row.n } as _, k (row.first + k)}
          {@const i = row.first + k}
          {@const p = item(i, tick)}
          <button class="cell" class:sel={selected.has(i)}
                  style:top="{row.top}px" style:left="{k * (CELL + GAP)}px"
                  onclick={(e) => onselect(i, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })}
                  ondblclick={() => onopen(i)} onkeydown={(e) => key(e, i)}
                  aria-label={p ? (p.kind === 'video' ? `video ${p.path}` : p.path) : 'photo'} aria-pressed={selected.has(i)}>
            {#if p && p.kind === 'video'}
              {#if noPoster[p.id]}
                <span class="ph dark cam"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h9A1.5 1.5 0 0 1 15 6.5v2.6l5.2-3a.5.5 0 0 1 .8.4v11a.5.5 0 0 1-.8.4l-5.2-3v2.6a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 3 17.5z"/></svg></span>
              {:else}
                <img alt="" src={posterUrl(p.id, p.mtime, p.size, undefined, posterNonce[p.id] ?? 0)} onerror={() => onPosterMissing(p)} />
              {/if}
              <span class="badge" aria-hidden="true"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h9A1.5 1.5 0 0 1 15 6.5v2.6l5.2-3a.5.5 0 0 1 .8.4v11a.5.5 0 0 1-.8.4l-5.2-3v2.6a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 3 17.5z"/></svg></span>
              {#if p.duration_ms !== null}<span class="dur">{formatDuration(p.duration_ms)}</span>{/if}
            {:else if p && !broken[i]}
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
  .ph.dark { background: #2a2a30; color: #bbb; }
  .badge, .dur { position: absolute; bottom: 6px; background: rgba(0, 0, 0, .6); color: #fff; border-radius: 4px; font-size: 12px; line-height: 1; padding: 3px 5px; }
  .badge { left: 6px; display: flex; align-items: center; padding: 3px 5px; }
  .badge svg { width: 16px; height: 12px; }
  .cam svg { width: 36px; height: 36px; }
  .dur { right: 6px; font-variant-numeric: tabular-nums; }
</style>
