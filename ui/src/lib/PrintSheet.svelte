<script lang="ts">
  /** One printed page: the photos laid out in a grid on a sheet of paper (sizes in millimetres). */
  import { basename, formatDateTime } from './format';
  import type { Photo } from './api';
  import { thumbUrl } from './url';

  let { photos, w, h, cols, rows, margin, fit, captions }: {
    photos: Photo[];
    w: number; h: number;
    cols: number; rows: number;
    /** Space round the edge of the page. */
    margin: number;
    fit: 'fill' | 'whole';
    captions: boolean;
  } = $props();
  const gap = $derived(cols * rows > 1 ? 4 : 0);
</script>

<div class="sheet" style="width:{w}mm;height:{h}mm;padding:{margin}mm;--cols:{cols};--rows:{rows};--gap:{gap}mm">
  {#each photos as p, i (i)}
    <figure class:whole={fit === 'whole'}>
      <img alt="" src={thumbUrl('view', p.id, p.mtime, p.size)} />
      {#if captions}<figcaption>{basename(p.path)} · {formatDateTime(p.taken_at)}</figcaption>{/if}
    </figure>
  {/each}
</div>

<style>
  .sheet { box-sizing: border-box; background: #fff; display: grid; grid-template-columns: repeat(var(--cols), minmax(0, 1fr)); grid-template-rows: repeat(var(--rows), minmax(0, 1fr)); gap: var(--gap); overflow: hidden; break-after: page; }
  figure { margin: 0; min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 1mm; }
  img { flex: 1; min-height: 0; width: 100%; object-fit: cover; display: block; }
  .whole img { object-fit: contain; }
  figcaption { flex: none; font: 9pt sans-serif; color: #333; text-align: center; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
