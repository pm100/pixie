<script lang="ts">
  import { listPhotos, openInDefaultPlayer, type Photo, type View } from './api';
  import { filmRange } from './filmstrip';
  import { neighbors } from './neighbors';
  import PhotoCaption from './PhotoCaption.svelte';
  import { posterUrl, thumbUrl, videoUrl } from './url';

  let { index, total, view, solo = null, page, onclose, onnav, onpopout, filmstrip = true }: {
    index: number; total: number; view: View; solo?: Photo | null; onclose: () => void; onnav: (i: number) => void;
    /** Where the photos come from, when not the Browse listing described by `view` (the Selected tab pages through its own). */
    page?: (offset: number, limit: number) => Promise<Photo[]>;
    onpopout?: () => void;
    /** The strip of neighbours along the bottom; the pop-out window turns it off. */
    filmstrip?: boolean;
  } = $props();

  const PRELOAD = 3; // photos kept ready on each side of the current one
  const FILM = 6; // items shown on each side of the current one in the filmstrip
  const VIDEO_TIMEOUT_MS = 8000;

  let shownSrc = $state<string | null>(null); // last photo that finished loading
  let failed = $state(false);
  let current = $state<Photo | null>(null);
  let videoFailed = $state(false);
  let videoReady = false;
  // Primitives, so the effects below re-run when the item changes, not when a refresh swaps in an equal object.
  const curId = $derived(current?.id ?? null);
  const curKind = $derived(current?.kind ?? null);
  let player: HTMLVideoElement | undefined = $state();
  const photos: Record<number, Photo> = {};
  let stripTick = $state(0); // bumped when a page of neighbours arrives, so the filmstrip redraws

  const strip = $derived.by(() => {
    stripTick;
    if (solo) return [];
    const { lo, hi } = filmRange(index, total, FILM);
    const out: { i: number; p: Photo | undefined }[] = [];
    for (let k = lo; k < hi; k++) out.push({ i: k, p: photos[k] });
    return out;
  });
  let token = 0; // latest navigation wins; older loads are ignored

  const urlFor = (p: Photo) => thumbUrl('view', p.id, p.mtime, p.size);

  function load(src: string, mine: number) {
    const img = new Image();
    img.onload = () => { if (mine === token) { shownSrc = src; failed = false; } };
    img.onerror = () => { if (mine === token) failed = true; };
    img.src = src;
  }

  $effect(() => {
    if (solo) {
      // One photo opened from the map: no neighbours, no listing.
      const mine = ++token;
      if (current?.id !== solo.id) {
        videoFailed = false;
        videoReady = false;
        current = solo;
      }
      if (solo.kind === 'photo') load(urlFor(solo), mine);
      else { shownSrc = null; failed = false; }
      return;
    }
    const i = index;
    const mine = ++token;
    // One call for the whole neighbourhood instead of one per arrow press.
    const { lo, hi } = filmRange(i, total, FILM);
    (page ? page(lo, hi - lo) : listPhotos(lo, hi - lo, view.order, view.folderId, view.kind, view.bounds, view.maxId, view.since, view.until, view.search)).then((rows) => {
      rows.forEach((p, k) => (photos[lo + k] = p));
      stripTick++;
      if (mine !== token || !photos[i]) return;
      const here = photos[i];
      if (current?.id !== here.id) {
        // A new item. The same item seen again (a scan refreshed the list) keeps its player and state.
        videoFailed = false;
        videoReady = false;
        current = here;
      }
      // Ask for neighbours first and the current photo last: the newest request is
      // served first, and the photo on screen matters most. Videos are not preloaded.
      for (const j of neighbors(i, total, PRELOAD).reverse()) {
        const p = photos[j];
        if (p && p.kind === 'photo') new Image().src = urlFor(p); // the browser caches it for the next press
      }
      if (here.kind === 'photo') load(urlFor(here), mine);
      else { shownSrc = null; failed = false; }
    });
  });

  // A video that has not produced metadata after a while cannot be played here (for example
  // iPhone HEVC video on a PC without Microsoft's HEVC extension).
  $effect(() => {
    if (curId === null || curKind !== 'video') return;
    const timer = setTimeout(() => { if (!videoReady) videoFailed = true; }, VIDEO_TIMEOUT_MS);
    return () => clearTimeout(timer);
  });

  // Leaving a video (or closing the viewer) must stop it at once.
  $effect(() => {
    const el = player;
    return () => {
      if (!el) return;
      el.pause();
      el.removeAttribute('src'); // release the decoder and its buffer now, not at garbage collection
      el.load();
    };
  });

  /** One step forward (+1) or back (-1), from a key or a click; ignored at either end. */
  function step(delta: 1 | -1) {
    const to = index + delta;
    if (solo || to < 0 || to >= total) return;
    (document.activeElement as HTMLElement | null)?.blur();
    onnav(to);
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowRight') step(1);
    else if (e.key === 'ArrowLeft') step(-1);
    else if (e.key === ' ' && current?.kind === 'video' && player && (e.target as HTMLElement | null)?.tagName !== 'BUTTON') {
      e.preventDefault();
      if (player.paused) void player.play(); else player.pause();
    }
  }
</script>

<svelte:window onkeydown={key} />
<div class="overlay" role="dialog" aria-label={current?.kind === 'video' ? 'Video viewer' : 'Photo viewer'}>
  <button class="close" onclick={onclose} aria-label="Close">✕</button>
  {#if onpopout && !solo}<button class="popout" onclick={onpopout}>Pop out ⧉</button>{/if}
  {#if current?.kind === 'video'}
    {#key current.id}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={player} controls autoplay src={videoUrl(current.id, current.mtime, current.size)}
             onloadedmetadata={(e) => {
               videoReady = true;
               videoFailed = false;
               // Opening a video plays it. `autoplay` alone is not always honoured, so ask explicitly;
               // a refusal (autoplay policy) just leaves it paused with controls showing.
               e.currentTarget.play().catch(() => {});
             }}
             onerror={() => (videoFailed = true)}></video>
    {/key}
    {#if videoFailed}
      <div class="msg">
        <p>This video can't be played here. If it is an iPhone video, Windows may need Microsoft's HEVC Video Extensions.</p>
        <button onclick={() => current && openInDefaultPlayer(current.id)}>Open in default player</button>
      </div>
    {/if}
  {:else}
    {#if shownSrc}
      <!-- Stays on the previous photo until the next one is ready: no blank flash. -->
      <img alt="" src={shownSrc} />
    {/if}
    {#if failed}
      <p class="msg">This photo couldn't be opened. It may be damaged.</p>
    {/if}
  {/if}
  <PhotoCaption photo={current} />
  {#if filmstrip && !solo && strip.length}
    <div class="film" role="listbox" aria-label="Nearby items">
      {#each strip as s (s.i)}
        <button class="cell" class:on={s.i === index} onclick={() => onnav(s.i)} aria-label={`Item ${s.i + 1}`}>
          {#if s.p}
            <img alt="" loading="lazy"
                 src={s.p.kind === 'video' ? posterUrl(s.p.id, s.p.mtime, s.p.size) : thumbUrl('grid', s.p.id, s.p.mtime, s.p.size)}
                 onerror={(e) => ((e.currentTarget as HTMLElement).style.visibility = 'hidden')} />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
  {#if !solo}
    {#if index > 0}<button class="step prev" onclick={() => step(-1)} aria-label="Previous (left arrow)" title="Previous (←)"><svg viewBox="0 0 24 24" width="30" height="30" aria-hidden="true"><path d="M15 4 7 12l8 8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>{/if}
    {#if index < total - 1}<button class="step next" onclick={() => step(1)} aria-label="Next (right arrow)" title="Next (→)"><svg viewBox="0 0 24 24" width="30" height="30" aria-hidden="true"><path d="M9 4l8 8-8 8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>{/if}
    <div class="count"><b>{(index + 1).toLocaleString()} / {total.toLocaleString()}</b><span><button class="kbd" onclick={() => step(-1)} disabled={index <= 0} aria-label="Previous" title="Previous"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13 8H3M7.5 3.5 3 8l4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button><button class="kbd" onclick={() => step(1)} disabled={index >= total - 1} aria-label="Next" title="Next"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M3 8h10M8.5 3.5 13 8l-4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button> browse</span><span><button class="kbd" onclick={onclose} title="Close">Esc</button> close</span></div>
  {/if}
</div>

<style>
  /* Above the map: Leaflet panes and controls use z-index up to 1000. */
  .overlay { position: fixed; inset: 0; z-index: 2000; background: #111; overflow: hidden; color: #eee; }
  /* Fill the window and let object-fit shrink the photo to fit; no dependence on the
     photo's own size (a grid/flex cell would grow to it and crop). */
  .overlay > img, .overlay > video { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  video { background: #000; }
  .msg { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: 12px; margin: 0; padding: 24px; text-align: center; background: #111; }
  .msg p { margin: 0; max-width: 40ch; }
  .close { position: absolute; z-index: 1; top: 12px; right: 16px; background: rgba(0, 0, 0, .55); border: 0; border-radius: 6px; padding: 2px 10px; color: #fff; font-size: 22px; cursor: pointer; }
  .popout { position: absolute; z-index: 1; top: 12px; right: 60px; background: rgba(0, 0, 0, .55); border: 0; border-radius: 6px; padding: 7px 10px; color: #fff; font-size: 14px; cursor: pointer; }
  .film { position: absolute; z-index: 1; left: 50%; transform: translateX(-50%); bottom: 56px; display: flex; gap: 4px; padding: 6px; background: rgba(0, 0, 0, .45); border-radius: 8px; max-width: calc(100% - 32px); overflow: hidden; }
  .cell { width: 54px; height: 54px; padding: 0; border: 2px solid transparent; border-radius: 4px; background: #222; cursor: pointer; flex: none; overflow: hidden; }
  .cell.on { border-color: #ffc83d; }
  .cell img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .step { position: absolute; z-index: 1; top: 50%; transform: translateY(-50%); width: 48px; height: 72px; border: 0; border-radius: 8px; background: rgba(0, 0, 0, .35); color: #fff; display: grid; place-items: center; padding: 0; cursor: pointer; opacity: .55; }
  .step:hover, .step:focus-visible { opacity: 1; background: rgba(0, 0, 0, .6); }
  .step.prev { left: 12px; } .step.next { right: 12px; }
  /* On a dark pill at full strength so it reads over any photo. */
  .count { position: absolute; z-index: 1; left: 50%; transform: translateX(-50%); bottom: 10px; display: flex; gap: 16px; align-items: center; padding: 6px 16px; border-radius: 16px; background: rgba(0, 0, 0, .75); color: #fff; font-size: 14px; white-space: nowrap; }
  .count b { font-weight: 600; }
  .kbd { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-width: 26px; height: 22px; padding: 0 6px; margin-right: 3px; border: 1px solid #aaa; border-radius: 4px; background: #333; color: #fff; font: inherit; font-size: 13px; line-height: 1; cursor: pointer; }
  .kbd:hover:not(:disabled) { background: #555; border-color: #fff; }
  .kbd:disabled { opacity: .35; cursor: default; }
</style>
