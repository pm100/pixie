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
      <button class="photo" onclick={() => onzoom(i)} title="See the whole photo this face is in" aria-label="See the whole photo">
        <img alt="" loading="lazy" decoding="async" src={faceUrl(t.id, undefined, 320)} />
      </button>
      <button class="tick" aria-pressed={picked.has(t.id)} aria-label="Select this face" title="Select (shift-click selects a run)" onclick={(e) => onclick(t.id, e.shiftKey)}>{picked.has(t.id) ? '✓' : ''}</button>
      {#if badge}<figcaption>{@render badge(t)}</figcaption>{/if}
    </figure>
  {/each}
</div>

<style>
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  figure { position: relative; margin: 0; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .photo { padding: 0; border: 3px solid transparent; border-radius: 10px; background: none; cursor: zoom-in; display: block; }
  figure.on .photo { border-color: #2f6fed; }
  .photo img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 7px; background: #dde2ee; display: block; }
  figure.on .photo img { opacity: .85; }
  .tick { position: absolute; left: 8px; top: 8px; width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 2px solid #fff; background: rgba(0, 0, 0, .35); color: #fff; font-size: 14px; line-height: 20px; text-align: center; box-sizing: border-box; cursor: pointer; }
  .tick:hover { background: rgba(0, 0, 0, .6); }
  figure.on .tick { background: #2f6fed; }
  figcaption { display: flex; flex-wrap: wrap; gap: 4px; font-size: 12px; min-height: 0; }
</style>
