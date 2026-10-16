<script lang="ts">
  import { listPhotos, type Photo } from './api';
  import { neighbors } from './neighbors';
  import { thumbUrl } from './url';

  let { index, total, onclose, onnav }: {
    index: number; total: number; onclose: () => void; onnav: (i: number) => void;
  } = $props();

  const PRELOAD = 3; // photos kept ready on each side of the current one

  let shownSrc = $state<string | null>(null); // last photo that finished loading
  let failed = $state(false);
  const photos: Record<number, Photo> = {};
  let token = 0; // latest navigation wins; older loads are ignored

  const urlFor = (p: Photo) => thumbUrl('view', p.id, p.mtime, p.size);

  function load(src: string, mine: number) {
    const img = new Image();
    img.onload = () => { if (mine === token) { shownSrc = src; failed = false; } };
    img.onerror = () => { if (mine === token) failed = true; };
    img.src = src;
  }

  $effect(() => {
    const i = index;
    const mine = ++token;
    // One call for the whole neighbourhood instead of one per arrow press.
    const lo = Math.max(0, i - PRELOAD);
    const hi = Math.min(total, i + PRELOAD + 1);
    listPhotos(lo, hi - lo).then((rows) => {
      rows.forEach((p, k) => (photos[lo + k] = p));
      if (mine !== token || !photos[i]) return;
      // Ask for neighbours first and the current photo last: the newest request is
      // served first, and the photo on screen matters most.
      for (const j of neighbors(i, total, PRELOAD).reverse()) {
        const p = photos[j];
        if (p) new Image().src = urlFor(p); // the browser caches it for the next press
      }
      load(urlFor(photos[i]), mine);
    });
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
  {#if shownSrc}
    <!-- Stays on the previous photo until the next one is ready: no blank flash. -->
    <img alt="" src={shownSrc} />
  {/if}
  {#if failed}
    <p class="msg">This photo couldn't be opened. It may be damaged.</p>
  {/if}
  <div class="count">{index + 1} / {total}</div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: #111; overflow: hidden; color: #eee; }
  /* Fill the window and let object-fit shrink the photo to fit; no dependence on the
     photo's own size (a grid/flex cell would grow to it and crop). */
  img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  .msg { position: absolute; inset: 0; display: grid; place-items: center; margin: 0; background: #111; }
  .close { position: absolute; z-index: 1; top: 12px; right: 16px; background: none; border: 0; color: #eee; font-size: 22px; cursor: pointer; }
  .count { position: absolute; z-index: 1; text-shadow: 0 0 4px #000; bottom: 10px; font-size: 13px; opacity: .7; }
</style>
