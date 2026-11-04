<script lang="ts">
  /** The photos the tools work on, as a grid of tiles that draws only the rows in view. It is given just the ids (any number of them) and
   *  reads the photos for the rows on screen, a screenful at a time, so a list of a hundred thousand is as quick as a list of ten. */
  import type { Photo } from './api';
  import { columnsFor, visibleRange } from './gridMath';
  import { posterUrl, thumbUrl } from './url';

  let { ids, chosen, load, onchoose, onopen, picked, onunpick }: {
    ids: number[];
    /** The photo clicked here (blue outline). */
    chosen: number | null;
    /** The photos for these ids (a screenful). */
    load: (ids: number[]) => Promise<Photo[]>;
    onchoose: (id: number) => void;
    /** Double-click or Enter: open the viewer at this position in the list. */
    onopen: (index: number) => void;
    /** The photos are the picked ones: each tile has a marker that takes it off. */
    picked: boolean;
    onunpick: (id: number) => void;
  } = $props();

  const CELL = 180;
  const GAP = 6;
  const ROW = CELL + GAP;
  const PAD_X = 14;
  const PAD_Y = 12;

  let box = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewH = $state(600);
  let viewW = $state(800);

  // The photos read so far, by id; `seen` makes the tiles draw again when more arrive.
  const cache = new Map<number, Photo>();
  let seen = $state(0);
  let broken = $state<Record<number, boolean>>({});

  /** The photo read for `id`, if it has been (`seen` is only there so that the tile draws again when more arrive). */
  const photoOf = (id: number, seen: number): Photo | undefined => (seen >= 0 ? cache.get(id) : undefined);

  const cols = $derived(columnsFor(viewW - 2 * PAD_X, CELL, GAP));
  const range = $derived(visibleRange(ids.length, cols, ROW, Math.max(0, scrollTop - PAD_Y), viewH, 2));
  const slice = $derived(ids.slice(range.firstIndex, range.endIndex));
  const height = $derived(range.rows * ROW + 2 * PAD_Y);

  // Another list: let go of photos no longer in it.
  $effect(() => {
    const keep = new Set(ids);
    for (const id of [...cache.keys()]) if (!keep.has(id)) cache.delete(id);
  });

  // Read the photos of the rows in view that are not here yet.
  $effect(() => {
    const need = slice.filter((id) => !cache.has(id));
    if (need.length === 0) return;
    let live = true;
    load(need).then((ps) => {
      if (!live) return;
      for (const p of ps) cache.set(p.id, p);
      seen++;
    }).catch(() => {});
    return () => { live = false; };
  });

  // Back to the top for a different list.
  let first: number | undefined;
  $effect(() => {
    const f = ids[0];
    if (first !== undefined && f !== first && box) box.scrollTop = 0;
    first = f;
  });
</script>

<div class="grid" bind:this={box} bind:clientHeight={viewH} bind:clientWidth={viewW} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{height}px">
    {#each slice as id, k (id)}
      {@const i = range.firstIndex + k}
      {@const p = photoOf(id, seen)}
      <div class="tile" class:chosen={chosen === id} style="left:{PAD_X + (i % cols) * ROW}px;top:{PAD_Y + Math.floor(i / cols) * ROW}px">
        {#if p}
          <!-- The whole photo is the button: one click chooses it, a double click or Enter opens it. The unpick marker is a separate button over its corner. -->
          <button class="open" aria-label={p.kind === 'video' ? `Video ${p.path}` : `Photo ${p.path}`} aria-pressed={chosen === id}
                  onclick={() => onchoose(id)} ondblclick={() => onopen(i)}
                  onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); onopen(i); } }}></button>
          {#if p.kind === 'video' && !broken[id]}
            <img alt="" src={posterUrl(id, p.mtime, p.size)} onerror={() => (broken[id] = true)} />
          {:else if p.kind === 'photo' && !broken[id]}
            <img alt="" src={thumbUrl('grid', id, p.mtime, p.size)} onerror={() => (broken[id] = true)} />
          {:else}
            <span class="ph" aria-hidden="true">{p.kind === 'video' ? '▶' : '⚠︎'}</span>
          {/if}
          {#if picked}
            <button class="pick on" aria-label="Take off Picked" title="Take off Picked" onclick={() => onunpick(id)}>
              <span class="tk" aria-hidden="true"><svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2.5l2.9 6.2 6.8.8-5 4.7 1.3 6.7L12 17.5 6 20.9l1.3-6.7-5-4.7 6.8-.8z" /></svg></span>
            </button>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .grid { flex: 1; min-height: 0; overflow-y: auto; position: relative; }
  .inner { position: relative; }
  .tile { position: absolute; width: 180px; height: 180px; border-radius: 6px; overflow: hidden; background: #e8e8ec; }
  .tile.chosen { outline: 3px solid #2f6fed; outline-offset: -3px; }
  .tile img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
  .open { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: none; cursor: pointer; }
  .open:focus-visible { outline: 3px solid #2f6fed; outline-offset: -3px; }
  .pick { position: absolute; z-index: 1; left: 0; top: 0; width: 46px; height: 46px; padding: 0; border: 0; background: none; cursor: pointer; }
  .tk { position: absolute; left: 8px; top: 8px; width: 22px; height: 22px; box-sizing: border-box; display: grid; place-items: center; border-radius: 50%; border: 2px solid #fff; background: #2f6fed; box-shadow: 0 1px 3px rgba(0, 0, 0, .5); color: #fff; opacity: 0; }
  .tk svg { width: 14px; height: 14px; }
  /* Like the grid: the marker shows while the mouse is over the corner (here it takes the photo off, so it turns red). */
  .pick:hover .tk, .pick:focus-visible .tk { opacity: 1; }
  .pick:hover .tk { background: #b3261e; }
</style>
