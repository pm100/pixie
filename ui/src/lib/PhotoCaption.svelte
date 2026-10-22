<script lang="ts">
  import { placeFor, type Photo, type Place } from './api';
  import { formatDateTime, formatGps } from './format';
  import { createLatest } from './latest';
  import { placeLines } from './placeLines';

  let { photo, top = 12 }: { photo: Photo | null; top?: number } = $props();

  // Date and place of the photo on screen. The coordinates show at once; the place name replaces them
  // when the lookup answers (it needs the network and may never). A slow answer for a photo that is no
  // longer shown is dropped.
  const lat = $derived(photo?.gps_lat ?? null);
  const lon = $derived(photo?.gps_lon ?? null);
  const coords = $derived(formatGps(lat, lon));
  let place = $state<Place | null>(null);
  const placeToken = createLatest();
  const lines = $derived(placeLines(place));
  $effect(() => {
    place = null;
    const t = placeToken.next();
    if (lat === null || lon === null) return;
    placeFor(lat, lon)
      .then((p) => { if (placeToken.isCurrent(t)) place = p; })
      .catch(() => {});
  });
</script>

{#if photo}
  <div class="cap" style={`top: ${top}px`}>
    <div class="when">{formatDateTime(photo.taken_at)}</div>
    {#if lines.primary}
      <div class="where"><strong>{lines.primary}</strong>{#if lines.secondary}<span> · {lines.secondary}</span>{/if}</div>
    {:else if coords}
      <div class="where">{coords}</div>
    {/if}
  </div>
{/if}

<style>
  .cap { position: absolute; z-index: 1; left: 16px; max-width: calc(100% - 90px); padding: 8px 12px; border-radius: 6px; background: rgba(0, 0, 0, .45); color: #eee; text-shadow: 0 0 4px #000; pointer-events: none; line-height: 1.35; }
  .cap .when { font-size: 15px; font-weight: 600; }
  .cap .where { font-size: 13px; opacity: .9; }
</style>
