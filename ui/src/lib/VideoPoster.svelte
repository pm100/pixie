<script module lang="ts">
  import type { Photo } from './api';
  import { makePoster } from './posterFrame';
  import { createPosterQueue } from './posterQueue';

  // One poster is made at a time across every instance, as in the Browse grid.
  const known: Record<number, Photo> = {};
  const queue = createPosterQueue((id) => makePoster(known[id]));
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import { posterUrl } from './url';

  let { photo }: { photo: Photo } = $props();

  // The same picture Browse shows for a video: the saved poster frame, made by the webview if there is none yet.
  // Only a poster that is on screen (or just off it) is asked for or made, and one that scrolls away is dropped from
  // the queue, so a long list does not queue hundreds of videos in front of the one being looked at.
  let box = $state<HTMLElement>();
  let visible = $state(false);
  let loaded = $state(false);
  let nonce = $state(0);
  let tries = 0;
  let none = $state(false); // the player cannot decode it

  $effect(() => {
    const el = box;
    if (!el) return;
    const watch = new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { rootMargin: '300px' });
    watch.observe(el);
    return () => { watch.disconnect(); queue.cancel(photo.id); };
  });
  $effect(() => {
    if (!visible) untrack(() => { loaded = false; queue.cancel(photo.id); });
  });

  function missing() {
    if (!visible) return;
    known[photo.id] = photo;
    if (++tries > 2) { none = true; return; } // saved but still not served: stop
    queue.request(photo.id).then((ok) => {
      if (ok) nonce++;
      else if (visible) none = true;
      else tries = Math.max(0, tries - 1); // dropped because it scrolled away: it may be asked for again
    });
  }
</script>

<div class="box" bind:this={box}>
  {#if none}
    <span class="ph" aria-hidden="true"><svg viewBox="0 0 24 24" fill="currentColor"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h9A1.5 1.5 0 0 1 15 6.5v2.6l5.2-3a.5.5 0 0 1 .8.4v11a.5.5 0 0 1-.8.4l-5.2-3v2.6a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 3 17.5z"/></svg></span>
  {:else if visible}
    <img alt="" class:loaded src={posterUrl(photo.id, photo.mtime, photo.size, undefined, nonce)} onload={() => (loaded = true)} onerror={missing} />
  {/if}
</div>

<style>
  .box { position: relative; width: 100%; height: 100%; border-radius: 8px; background: #dde2ee; overflow: hidden; }
  img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; opacity: 0; }
  img.loaded { opacity: 1; }
  .ph { position: absolute; inset: 0; display: grid; place-items: center; color: #8a93a8; }
  .ph svg { width: 40%; height: 40%; }
</style>
