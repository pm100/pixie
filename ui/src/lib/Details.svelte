<script lang="ts">
  import { facesFor, placeFor, tagsFor, type Photo, type PhotoTag, type Place } from './api';
  import { personLabel, type FaceInfo } from './people';
  import { faceUrl } from './url';
  import { tagLabel } from './tagNav';
  import { basename, dirname, formatBytes, formatDateTime, formatDuration, formatGps } from './format';
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

  // What the photo shows. Re-read when the selection changes; a slow answer for an old selection is dropped.
  let tags = $state<PhotoTag[]>([]);
  const tagsLatest = createLatest();
  const photoId = $derived(photo?.id ?? null);
  $effect(() => {
    tags = [];
    const token = tagsLatest.next();
    if (photoId === null) return;
    tagsFor(photoId).then((t) => { if (tagsLatest.isCurrent(token)) tags = t; }).catch(() => {});
  });

  // The people in the photo. Re-read when the selection changes; a slow answer for an old selection is dropped.
  let faces = $state<FaceInfo[]>([]);
  const facesLatest = createLatest();
  $effect(() => {
    faces = [];
    const token = facesLatest.next();
    if (photoId === null) return;
    facesFor(photoId).then((f) => { if (facesLatest.isCurrent(token)) faces = f; }).catch(() => {});
  });
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
    <p class="hint"><strong>{count.toLocaleString()}</strong> photos selected</p>
  {:else if photo}
    <h3 title={photo.path}>{basename(photo.path)}</h3>
    <dl>
      <dt>Taken</dt><dd>{formatDateTime(photo.taken_at)}</dd>
      {#if photo.width && photo.height}<dt>Size</dt><dd>{photo.width} × {photo.height}</dd>{/if}
      {#if photo.kind === 'video'}<dt>Duration</dt><dd>{formatDuration(photo.duration_ms)}</dd>{/if}
      <dt>File</dt><dd>{formatBytes(photo.size)}</dd>
      <dt>Folder</dt><dd class="path" title={dirname(photo.path)}><bdi dir="ltr">{dirname(photo.path)}</bdi></dd>
      {#if photo.camera}<dt>Camera</dt><dd>{photo.camera}</dd>{/if}
      {#if gps}<dt>Location</dt><dd>{gps}</dd>{/if}
      {#if faces.length > 0}<dt>People</dt><dd class="chips">{#each faces as f (f.id)}<span class="chip face"><img alt="" src={faceUrl(f.id)} />{personLabel({ name: f.person_name })}</span>{/each}</dd>{/if}
      {#if tags.length > 0}<dt>Shows</dt><dd class="chips">{#each tags as t (t.tag)}<span class="chip" title="Confidence {Math.round(t.score * 100)}%">{tagLabel(t.tag)}</span>{/each}</dd>{/if}
    </dl>
    {#if lat != null && lon != null}
      <LocationMap {lat} {lon} />
      {#if lines.primary}<div class="place"><strong>{lines.primary}</strong>{#if lines.secondary}<br />{lines.secondary}{/if}</div>{/if}
    {/if}
  {:else}
    <p class="hint">Select a photo to see its details.</p>
  {/if}
</div>

<style>
  .details { padding: 12px 14px; font-size: 14px; }
  h3 { margin: 0 0 10px; word-break: break-all; font-size: 15px; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; margin: 0 0 16px; }
  dt { color: #777; }
  dd { margin: 0; min-width: 0; }
  /* Cut off at the left (rtl) so the end of a long path shows; the <bdi> keeps the path itself left-to-right, or the colon of a
     drive letter (P:) would jump to the wrong side. */
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { padding: 1px 8px; border-radius: 10px; background: #eef2fb; color: #2a3f7a; font-size: 12px; }
  .face { display: inline-flex; align-items: center; gap: 4px; padding-left: 2px; }
  .face img { width: 18px; height: 18px; border-radius: 50%; object-fit: cover; }
  .hint { color: #777; }
  .place { margin: 0 0 16px; }
</style>
