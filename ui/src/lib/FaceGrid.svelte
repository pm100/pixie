<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { FaceTile } from './people';
  import { faceUrl } from './url';

  let { tiles, picked, onclick, onzoom, badge }: {
    tiles: FaceTile[];
    picked: ReadonlySet<number>;
    /** A face was clicked (`shift`: pick everything since the last click). */
    onclick: (id: number, shift: boolean) => void;
    /** Show the whole photo this face is in. */
    onzoom: (index: number) => void;
    /** Something under a face (who it looks like, say). */
    badge?: Snippet<[FaceTile]>;
  } = $props();
</script>

<div class="tiles">
  {#each tiles as t, i (t.id)}
    <figure class:on={picked.has(t.id)}>
      <button class="pick" aria-pressed={picked.has(t.id)} aria-label="Pick this face" onclick={(e) => onclick(t.id, e.shiftKey)}>
        <img alt="" loading="lazy" decoding="async" src={faceUrl(t.id, undefined, 320)} />
        <span class="tick" aria-hidden="true">{picked.has(t.id) ? '✓' : ''}</span>
      </button>
      <button class="zoom" onclick={() => onzoom(i)} title="See the whole photo this face is in" aria-label="See the whole photo">⤢</button>
      {#if badge}<figcaption>{@render badge(t)}</figcaption>{/if}
    </figure>
  {/each}
</div>

<style>
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  figure { position: relative; margin: 0; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .pick { padding: 0; border: 3px solid transparent; border-radius: 10px; background: none; cursor: pointer; display: block; position: relative; }
  figure.on .pick { border-color: #2f6fed; }
  .pick img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 7px; background: #dde2ee; display: block; }
  figure.on .pick img { opacity: .85; }
  .tick { position: absolute; left: 6px; top: 6px; width: 22px; height: 22px; border-radius: 50%; border: 2px solid #fff; background: rgba(0, 0, 0, .35); color: #fff; font-size: 14px; line-height: 18px; text-align: center; box-sizing: border-box; }
  figure.on .tick { background: #2f6fed; }
  .zoom { position: absolute; right: 6px; top: 6px; width: 24px; height: 24px; padding: 0; border: 0; border-radius: 6px; background: rgba(0, 0, 0, .5); color: #fff; font-size: 15px; line-height: 1; cursor: zoom-in; opacity: 0; }
  figure:hover .zoom, .zoom:focus-visible { opacity: 1; }
  figcaption { display: flex; flex-wrap: wrap; gap: 4px; font-size: 12px; min-height: 0; }
</style>
