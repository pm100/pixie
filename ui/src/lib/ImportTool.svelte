<script lang="ts">
  /** Import/Export > Import: read photos from a camera card, a USB stick or any folder, look at them in Browse, and keep the ones you want in one
   *  of your folders. A source is only read: nothing is copied until Keep, and nothing on the card is ever changed. */
  import { open } from '@tauri-apps/plugin-dialog';
  import { importClose, importDiscard, importKeep, importOpen, importPickedCount, importStop, importZipExtract, importZipLook, type FolderInfo, type KeepLayout, type KeepWhich, type OpenSource, type Removable, type ZipLook } from './api';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { folderName, plural } from './format';
  import { LAYOUTS, defaultDestination, extractLine, keepLine, keepPercent, keepWords, spaceLine, summaryLine, zipLine, zipProblems } from './importer';
  import { imports, refreshAvailable, refreshSources } from './importStore.svelte';
  import { loadPref, savePref } from './prefs';

  let { folders, master = null, picks, onbrowse = () => {} }: {
    /** The library folders a photo can be kept into. */
    folders: FolderInfo[];
    /** The sync folder, kept into by default when it is one of the folders. */
    master?: string | null;
    /** The photos picked, so the count of those on a source follows. */
    picks: ReadonlySet<number>;
    /** Look at this source in Browse. */
    onbrowse?: (source: number) => void;
  } = $props();

  let dest = $state<number | null>(null);
  let layout = $state<KeepLayout>(loadPref<KeepLayout>('import.layout', 'year-date'));
  let error = $state('');
  let pickedIn = $state<Record<number, number>>({});
  /** A zip that was chosen and looked at, waiting for the person to extract it (or not). */
  let zip = $state<{ path: string; look: ZipLook } | null>(null);
  /** An Inbox source the person is about to discard (the confirmation is showing). */
  let discarding = $state<OpenSource | null>(null);

  // Where to keep: the last folder used, else the sync folder, else the first.
  $effect(() => {
    if (dest !== null && folders.some((f) => f.id === dest)) return;
    dest = defaultDestination(folders, master, loadPref<number | null>('import.dest', null));
  });
  // The drives that could be opened, looked at again every two seconds while this is shown (a card may be put in or taken out).
  $effect(() => {
    void refreshAvailable();
    const t = setInterval(() => void refreshAvailable(), 2000);
    return () => clearInterval(t);
  });
  // How many photos of each source are picked, again whenever the picks or the sources change.
  $effect(() => {
    void picks;
    const ids = imports.sources.map((s) => s.id);
    Promise.all(ids.map(async (id) => [id, await importPickedCount(id).catch(() => 0)] as const)).then((rows) => { pickedIn = Object.fromEntries(rows); });
  });

  const isOpen = (path: string) => imports.sources.some((s) => s.path === path);
  const busy = $derived(imports.keeping !== null);

  async function openPath(path: string, label?: string) {
    error = '';
    try {
      await importOpen(path, label);
      await refreshSources();
    } catch (e) {
      error = String(e);
    }
  }
  const openDrive = (d: Removable) => openPath(d.path, d.label);
  async function openFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string') await openPath(dir);
  }
  async function chooseZip() {
    const file = await open({ multiple: false, directory: false, filters: [{ name: 'Zip files', extensions: ['zip'] }] });
    if (typeof file !== 'string') return;
    error = '';
    zip = null;
    try {
      zip = { path: file, look: await importZipLook(file) };
    } catch (e) {
      error = String(e);
    }
  }
  async function extractZip() {
    if (zip === null) return;
    error = '';
    imports.extractDone = null;
    try {
      await importZipExtract(zip.path);
      zip = null;
      await refreshSources();
    } catch (e) {
      error = String(e);
    }
  }
  async function discard(s: OpenSource) {
    discarding = null;
    error = '';
    try {
      await importDiscard(s.id);
    } catch (e) {
      error = String(e);
    }
    await refreshSources();
  }
  async function keep(s: OpenSource, which: KeepWhich) {
    if (dest === null) return;
    error = '';
    imports.done = null;
    imports.progress = null;
    imports.keeping = s.id;
    savePref('import.dest', dest);
    savePref('import.layout', layout);
    try {
      await importKeep(s.id, which, dest, layout);
    } catch (e) {
      imports.keeping = null;
      error = String(e);
    }
  }
  async function done(s: OpenSource) {
    error = '';
    try {
      await importClose(s.id);
    } catch (e) {
      error = String(e);
    }
    await refreshSources();
  }
  const stop = () => void importStop();
</script>

<div class="import">
  <h2>Import</h2>
  <p class="hint">Bring photos in from a camera card, a USB stick or any folder.</p>
  <ul class="points">
    <li>Look at what is there first, in Browse. Nothing is copied yet.</li>
    <li>Keep the ones you want, filed by year and date, in one of your folders.</li>
    <li>The card or folder is only read: your originals are never changed or deleted.</li>
  </ul>
  {#if error}<p class="err" role="alert">{error}</p>{:else if imports.error}<p class="err" role="alert">{imports.error}</p>{/if}

  <h3>From</h3>
  {#if imports.available.length === 0}<p class="hint">No card or USB stick found. Put one in, or choose a folder.</p>{/if}
  <ul class="drives">
    {#each imports.available as d (d.path)}
      <li>
        <span class="name">{d.label}</span>{#if d.card}<small>camera card</small>{/if}
        <button disabled={isOpen(d.path)} onclick={() => openDrive(d)}
                title={isOpen(d.path) ? 'This card is already open below' : 'Read the list of photos and videos on this drive. Nothing is copied or changed'}>{isOpen(d.path) ? 'Open' : 'Look'}</button>
      </li>
    {/each}
  </ul>
  <div class="row">
    <button onclick={openFolder} title="Choose any folder to look at, such as one on a stick, a network share or a cloud folder">A folder…</button>
    <button onclick={chooseZip} title="Choose a zip file of photos and videos to look at. Nothing is extracted until you say so">A zip file…</button>
  </div>

  {#if zip}
    <section class="source" aria-label={zip.look.name}>
      <h3>{zip.look.name}</h3>
      <p>{zipLine(zip.look)}</p>
      {#each zipProblems(zip.look) as p (p)}<p class="hint">{p}</p>{/each}
      {#if spaceLine(zip.look)}<p class:err={!zip.look.fits}>{spaceLine(zip.look)}</p>{/if}
      <div class="row">
        <button class="primary" disabled={!zip.look.fits || zip.look.photos + zip.look.videos === 0} onclick={extractZip}
                title={zip.look.photos + zip.look.videos === 0 ? 'There is nothing to extract' : !zip.look.fits ? 'There is not enough free space for this zip' : 'Extract the photos and videos into the Inbox, where you can look at them and keep the ones you want. The zip file is not changed'}>Extract into the Inbox</button>
        <button onclick={() => (zip = null)} title="Do not extract this zip">Cancel</button>
      </div>
    </section>
  {/if}

  {#each imports.sources as s (s.id)}
    {@const pickedHere = pickedIn[s.id] ?? 0}
    <section class="source" aria-label={s.label}>
      <h3>{s.label}{#if s.inbox} <small class="tag" title="Extracted from a zip file into the Inbox. Keeping moves the photos out of the Inbox">from a zip</small>{/if}</h3>
      {#if !s.inbox}<p class="path" title={s.path}>{s.path}</p>{/if}
      {#if s.extracting}
        <div class="progress" role="status">
          <progress value={imports.extract && imports.extract.bytes_total > 0 ? Math.round((imports.extract.bytes_done / imports.extract.bytes_total) * 100) : 0} max="100" aria-label="Extracting the zip"></progress>
          <span>{extractLine(imports.extract?.done ?? 0, imports.extract?.total ?? 0)}</span>
          <button onclick={stop} title="Stop extracting. What was extracted so far stays in the Inbox">Stop</button>
        </div>
      {:else if s.scanning}
        <p role="status">Reading… {imports.scan && imports.scan.source === s.id ? plural(imports.scan.seen, 'photo or video', 'photos or videos') + ' found' : ''}</p>
        <button onclick={stop} title="Stop reading this card. What was found so far is kept">Stop</button>
      {:else}
        <p>{summaryLine(s.summary)}</p>
        <div class="row">
          <button onclick={() => onbrowse(s.id)} title="Show this card's photos in Browse, where you can look at them and pick the ones you want">Open in Browse</button>
        </div>
        {#if s.summary.photos + s.summary.videos > 0}
          <label class="opt">
            Keep into:
            <select bind:value={dest} disabled={busy || folders.length === 0} aria-label="Folder to keep into" title="The folder the photos you keep are copied into">
              {#each folders as f (f.id)}<option value={f.id}>{folderName(f.path)} ({f.path})</option>{/each}
            </select>
          </label>
          <label class="opt">
            Filed by:
            <select bind:value={layout} disabled={busy} aria-label="How to file the photos" title="How the photos you keep are filed inside that folder">
              {#each LAYOUTS as l (l.id)}<option value={l.id} title={l.title}>{l.label}</option>{/each}
            </select>
          </label>
          {#if folders.length === 0}<p class="hint">Add a folder in Settings, Photo folders, first.</p>{/if}
          <div class="row">
            <button class="primary" disabled={busy || dest === null || s.summary.new === 0} onclick={() => keep(s, 'new')}
                    title={s.summary.new === 0 ? 'Nothing here is new' : s.inbox ? 'Move everything that is new out of the Inbox into the folder, checking each copy first' : 'Copy everything that is new into the folder, checking each copy. The card is only read'}>{keepWords('new', s.summary.new)}</button>
            <button disabled={busy || dest === null || pickedHere === 0} onclick={() => keep(s, 'picked')}
                    title={pickedHere === 0 ? 'Pick photos in Browse first (the star in a photo’s corner)' : s.inbox ? 'Move the photos you picked out of the Inbox into the folder, checking each copy first' : 'Copy the photos you picked on this card into the folder, checking each copy'}>{keepWords('picked', pickedHere)}</button>
          </div>
        {/if}
      {/if}

      {#if imports.keeping === s.id}
        <div class="progress" role="status">
          <progress value={keepPercent(imports.progress)} max="100" aria-label="Keeping photos"></progress>
          <span>{imports.progress ? `${imports.progress.done.toLocaleString()} of ${imports.progress.total.toLocaleString()}` : 'Starting…'}</span>
          <button onclick={stop} title="Stop after the file being copied. What is already kept stays, and the rest is still there to keep later">Stop</button>
        </div>
      {:else if imports.done && imports.done.source === s.id}
        {#if imports.done.error}
          <p class="err" role="alert">{imports.done.error}</p>
        {:else if imports.done.report}
          <p class="result" role="status">{keepLine(imports.done.report)}</p>
          {#each imports.done.report.problems as p (p)}<p class="err">{p}</p>{/each}
        {/if}
      {/if}

      <div class="row">
        {#if s.inbox}
          <button class="danger" disabled={imports.keeping === s.id || s.extracting} onclick={() => (discarding = s)}
                  title="Delete what is left of this zip from the Inbox. The zip file itself is not touched, and the photos you kept stay in your folder">Discard what is left</button>
        {:else}
          <button disabled={imports.keeping === s.id} onclick={() => done(s)} title="Close this card in Pixie. Nothing on the card is changed, and the photos you kept stay in your folder">Done with this card</button>
        {/if}
      </div>
    </section>
  {/each}
</div>

{#if discarding}
  <ConfirmDialog title="Discard what is left?" confirmLabel="Discard" danger
                 message={`${plural(discarding.summary.photos + discarding.summary.videos, 'photo or video', 'photos or videos')} extracted from ${discarding.label} will be deleted from the Inbox. The zip file itself is not changed, and photos you already kept stay where they are.`}
                 onconfirm={() => discarding && void discard(discarding)} oncancel={() => (discarding = null)} />
{/if}

<style>
  .import { padding: var(--page-pad); font-size: 14px; }
  h2 { margin: 0 0 8px; font-size: 18px; }
  h3 { margin: 16px 0 6px; font-size: 14px; }
  p { margin: 0 0 8px; }
  .hint { color: #666; max-width: 60ch; }
  .points { list-style: disc; margin: 0 0 10px; padding-left: 20px; color: #666; max-width: 60ch; }
  .err { color: #b00020; margin: 0 0 4px; }
  .path { color: #666; margin: 0 0 6px; overflow-wrap: anywhere; }
  .result { margin: 8px 0 4px; }
  .drives { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 6px; }
  .drives li { display: flex; align-items: center; gap: 8px; }
  .drives small { color: #666; }
  .name { font-weight: 600; }
  .tag { margin-left: 6px; padding: 1px 6px; border-radius: 8px; background: #eef0f6; color: #555; font-weight: 400; }
  .source { margin-top: 16px; padding: 10px 12px; border: 1px solid #c3c8d4; border-radius: 8px; max-width: 70ch; }
  .source h3 { margin-top: 0; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; margin: 8px 0 0; }
  .opt { display: flex; align-items: center; gap: 8px; margin: 8px 0 0; white-space: nowrap; }
  .opt select { flex: 1 1 0; min-width: 0; max-width: 100%; }
  .progress { display: flex; align-items: center; gap: 10px; margin: 10px 0 0; }
  .progress progress { flex: 1; max-width: 320px; }
</style>
