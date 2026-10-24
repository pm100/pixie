<script lang="ts">
  import type { Snippet } from 'svelte';
  import { getPhoto, type Photo } from './api';
  import { createLatest } from './latest';
  import PhotoCaption from './PhotoCaption.svelte';
  import type { FaceTile } from './people';
  import { thumbUrl } from './url';

  let { tiles, index, title, onclose, onstep, actions }: {
    tiles: FaceTile[];
    /** The face on screen, by position in `tiles`. */
    index: number;
    title?: string;
    onclose: () => void;
    onstep: (by: number) => void;
    /** What can be done with the face on screen. */
    actions?: Snippet<[FaceTile]>;
  } = $props();

  const viewed = $derived(tiles[index] ?? null);
  /** The photo behind the face on screen, for the same date and place caption the photo viewer shows. */
  let viewedPhoto = $state<Photo | null>(null);
  const photoToken = createLatest();
  $effect(() => {
    const id = viewed?.photo_id ?? null;
    viewedPhoto = null;
    const t = photoToken.next();
    if (id === null) return;
    getPhoto(id).then((p) => { if (photoToken.isCurrent(t)) viewedPhoto = p; }).catch(() => {});
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.target instanceof Element && e.target.closest('input, select, textarea')) return; // typing a name
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowLeft') onstep(-1);
    else if (e.key === 'ArrowRight') onstep(1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

{#if viewed}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="lightbox" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }} role="dialog" tabindex="-1" aria-modal="true" aria-label="The whole photo">
    <div class="lb-bar">
      {#if title}<span>{title}</span>{/if}
      <span class="sp"></span>
      {#if actions}<div class="lb-actions">{@render actions(viewed)}</div>{/if}
      <button class="lb-close" onclick={onclose} aria-label="Close">✕</button>
    </div>
    <PhotoCaption photo={viewedPhoto} top={52} />
    {#if index > 0}<button class="step prev" onclick={() => onstep(-1)} aria-label="Previous (left arrow)" title="Previous (←)"><svg viewBox="0 0 24 24" width="30" height="30" aria-hidden="true"><path d="M15 4 7 12l8 8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>{/if}
    {#if index < tiles.length - 1}<button class="step next" onclick={() => onstep(1)} aria-label="Next (right arrow)" title="Next (→)"><svg viewBox="0 0 24 24" width="30" height="30" aria-hidden="true"><path d="M9 4l8 8-8 8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>{/if}
    <div class="count"><b>{(index + 1).toLocaleString()} / {tiles.length.toLocaleString()}</b><span><button class="kbd" onclick={() => onstep(-1)} disabled={index <= 0} aria-label="Previous" title="Previous"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13 8H3M7.5 3.5 3 8l4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button><button class="kbd" onclick={() => onstep(1)} disabled={index >= tiles.length - 1} aria-label="Next" title="Next"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M3 8h10M8.5 3.5 13 8l-4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button> browse</span><span><button class="kbd" onclick={onclose} title="Close">Esc</button> close</span></div>
    <div class="lb-stage">
      <div class="lb-frame">
        {#key viewed.photo_id}
          <img alt="Where this face was found" src={thumbUrl('view', viewed.photo_id, viewed.mtime, viewed.size)} />
        {/key}
        <div class="lb-box" style={`left: ${viewed.x * 100}%; top: ${viewed.y * 100}%; width: ${viewed.w * 100}%; height: ${viewed.h * 100}%`}></div>
      </div>
    </div>
  </div>
{/if}

<style>
  button { font: inherit; font-size: 13px; }
  .lightbox { position: fixed; inset: 0; z-index: 50; display: flex; flex-direction: column; background: rgba(12, 14, 20, .94); color: #eee; }
  .lb-bar { display: flex; align-items: center; gap: 10px; padding: 8px 14px; background: rgba(0, 0, 0, .35); font-size: 13px; }
  .lb-bar .sp { flex: 1; }
  .lb-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .lb-close { background: rgba(0, 0, 0, .55); border: 0; border-radius: 6px; padding: 2px 10px; color: #fff; font-size: 22px; cursor: pointer; }
  .step { position: absolute; z-index: 1; top: 50%; transform: translateY(-50%); width: 48px; height: 72px; border: 0; border-radius: 8px; background: rgba(0, 0, 0, .35); color: #fff; display: grid; place-items: center; padding: 0; cursor: pointer; opacity: .55; }
  .step:hover, .step:focus-visible { opacity: 1; background: rgba(0, 0, 0, .6); }
  .step.prev { left: 12px; } .step.next { right: 12px; }
  .count { position: absolute; z-index: 1; left: 50%; transform: translateX(-50%); bottom: 10px; display: flex; gap: 16px; align-items: center; padding: 6px 16px; border-radius: 16px; background: rgba(0, 0, 0, .75); color: #fff; font-size: 14px; white-space: nowrap; }
  .count b { font-weight: 600; }
  .kbd { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-width: 26px; height: 22px; padding: 0 6px; margin-right: 3px; border: 1px solid #aaa; border-radius: 4px; background: #333; color: #fff; font: inherit; font-size: 13px; line-height: 1; cursor: pointer; }
  .kbd:hover:not(:disabled) { background: #555; border-color: #fff; }
  .kbd:disabled { opacity: .35; cursor: default; }
  .lb-stage { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; padding: 12px; }
  /* The frame shrink-wraps the picture, so the box's percentages are percentages of the photo itself. */
  .lb-frame { position: relative; display: inline-block; line-height: 0; }
  .lb-frame img { max-width: calc(100vw - 24px); max-height: calc(100vh - 90px); background: #222; }
  .lb-box { position: absolute; box-sizing: border-box; border: 3px solid #ffd54a; border-radius: 4px; box-shadow: 0 0 0 1px rgba(0, 0, 0, .6), 0 0 0 9999px rgba(0, 0, 0, .25); pointer-events: none; }
</style>
