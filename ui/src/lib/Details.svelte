<script lang="ts">
  import { placeFor, type Photo, type Place } from './api';
  import { basename, dirname, formatBytes, formatDateTime, formatGps } from './format';
  import { createLatest } from './latest';
  import { placeLines } from './placeLines';
  import LocationMap from './LocationMap.svelte';

  let { photo, count }: { photo: Photo | null; count: number } = $props();
  const gps = $derived(photo ? formatGps(photo.gps_lat, photo.gps_lon) : null);
  const lat = $derived(photo?.gps_lat ?? null);
  const lon = $derived(photo?.gps_lon ?? null);

  let place = $state<Place | null>(null);
  const latest = createLatest();
  const lines = $derived(placeLines(place));

  // Look the place up when the selected photo's location changes. A slow answer for a
  // photo that is no longer selected is dropped. Offline or failed: show nothing.
  $effect(() => {
    place = null;
    const token = latest.next();
    if (lat == null || lon == null) return;
    placeFor(lat, lon)
      .then((p) => {
        if (latest.isCurrent(token)) place = p;
      })
      .catch(() => {});
  });
</script>

<div class="details">
  {#if count > 1}
    <p class="hint"><strong>{count}</strong> photos selected</p>
  {:else if photo}
    <h3 title={photo.path}>{basename(photo.path)}</h3>
    <dl>
      <dt>Taken</dt><dd>{formatDateTime(photo.taken_at)}</dd>
      {#if photo.width && photo.height}<dt>Size</dt><dd>{photo.width} × {photo.height}</dd>{/if}
      <dt>File</dt><dd>{formatBytes(photo.size)}</dd>
      <dt>Folder</dt><dd class="path" title={dirname(photo.path)}>{dirname(photo.path)}</dd>
      {#if photo.camera}<dt>Camera</dt><dd>{photo.camera}</dd>{/if}
      {#if gps}<dt>Location</dt><dd>{gps}</dd>{/if}
    </dl>
    {#if lat != null && lon != null}
      <LocationMap {lat} {lon} />
      {#if lines.primary}<div class="place"><strong>{lines.primary}</strong>{#if lines.secondary}<br />{lines.secondary}{/if}</div>{/if}
    {/if}
  {:else}
    <p class="hint">Select a photo to see its details.</p>
  {/if}
  <button disabled={count === 0} title="Coming soon">Email…</button>
</div>

<style>
  .details { padding: 12px 14px; font-size: 14px; }
  h3 { margin: 0 0 10px; word-break: break-all; font-size: 15px; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; margin: 0 0 16px; }
  dt { color: #777; }
  dd { margin: 0; min-width: 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .hint { color: #777; }
  .place { margin: 0 0 16px; }
</style>
