<script lang="ts">
  /** The photos the Collage tool works on (the Working-on set), on the right of the page. Drag one onto the page, or double-click it, to add it. */
  import type { Photo } from './api';
  import { plural } from './format';
  import { thumbUrl } from './url';

  let { photos, onPage, onadd }: {
    photos: Photo[];
    /** The photos that have a frame on the page. */
    onPage: ReadonlySet<number>;
    onadd: (photoId: number) => void;
  } = $props();

  function dragStart(e: DragEvent, p: Photo) {
    if (!e.dataTransfer) return;
    e.dataTransfer.setData('application/x-pixie-photo', String(p.id));
    e.dataTransfer.effectAllowed = 'copy';
  }

  const name = (p: Photo) => p.path.split(/[\\/]/).pop() ?? p.path;
</script>

<div class="list">
  <div class="head">
    <b>Working on</b>
    <span class="count">{plural(photos.length, 'photo')} · {onPage.size} on the page</span>
  </div>

  <div class="tiles">
    {#each photos as p (p.id)}
      <button class="tile" aria-label={`Photo ${name(p)}${onPage.has(p.id) ? ', on the page' : ''}`}
              title="Drag onto the page to add it, or double-click it"
              draggable="true" ondragstart={(e) => dragStart(e, p)} ondblclick={() => onadd(p.id)}>
        <img alt="" loading="lazy" draggable="false" src={thumbUrl('grid', p.id, p.mtime, p.size)} />
        {#if onPage.has(p.id)}<span class="on" aria-hidden="true">✓</span>{/if}
      </button>
    {/each}
  </div>

  {#if photos.length === 0}
    <p class="none">No photos to use. Pick photos in Browse, or choose another set under Working on.</p>
  {/if}

</div>

<style>
  .list { width: 100%; height: 100%; display: flex; flex-direction: column; background: #fafafb; border-left: 1px solid #e4e4e8; font-size: 14px; min-height: 0; }
  .head { padding: 10px 12px 6px; display: flex; flex-direction: column; gap: 2px; }
  .count { color: #777; font-size: 12px; }
  .tiles { flex: 1; min-height: 0; overflow-y: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 6px; padding: 4px 12px 12px; align-content: start; grid-auto-rows: max-content; }
  /* The rows are sized from their content (grid-auto-rows above): tiles hide their overflow, which would otherwise let a row shrink to nothing to fit the list's height. The tile is as tall as it is wide because of the empty block inside it (::before). */
  .tile { position: relative; align-self: start; padding: 0; border: 2px solid transparent; border-radius: 4px; background: #e8e8ec; overflow: hidden; cursor: grab; }
  .tile::before { content: ''; display: block; padding-top: 100%; }
  .tile img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; display: block; pointer-events: none; }
  .on { position: absolute; right: 3px; top: 3px; width: 18px; height: 18px; display: grid; place-items: center; border-radius: 50%; background: #2f6fed; color: #fff; font-size: 12px; line-height: 1; }
  .none { margin: 0; padding: 0 12px; color: #777; font-size: 13px; }
</style>
