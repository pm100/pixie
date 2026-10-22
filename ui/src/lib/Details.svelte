<script lang="ts">
  import { facesFor, moveFace, newPersonFromFace, placeFor, rejectFace, tagsFor, type Photo, type PhotoTag, type Place } from './api';
  import { personLabel, resolveFaceName, type FaceInfo, type PersonInfo } from './people';
  import { faceUrl } from './url';
  import { tagLabel } from './tagNav';
  import { basename, dirname, formatBytes, formatDateTime, formatDuration, formatGps } from './format';
  import { createLatest } from './latest';
  import { placeLines } from './placeLines';
  import LocationMap from './LocationMap.svelte';

  let { photo, count, people = [], onfaceschange }: { photo: Photo | null; count: number; people?: PersonInfo[]; onfaceschange?: () => void } = $props();
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
  function loadFaces() {
    const token = facesLatest.next();
    if (photoId === null) { faces = []; return; }
    facesFor(photoId).then((f) => { if (facesLatest.isCurrent(token)) faces = f; }).catch(() => {});
  }
  $effect(() => {
    photoId; // re-run when the selection changes
    faces = [];
    loadFaces();
  });
  /** The face being edited and what has been typed for it. */
  let editing = $state<number | null>(null);
  let typed = $state('');
  let problem = $state<string | null>(null);
  function edit(f: FaceInfo) {
    editing = editing === f.id ? null : f.id;
    typed = f.person_name ?? '';
    problem = null;
  }
  async function changed() {
    editing = null;
    loadFaces();
    onfaceschange?.();
  }
  async function assign(f: FaceInfo) {
    const what = resolveFaceName(people, typed, f.person_id);
    if (what.kind === 'none') { editing = null; return; }
    try {
      if (what.kind === 'move') await moveFace(f.id, what.personId);
      else await newPersonFromFace(f.id, what.name);
    } catch (e) { problem = String(e); return; }
    await changed();
  }
  async function notThisPerson(f: FaceInfo) {
    try { await rejectFace(f.id); } catch (e) { problem = String(e); return; }
    await changed();
  }

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
      <dt>Folder</dt><dd class="path" title={dirname(photo.path)}>{dirname(photo.path)}</dd>
      {#if photo.camera}<dt>Camera</dt><dd>{photo.camera}</dd>{/if}
      {#if gps}<dt>Location</dt><dd>{gps}</dd>{/if}
      {#if faces.length > 0}<dt>People</dt><dd class="chips">{#each faces as f (f.id)}<button class="chip face" class:on={editing === f.id} title="Who is this? Click to change" onclick={() => edit(f)}><img alt="" src={faceUrl(f.id)} />{personLabel({ name: f.person_name })}</button>{/each}</dd>{/if}
      {#each faces.filter((f) => f.id === editing) as f (f.id)}
        <dt></dt>
        <dd class="editor">
          <form onsubmit={(e) => { e.preventDefault(); assign(f); }}>
            <label>Who is this?
              <input list="people-names" bind:value={typed} placeholder="Type a name" />
            </label>
            <datalist id="people-names">{#each people as p (p.id)}{#if p.name}<option value={p.name}></option>{/if}{/each}</datalist>
            <div class="row">
              <button type="submit">Save</button>
              {#if f.person_id !== null}<button type="button" onclick={() => notThisPerson(f)} title="This face is not {personLabel({ name: f.person_name })}">Not {personLabel({ name: f.person_name })}</button>{/if}
              <button type="button" onclick={() => (editing = null)}>Cancel</button>
            </div>
            {#if problem}<div class="err">{problem}</div>{/if}
          </form>
        </dd>
      {/each}
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
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { padding: 1px 8px; border-radius: 10px; background: #eef2fb; color: #2a3f7a; font-size: 12px; }
  .editor form { display: flex; flex-direction: column; gap: 6px; }
  .editor input { width: 100%; box-sizing: border-box; font: inherit; padding: 3px 6px; }
  .editor .row { display: flex; flex-wrap: wrap; gap: 6px; }
  .err { color: #b3261e; font-size: 12px; }
  button.face { border: 0; cursor: pointer; font: inherit; font-size: 12px; }
  button.face.on { outline: 2px solid #2f6fed; }
  .face { display: inline-flex; align-items: center; gap: 4px; padding-left: 2px; }
  .face img { width: 18px; height: 18px; border-radius: 50%; object-fit: cover; }
  .hint { color: #777; }
  .place { margin: 0 0 16px; }
</style>
