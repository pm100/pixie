<script lang="ts">
  /** Import/Export > Sync: keep a folder (the sync folder) in step with the folders Pixie watches. What is in the folders ticked here and
   *  not yet in the sync folder is synced into it (Sync now). With no sync folder, only the choice of one is shown. */
  import type { FolderInfo } from './api';
  import { copyLine, lastCopyLine, type CopyStatus } from './master';

  let { folders, master = null, onmaster = () => {}, subfolder = '', onsubfolder = async () => {}, copyStatus = null, oncopy = () => {}, onstopcopy = () => {}, onfeeds = () => {}, remind = true, onremind = () => {} }: {
    folders: FolderInfo[];
    /** The sync folder, or null while there is none: then nothing else of the feature is shown. */
    master?: string | null; onmaster?: (path: string | null) => void;
    /** The folder inside the sync folder that copies go into ('' for the sync folder itself), and choosing it (rejects with words for the person when the name cannot be used). */
    subfolder?: string; onsubfolder?: (name: string) => Promise<void>;
    /** What is waiting to be synced into the sync folder, how the last sync went, and the controls to start and stop one. */
    copyStatus?: CopyStatus | null; oncopy?: () => void; onstopcopy?: () => void;
    /** Whether new items found in a folder wait for review before they are synced into the sync folder. */
    onfeeds?: (id: number, feeds: boolean) => void;
    /** Whether Pixie reminds at start about new items waiting to be synced, and a change to that (kept on this computer). */
    remind?: boolean; onremind?: (on: boolean) => void;
  } = $props();

  let draft = $state('');
  let subError = $state('');
  $effect(() => { draft = subfolder; });
  async function saveSub() {
    subError = '';
    if (draft.trim() === subfolder) return;
    try { await onsubfolder(draft); } catch (e) { subError = String(e); draft = subfolder; }
  }
  /** Where copies go, as a path to show: the sync folder, then the folder chosen inside it. */
  const target = $derived.by(() => {
    if (master === null || subfolder === '') return master ?? '';
    const sep = master.includes('\\') ? '\\' : '/';
    return master.replace(/[\\/]+$/, '') + sep + subfolder.split('/').join(sep);
  });
</script>

<div class="sync">
  <h2>Sync</h2>
  <p class="hint">Keeps one folder, the sync folder, up to date with photos from your other folders: a family drive, say, or a drive on your own computer that gets what arrives in the cloud. Only photos you mark private are kept out. Originals are never changed or deleted. Until you choose a sync folder, none of this is used.</p>
  <label class="opt">
    Sync folder:
    <select value={master ?? ''} disabled={folders.length === 0} title="Choose which of your folders photos are synced into, or None to go without one. No files are moved or deleted"
            aria-label="Sync folder" onchange={(e) => onmaster(e.currentTarget.value === '' ? null : e.currentTarget.value)}>
      <option value="">None</option>
      {#each folders as f (f.id)}<option value={f.path}>{f.path}</option>{/each}
    </select>
  </label>
  {#if folders.length === 0}<p class="hint">Add a folder in Settings, Photo folders, first.</p>{/if}

  {#if master !== null}
    <label class="sub">
      Subfolder:
      <input type="text" bind:value={draft} placeholder="None: copy straight into the sync folder" spellcheck="false" aria-label="Subfolder of the sync folder" title="A folder inside the sync folder to put the copies in, such as paul_sync, so they stay apart from other files on the drive. Leave empty to copy straight into the sync folder. It is made when the first photo is copied" onchange={saveSub} />
    </label>
    {#if subError}<p class="err" role="alert">{subError}</p>{/if}
    <p class="hint">Photos are copied into {target}, filed by year and date. Changing this does not move photos already copied.</p>
    <h3>Sync from</h3>
    <p class="hint">What is in a ticked folder is synced into the sync folder, except photos you mark private. Browse shows what is still to be synced under Sync, New, not yet synced.</p>
    <ul>
      {#each folders.filter((f) => f.path !== master) as f (f.id)}
        <li>
          <label title="Sync what is in this folder into the sync folder, except photos marked private">
            <input type="checkbox" checked={f.feeds_master} onchange={(e) => onfeeds(f.id, e.currentTarget.checked)} /> {f.path}
          </label>
        </li>
      {/each}
    </ul>
    {#if folders.every((f) => f.path === master)}<p class="hint">There are no other folders to sync from.</p>{/if}

    {#if copyStatus !== null}
      <h3>Syncing</h3>
      <p class="copy" role="status">{copyLine(copyStatus)}</p>
      {#if copyStatus.running}
        <button onclick={onstopcopy} title="Pause syncing after the file being synced; the rest stays waiting, and Sync now carries on">Pause</button>
      {:else}
        <button class="primary" disabled={copyStatus.pending === 0} onclick={oncopy} title={copyStatus.pending === 0 ? 'Nothing is waiting to be synced' : 'Sync the new items into the sync folder now. Originals are never changed or deleted'}>Sync now</button>
      {/if}
      {#if copyStatus.last !== null && !copyStatus.running}
        <p class="hint">{lastCopyLine(copyStatus.last)}</p>
        {#each copyStatus.last.problems as p (p)}<p class="err">{p}</p>{/each}
      {/if}
    {/if}

    <h3>Reminder</h3>
    <label class="opt" title="When Pixie starts, say so if there are new items waiting to be synced">
      <input type="checkbox" checked={remind} onchange={(e) => onremind(e.currentTarget.checked)} /> Remind me at start about new items
    </label>
  {/if}
</div>

<style>
  .sync { padding: var(--page-pad); font-size: 14px; }
  h2 { margin: 0 0 8px; font-size: 18px; }
  h3 { margin: 16px 0 6px; font-size: 14px; }
  p { margin: 0 0 8px; }
  .hint { color: #666; max-width: 60ch; }
  .sub { display: flex; align-items: center; gap: 8px; margin: 12px 0 6px; }
  .sub input { flex: 0 1 360px; min-width: 0; padding: 5px 8px; font-size: 14px; }
  .err { color: #b00020; margin: 0 0 4px; }
  .copy { margin: 8px 0; }
  ul { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 4px; }
  li label { display: inline-flex; align-items: center; gap: 6px; }
</style>
