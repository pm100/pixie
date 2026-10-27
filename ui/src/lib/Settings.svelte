<script module lang="ts">
  export type SettingsPage = 'folders' | 'viewing' | 'browsing' | 'trips' | 'analysis' | 'email' | 'about' | 'developer';
</script>

<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { isDebugBuild, openDataFolder, resetLibrary, type FolderInfo, type LoadStatus } from './api';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import AnalysisSettings from './AnalysisSettings.svelte';
  import EmailSettings from './EmailSettings.svelte';
  import type { AnalysisOptions } from './analysisOptions';
  import type { AnalysisView } from './analysisView';
  import PhotoFolders from './PhotoFolders.svelte';
  import TripSettings from './TripSettings.svelte';
  import type { TripPrefs } from './trips';
  import type { ScanState } from './scanProgress';
  import { MAX_RECENT_DAYS, parseRecentDays } from './recent';
  import type { ViewerMode } from './viewerPref';
  import type { NetworkRescan } from './networkRescan';
  import type { FolderStates } from './folderState';

  let { folders, folderStates = {}, analysing = null, analysisNote = null, scanning, scanFolder, scan, onstop, onadd, onremove, networkRescan, onnetworkrescan, page, onpage, viewerMode, onviewer, recentDays, onrecentdays, tripPrefs, ontripprefs, analysis, onanalysispause, onanalysisretry, analysisOptions, onanalysisoptions, loading, emailLimitMb, onemaillimit }: {
    /** The loading of the files found, and whether the Pause is on. */
    loading: LoadStatus;
    /** Which analyses are on, and a change to that. */
    analysisOptions: AnalysisOptions; onanalysisoptions: (next: AnalysisOptions) => void;
    /** The email size limit in megabytes, and how it is changed. */
    emailLimitMb: number; onemaillimit: (mb: number) => void;
    folders: FolderInfo[]; folderStates?: FolderStates; scanning: boolean;
    /** The folders just added, and how far their analysis has got (null: nothing newly added is still being analysed). */
    analysing?: { ids: number[]; fraction: number | null } | null;
    /** Says what the Picture analysis numbers are about when they are for the folders just added. */
    analysisNote?: string | null; scanFolder: number | null; scan: ScanState; onstop: () => void; onadd: () => void; onremove: (id: number) => Promise<void>;
    networkRescan: NetworkRescan; onnetworkrescan: (r: NetworkRescan) => void; page: SettingsPage; onpage: (p: SettingsPage) => void;
    viewerMode: ViewerMode; onviewer: (m: ViewerMode) => void;
    recentDays: number; onrecentdays: (n: number) => void;
    tripPrefs: TripPrefs; ontripprefs: (p: TripPrefs) => void;
    analysis: AnalysisView; onanalysispause: (paused: boolean) => void; onanalysisretry: () => void;
  } = $props();

  const pages: { id: SettingsPage; label: string }[] = [
    { id: 'folders', label: 'Photo folders' }, { id: 'viewing', label: 'Viewing' }, { id: 'browsing', label: 'Browsing' }, { id: 'trips', label: 'Trips' },
    { id: 'analysis', label: 'Picture analysis' }, { id: 'email', label: 'Email' }, { id: 'about', label: 'About' },
  ];
  let version = $state('');
  // Debug builds only: a Developer page.
  let debug = $state(false);
  let devError = $state('');
  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {});
    isDebugBuild().then((d) => (debug = d)).catch(() => {});
  });
  // The one Pause covers Auto tags, faces and the duplicate check; the menu says so when it is on.
  const analysisPaused = $derived(analysis.paused);
  const labelled = (p: { id: SettingsPage; label: string }) => (p.id === 'analysis' && analysisPaused ? `${p.label} (paused)` : p.label);
  const shownPages = $derived(debug ? [...pages, { id: 'developer' as SettingsPage, label: 'Developer' }] : pages);
  let confirmReset = $state(false);
  let resetting = $state(false);
  /** Forget everything and start the first run again: the window is reloaded, and with no folders it opens on the welcome. */
  async function startAgain() {
    confirmReset = false;
    resetting = true;
    devError = '';
    try {
      await resetLibrary();
      try { globalThis.localStorage?.clear(); } catch { /* preferences are a convenience */ }
      globalThis.location.reload();
    } catch (e) {
      devError = String(e);
      resetting = false;
    }
  }
  async function openData() {
    devError = '';
    try { await openDataFolder(); } catch (e) { devError = String(e); }
  }
</script>

<div class="settings">
  <nav aria-label="Settings">
    {#each shownPages as p (p.id)}
      <button class:on={page === p.id} onclick={() => onpage(p.id)}>{labelled(p)}</button>
    {/each}
  </nav>
  <div class="page">
    {#if page === 'folders'}
      <PhotoFolders {folders} {folderStates} {analysing} {scanning} {scanFolder} {scan} {onstop} {onadd} {onremove} {networkRescan} {onnetworkrescan} />
    {:else if page === 'viewing'}
      <h2>Viewing</h2>
      <p>Double-click or Enter on an item opens it in:</p>
      <label class="opt"><input type="radio" name="viewer" checked={viewerMode === 'window'} onchange={() => onviewer('window')} /> The full window</label>
      <label class="opt"><input type="radio" name="viewer" checked={viewerMode === 'popout'} onchange={() => onviewer('popout')} /> A separate window</label>
    {:else if page === 'browsing'}
      <h2>Browsing</h2>
      <p>The "recent" shortcut at the top of the date list shows items from the last
        <input class="days" type="number" min="1" max={MAX_RECENT_DAYS} step="1" value={recentDays} aria-label="Days counted as recent"
               onchange={(e) => { const n = Number(e.currentTarget.value); onrecentdays(n); e.currentTarget.value = String(parseRecentDays(n)); }} /> days.</p>
    {:else if page === 'trips'}
      <TripSettings prefs={tripPrefs} onprefs={ontripprefs} />
    {:else if page === 'analysis'}
      <AnalysisSettings view={analysis} note={analysisNote} onpause={onanalysispause} onretry={onanalysisretry} options={analysisOptions} onoptions={onanalysisoptions} {loading} />
    {:else if page === 'email'}
      <EmailSettings limitMb={emailLimitMb} onlimit={onemaillimit} />
    {:else if page === 'developer' && debug}
      <h2>Developer</h2>
      <p>Debug builds only.</p>
      <p><button onclick={openData}>Open data folder</button></p>
      <p><button class="danger" onclick={() => (confirmReset = true)} disabled={resetting}>{resetting ? 'Starting again…' : 'Start again from a clean slate…'}</button></p>
      <p class="hint">Forgets every folder, photo, face, name, tag and setting, and shows the welcome again. Your photo files and the downloaded models are left alone.</p>
      {#if devError}<p class="err">{devError}</p>{/if}
      {#if confirmReset}
        <ConfirmDialog title="Start again from a clean slate?" confirmLabel="Start again" danger
          message="This removes every folder, photo record, face, name, tag, pick and setting from Pixie, and the thumbnails made from them, then shows the welcome screen. Your photo files are not touched. The downloaded models are kept."
          onconfirm={startAgain} oncancel={() => (confirmReset = false)} />
      {/if}
    {:else}
      <h2>About</h2>
      <p>Pixie {version}</p>
      <p>A simple photo and video manager. MIT licence.</p>
    {/if}
  </div>
</div>

<style>
  .settings { display: grid; grid-template-columns: 230px minmax(0, 1fr); height: 100%; background: #fff; }
  /* Same rhythm as the Browse navigation: 14px text, 4px 12px rows, 4px corners. */
  nav { background: #fafafb; border-right: 1px solid #e4e4e8; padding: 6px 4px 16px; font-size: 14px; display: grid; align-content: start; gap: 1px; }
  nav button { text-align: left; border: 0; background: none; font: inherit; padding: 4px 12px; border-radius: 4px; cursor: pointer; }
  nav button:hover:not(.on) { background: #eef2fb; }
  nav button.on { background: #e3ecff; font-weight: 600; }
  .page { padding: 16px 24px; font-size: 14px; overflow-y: auto; }
  .page :global(h2) { margin: 0 0 8px; font-size: 18px; }
  .page :global(p) { margin: 0 0 8px; }
  .opt { display: block; margin: 6px 0; }
  .err { color: #b00020; }
  button.danger { background: #b3261e; border-color: #b3261e; color: #fff; }
  .hint { color: #666; font-size: 13px; }
  .days { width: 4.5em; font: inherit; padding: 2px 4px; }
</style>
