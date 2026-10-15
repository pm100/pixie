<script lang="ts">
  import { listPhotos, type Photo } from './api';
  import { thumbUrl } from './url';

  let { index, total, onclose, onnav }: {
    index: number; total: number; onclose: () => void; onnav: (i: number) => void;
  } = $props();

  let photo = $state<Photo | null>(null);
  let failed = $state(false);

  $effect(() => {
    failed = false; photo = null;
    listPhotos(index, 1).then((r) => (photo = r[0] ?? null));
  });

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowRight' && index < total - 1) onnav(index + 1);
    else if (e.key === 'ArrowLeft' && index > 0) onnav(index - 1);
  }
</script>

<svelte:window onkeydown={key} />
<div class="overlay" role="dialog" aria-label="Photo viewer">
  <button class="close" onclick={onclose} aria-label="Close">✕</button>
  {#if photo && !failed}
    <img alt="" src={thumbUrl('view', photo.id, photo.mtime, photo.size)} onerror={() => (failed = true)} />
  {:else if failed}
    <p>This photo couldn't be opened. It may be damaged.</p>
  {/if}
  <div class="count">{index + 1} / {total}</div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: #111; display: grid; place-items: center; color: #eee; }
  img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .close { position: absolute; top: 12px; right: 16px; background: none; border: 0; color: #eee; font-size: 22px; cursor: pointer; }
  .count { position: absolute; bottom: 10px; font-size: 13px; opacity: .7; }
</style>
