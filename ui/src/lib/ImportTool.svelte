<script lang="ts">
  /** Import/Export > Import: read photos from a camera card, a USB stick or any folder, look at them in Browse, and keep the ones you want in one
   *  of your folders. A source is only read: nothing is copied until Keep, and nothing on the card is ever changed. */
  import { open } from '@tauri-apps/plugin-dialog';
  import TagAs from './TagAs.svelte';
  import { importClose, importDiscard, importKeep, importOpen, importPickedCount, importStop, importZipExtract, importZipLook, type FolderInfo, type KeepWhich, type OpenSource, type ZipLook } from './api';
  import { plural } from './format';
  import { KEEP_LAYOUT, defaultDestination, extractLine, folderLabel, keepLine, keepPercent, showPath, spaceLine, summaryRows, workingRows, zipLine, zipProblems } from './importer';
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
  let error = $state('');
  /** The tag every photo imported in this window gets, if one was chosen (forgotten when the window is done). */
  let tag = $state<string | null>(null);
  let pickedIn = $state<Record<number, number>>({});
  /** A zip that was chosen and looked at, waiting for the person to extract it (or not). */
  /** The zip was chosen to be extracted and the Inbox is being set up for it. */
  let starting = $state(false);
  let zip = $state<{ path: string; look: ZipLook } | null>(null);

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

  const busy = $derived(imports.keeping !== null);
  /** One source at a time: loading another waits until this one is kept or discarded. */
  const hasSource = $derived(imports.sources.length > 0);
  /** The one loaded source, shown in a window of its own until the person is Done with it. */
  const source = $derived(imports.sources[0] ?? null);
  const showSource = $derived(source !== null && zip === null);
  /** Done is not offered while it would pull the rug from under work in progress: Stop comes first. */
  const doneTitle = (s: OpenSource) => (canFinish(s) ? 'Finish and clean up (Esc)' : 'Stop what is running first (Stop, in the bar at the top)');
  $effect(() => {
    if (source === null) tag = null; // whichever way the window went, the next import starts without a tag
  });
  const canFinish = (s: OpenSource) => imports.keeping !== s.id && !s.extracting && !(s.inbox && s.scanning);
  /** What to load from: each card or stick found, a folder, or a zip file. Choosing one loads it at once, then the list goes back to "Choose…". */
  let choice = $state('');
  const choices = $derived([
    ...imports.available.map((d) => ({ id: `drive:${d.path}`, label: d.card ? `${d.label} (camera card)` : d.label })),
    { id: 'folder', label: 'A folder…' },
    { id: 'zip', label: 'A zip file…' },
  ]);
  async function load() {
    const chosen = choice;
    choice = ''; // so the same entry can be chosen again, for instance after the file window was cancelled
    if (chosen === 'folder') await openFolder();
    else if (chosen === 'zip') await chooseZip();
    else if (chosen !== '') {
      const d = imports.available.find((a) => `drive:${a.path}` === chosen);
      if (d) await openPath(d.path, d.label);
    }
  }

  async function openPath(path: string, label?: string) {
    error = '';
    try {
      await importOpen(path, label);
      await refreshSources();
    } catch (e) {
      error = String(e);
    }
  }
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
    starting = true;
    try {
      await importZipExtract(zip.path);
      await refreshSources(); // the source is there before the first window goes, so there is one window all the way
      zip = null;
    } catch (e) {
      error = String(e);
    } finally {
      starting = false;
    }
  }
  async function keep(s: OpenSource, which: KeepWhich) {
    if (dest === null) return;
    error = '';
    imports.done = null;
    imports.progress = null;
    imports.keeping = s.id;
    savePref('import.dest', dest);
    try {
      await importKeep(s.id, which, dest, KEEP_LAYOUT, tag);
    } catch (e) {
      imports.keeping = null;
      error = String(e);
    }
  }
  /** Done (also Esc and the X): a card is closed in Pixie; a zip's extracted photos are deleted from the Inbox. Nothing on the card, and not the zip file, is touched. */
  async function finish(s: OpenSource) {
    if (!canFinish(s)) return;
    error = '';
    tag = null;
    try {
      if (s.inbox) await importDiscard(s.id);
      else await importClose(s.id);
    } catch (e) {
      error = String(e);
    }
    await refreshSources();
  }
  const stop = () => void importStop();
</script>

<svelte:window onkeydown={(e) => { if (e.key !== 'Escape') return; if (zip) { if (!starting) zip = null; } else if (source && showSource) void finish(source); }} />

<div class="import">
  <h2>Import</h2>
  <p class="lead">Bring photos and videos in from a camera card, a USB stick, a zip file or any folder.</p>
  <ul class="points">
    <li>You can select which files get imported.</li>
    <li>A card or folder is only read: your originals are never changed or deleted.</li>
  </ul>
  {#if error}<p class="err" role="alert">{error}</p>{:else if imports.error}<p class="err" role="alert">{imports.error}</p>{/if}

  <div class="row">
    <label class="opt">
      Import from:
      <select bind:value={choice} onchange={(e) => { choice = e.currentTarget.value; void load(); }} disabled={hasSource} aria-label="Where to import from" title={hasSource ? 'Import or discard the photos that are loaded first' : 'Choose a card or stick that was found, a folder, or a zip file. It is read at once; nothing is copied or changed until you import photos'}>
        <option value="" disabled>Choose&hellip;</option>
        {#each choices as c (c.id)}<option value={c.id}>{c.label}</option>{/each}
      </select>
    </label>
  </div>
</div>

{#if zip || (source && showSource)}
  <div class="scrim" role="presentation" onclick={(e) => { if (zip && !starting && e.target === e.currentTarget) zip = null; }}>
    <div class="dlg" class:wide={!zip} role="dialog" aria-modal="true" aria-label={zip ? zip.look.name : source?.label}>
      {#if zip}
        <h3>{zip.look.name}</h3>
        <ul class="points">
          <li>{zipLine(zip.look)}</li>
          {#each zipProblems(zip.look) as p (p)}<li>{p}</li>{/each}
          {#if spaceLine(zip.look)}<li class:err={!zip.look.fits}>{spaceLine(zip.look)}</li>{/if}
        </ul>
        <div class="actions end">
          <button disabled={starting} onclick={() => (zip = null)} title="Do not extract this zip">Cancel</button>
          <button class="primary" disabled={starting || !zip.look.fits || zip.look.photos + zip.look.videos === 0} onclick={extractZip}
                  title={zip.look.photos + zip.look.videos === 0 ? 'There is nothing to extract' : !zip.look.fits ? 'There is not enough free space for this zip' : 'Extract the photos and videos into the Inbox, where you can look at them and keep the ones you want. The zip file is not changed'}>OK</button>
        </div>
      {:else if source}
        {@const s = source}
        {@const pickedHere = pickedIn[s.id] ?? 0}
        <header>
          <h3 id="imp-title">{s.label}</h3>
          <button class="x" disabled={!canFinish(s)} onclick={() => finish(s)} aria-label="Close" title={doneTitle(s)}>&times;</button>
        </header>
        {#if !s.inbox && showPath(s.label, s.path)}<p class="path" title={s.path}>{s.path}</p>{/if}

        {@const working = s.extracting || s.scanning}
        <div class="progress" role="status">
          {#if s.extracting}
            <progress value={imports.extract && imports.extract.bytes_total > 0 ? Math.round((imports.extract.bytes_done / imports.extract.bytes_total) * 100) : 0} max="100" aria-label="Extracting the zip"></progress>
            <span>{extractLine(imports.extract?.done ?? 0, imports.extract?.total ?? 0)}</span>
            <button onclick={stop} title="Stop extracting. What was extracted so far stays in the Inbox">Stop</button>
          {:else if s.scanning}
            <span class="spin" aria-hidden="true"></span>
            <span>Reading&hellip;</span>
            <button onclick={stop} title="Stop reading this card. What was found so far is kept">Stop</button>
          {:else if imports.keeping === s.id}
            <progress value={keepPercent(imports.progress)} max="100" aria-label="Importing photos"></progress>
            <span>{imports.progress ? `${imports.progress.done.toLocaleString()} of ${imports.progress.total.toLocaleString()}` : 'Starting…'}</span>
            <button onclick={stop} title="Stop after the file being copied. What is already imported stays, and the rest is still there to import later">Stop</button>
          {:else}
            <progress value="100" max="100" aria-label="Nothing is running"></progress>
            <span>Ready</span>
          {/if}
        </div>
        <div class="body">
        <dl class="facts">
          {#each working ? workingRows(s.extracting ? imports.extract : null, s.scanning && imports.scan && imports.scan.source === s.id ? imports.scan.seen : 0) : summaryRows(s.summary, pickedHere) as r (r.label)}
            <dt>{r.label}:</dt>
            <dd>{r.value}{#if r.label === 'New' && (working || s.summary.photos + s.summary.videos > 0)}<button disabled={working || s.summary.new === 0} onclick={() => onbrowse(s.id)} title={working ? 'Available when the photos have been read' : s.summary.new === 0 ? 'Nothing here is new' : 'Show the new photos in Browse, where you can look at them and pick the ones you want'}>Browse</button>{/if}</dd>
          {/each}
        </dl>
        {#if working || s.summary.photos + s.summary.videos > 0}
          {@const nothingNew = s.summary.new === 0}
          <p class="how">Browse to pick the ones to import, or click All to import all new files.</p>
          <div class="form">
            <div class="tagrow" role="group" aria-labelledby="imp-tag">
              <span id="imp-tag" class="note">You can tag the imported files:</span>
              {#if tag !== null}
                <span class="pill">{tag}<button class="pillx" disabled={busy} aria-label="Do not tag" title="Do not tag the photos you import" onclick={() => (tag = null)}>&times;</button></span>
              {:else}
                <TagAs ids={[]} onchoose={(t) => (tag = t)} onchange={() => {}} disabled={working || busy || nothingNew} />
              {/if}
            </div>
            <span id="imp-do">Import:</span>
            <div class="actions" role="group" aria-labelledby="imp-do">
              <button disabled={working || busy || nothingNew || dest === null || pickedHere === 0} onclick={() => keep(s, 'picked')}
                      title={working ? 'Available when the photos have been read' : nothingNew ? 'Nothing here is new' : pickedHere === 0 ? 'Pick photos in Browse first (the star in a photo’s corner)' : s.inbox ? 'Move the photos you picked out of the Inbox into the folder, checking each copy first' : 'Copy the photos you picked on this card into the folder, checking each copy'}>Picked</button>
              <button class="primary" disabled={working || busy || nothingNew || dest === null} onclick={() => keep(s, 'new')}
                      title={working ? 'Available when the photos have been read' : nothingNew ? 'Nothing here is new' : s.inbox ? 'Move everything that is new out of the Inbox into the folder, checking each copy first' : 'Copy everything that is new into the folder, checking each copy. The card is only read'}>All</button>
            </div>
            <span id="imp-into">Into:</span>
            <select bind:value={dest} disabled={working || busy || nothingNew || folders.length === 0} aria-labelledby="imp-into" title={nothingNew ? 'Nothing here is new' : 'The folder the photos you import are put into, filed by year and date'}>
              {#each folders as f (f.id)}<option value={f.id}>{folderLabel(f.path)}</option>{/each}
            </select>
          </div>
          {#if folders.length === 0}<p class="hint">Add a folder in Settings, Photo folders, first.</p>{/if}
        {/if}

        <div class="outcome">
        {#if imports.keeping !== s.id && imports.done && imports.done.source === s.id}
          {#if imports.done.error}
            <p class="err" role="alert">{imports.done.error}</p>
          {:else if imports.done.report}
            <p class="result" role="status">{keepLine(imports.done.report)}</p>
            {#each imports.done.report.problems as p (p)}<p class="err">{p}</p>{/each}
          {/if}
        {/if}
        {#if error}<p class="err" role="alert">{error}</p>{/if}
        </div>
        </div>

        <footer>
          <span class="gap"></span>
          <button disabled={!canFinish(s)} onclick={() => finish(s)} title={doneTitle(s)}>Done</button>
        </footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .import { padding: var(--page-pad); font-size: 14px; }
  h2 { margin: 0 0 12px; font-size: 18px; }
  h3 { margin: 0; font-size: 16px; overflow-wrap: anywhere; }
  p { margin: 0 0 8px; }
  .hint { color: #666; max-width: 60ch; }
  .err { color: #b00020; margin: 0 0 4px; }
  .path { color: #666; margin: 4px 0 0; overflow-wrap: anywhere; }
  .lead { margin: 0 0 8px; }
  .how { color: #555; font-size: 13px; margin: 0 0 12px; }
  /* Everything between the two light lines is centred in the room there is. */
  .body { flex: 1; display: flex; flex-direction: column; justify-content: center; min-height: 0; padding: 36px 0 12px; }
  .outcome { min-height: 36px; } /* room for the result line below, matched above so the controls stay in the middle */
  .facts { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 4px 12px; margin: 12px 0; }
  .facts dt { color: #666; }
  .facts dd { margin: 0; font-weight: 600; display: flex; align-items: center; gap: 12px; }
  .facts dd button { font-weight: 400; }
  .result { margin: 12px 0 4px; }
  .points { list-style: disc; margin: 0 0 12px; padding-left: 20px; display: grid; gap: 4px; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .opt { display: flex; align-items: center; gap: 8px; }
  .scrim { position: fixed; inset: 0; z-index: 1400; display: grid; place-items: center; background: rgba(0, 0, 0, .4); }
  .dlg { width: min(460px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: auto; padding: 18px 20px; border-radius: 8px; background: #fff; color: #111; box-shadow: 0 8px 30px rgba(0, 0, 0, .35); font-size: 14px; }
  /* The import window keeps one size while it is used, so it never resizes under the person: the buttons stay at the bottom. (The zip's first screen is a window of its own, smaller.) */
  .dlg.wide { width: min(540px, calc(100vw - 32px)); height: min(490px, calc(100vh - 68px)); display: flex; flex-direction: column; }
  .dlg > h3 { margin-bottom: 10px; }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .x { padding: 0 8px; font-size: 18px; line-height: 1.3; }
  .form { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 8px 10px; margin: 8px 0 12px; }
  .form select { min-width: 0; max-width: 100%; }
  .tagrow { grid-column: 1 / -1; display: flex; align-items: center; gap: 8px; min-height: 30px; }
  .note { color: #555; }
  .pill { display: inline-flex; align-items: center; gap: 4px; padding: 1px 4px 1px 8px; border-radius: 4px; background: #e3ecff; color: #2a3f7a; font-size: 12px; }
  .pillx { border: 0; background: none; padding: 0 4px; font: inherit; line-height: 1; cursor: pointer; color: inherit; }
  .actions { display: flex; align-items: center; gap: 8px; margin: 0 0 4px; }
  .actions.end { justify-content: flex-end; margin-top: 16px; }
  .gap { flex: 1; }
  .progress { display: flex; align-items: center; gap: 10px; margin: 12px 0 0; min-height: 32px; padding-bottom: 12px; border-bottom: 1px solid #e3e6ee; }
  .spin { width: 16px; height: 16px; border: 2px solid #c3c8d4; border-top-color: #2f6fe4; border-radius: 50%; animation: spin .8s linear infinite; flex: none; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spin { animation-duration: 3s; } }
  .progress progress { flex: 1; min-width: 0; }
  footer { margin-top: auto; padding-top: 12px; border-top: 1px solid #e3e6ee; display: flex; }
</style>
