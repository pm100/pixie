<script lang="ts">
  import { facesFor, placeFor, tagsFor, tagUsage, untagPhotos, type Photo, type PhotoTag, type Place, type TagUse } from './api';
  import { personLabel, type FaceInfo } from './people';
  import { faceUrl } from './url';
  import { tagLabel } from './tagNav';
  import { basename, dirname, formatBytes, formatDateTime, formatDuration, formatGps } from './format';
  import { createLatest } from './latest';
  import { placeLines } from './placeLines';
  import EmailButton from './EmailButton.svelte';
  import TagAs from './TagAs.svelte';
  import { plural } from './format';
  import { tagsOnAll } from './userTags';
  import LocationMap from './LocationMap.svelte';

  let { photo, count, emailLimitMb, picks, onpicksclear, ontagschange, ontools }: {
    photo: Photo | null; count: number;
    /** The picked photos (Quick pick), and taking them all off. */
    picks: ReadonlySet<number>; onpicksclear: () => void;
    /** Tags were added or removed here (the tag named): the tag list in the tree needs reading again. */
    ontagschange: (tag: string) => void;
    /** Go to the Tools tab. */
    ontools: () => void;
    /** The email size limit (Settings, Email). */
    emailLimitMb: number;
  } = $props();
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

  // Your own tags, for the photo shown or for every picked photo. Read again when the photos or the tags change.
  const pickIds = $derived([...picks]);
  let showGroup = $state(false); // with both a photo and picks, which one the panel is about
  const group = $derived(picks.size > 0 && (photo === null || count > 1 || showGroup));
  const subjectIds = $derived(group ? pickIds : photoId === null ? [] : [photoId]);
  let usage = $state<TagUse[]>([]);
  let tagsRev = $state(0);
  const usageLatest = createLatest();
  $effect(() => {
    tagsRev;
    const ids = subjectIds;
    const token = usageLatest.next();
    if (ids.length === 0) { usage = []; return; }
    tagUsage(ids).then((u) => { if (usageLatest.isCurrent(token)) usage = u; }).catch(() => {});
  });
  const onAll = $derived(tagsOnAll(usage, subjectIds.length));
  const tagsChanged = (tag: string) => { tagsRev++; ontagschange(tag); };
  async function removeTag(name: string) {
    try { await untagPhotos(subjectIds, name); } catch { /* shown again as it was */ }
    tagsChanged(name);
  }

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
  {#if photo && count <= 1 && picks.size > 0}
    <div class="seg" role="group" aria-label="Show details for">
      <button aria-pressed={!group} onclick={() => (showGroup = false)}>This photo</button>
      <button aria-pressed={group} onclick={() => (showGroup = true)}>Picked ({picks.size.toLocaleString()})</button>
    </div>
  {/if}
  {#if group}
    <div class="gtitle"><b>{plural(picks.size, 'picked photo')}</b><button class="link" onclick={onpicksclear}>Clear</button></div>
    <p class="hint">Changes here apply to all of them.</p>
    <h4>Tags on all of them</h4>
    <div class="chips">
      {#each onAll as t (t)}<span class="chip utag"># {t}<button class="x" aria-label="Take {t} off all of them" onclick={() => removeTag(t)}>×</button></span>{/each}
      {#if onAll.length === 0}<span class="hint">No tag is on all of them.</span>{/if}
      <TagAs ids={pickIds} onchange={tagsChanged} />
    </div>
  {:else if count > 1}
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
      <dt>Tags</dt><dd class="chips">
        {#each onAll as t (t)}<span class="chip utag"># {t}<button class="x" aria-label="Take {t} off this photo" onclick={() => removeTag(t)}>×</button></span>{/each}
        <TagAs ids={subjectIds} onchange={tagsChanged} />
      </dd>
      {#if tags.length > 0}<dt>Shows</dt><dd class="chips">{#each tags as t (t.tag)}<span class="chip" title="Confidence {Math.round(t.score * 100)}%">{tagLabel(t.tag)}</span>{/each}</dd>{/if}
    </dl>
    <p class="actions"><EmailButton ids={photo ? [photo.id] : []} limitMb={emailLimitMb} /></p>
    {#if lat != null && lon != null}
      <LocationMap {lat} {lon} />
      {#if lines.primary}<div class="place"><strong>{lines.primary}</strong>{#if lines.secondary}<br />{lines.secondary}{/if}</div>{/if}
    {/if}
  {:else}
    <p class="hint">Select a photo to see its details.</p>
  {/if}
  {#if picks.size > 0}
    <p class="foot">Use the <button class="link" onclick={ontools}>Tools tab</button> to work with these pics.</p>
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
  .foot { margin: 14px 0 0; padding-top: 10px; border-top: 1px solid #e4e4e8; color: #777; font-size: 13px; }
  h4 { margin: 12px 0 4px; font-size: 12px; font-weight: 600; color: #777; text-transform: uppercase; letter-spacing: .04em; }
  .gtitle { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .link { border: 0; background: none; padding: 0; color: #2b5fd9; cursor: pointer; font: inherit; font-size: 13px; }
  .seg { display: flex; border: 1px solid #d6d9e2; border-radius: 6px; overflow: hidden; margin-bottom: 10px; }
  .seg button { flex: 1; border: 0; background: #fff; padding: 5px; font: inherit; cursor: pointer; }
  .seg button[aria-pressed="true"] { background: #2f6fed; color: #fff; font-weight: 600; }
  .chip.utag { background: #e3ecff; border-radius: 4px; }
  .x { border: 0; background: none; padding: 0 0 0 5px; color: inherit; opacity: .7; cursor: pointer; line-height: 1; }
  .x:hover { opacity: 1; }
  .actions { margin: 0 0 12px; }
  .place { margin: 0 0 16px; }
</style>
