<script module lang="ts">
  export type SettingsPage = 'folders' | 'viewing' | 'browsing' | 'analysis' | 'email' | 'about' | 'developer';
</script>

<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { isDebugBuild, openDataFolder, type FolderInfo } from './api';
  import AnalysisSettings from './AnalysisSettings.svelte';
  import type { AnalysisStatus } from './analysis';
  import PhotoFolders from './PhotoFolders.svelte';
  import type { ScanState } from './scanProgress';
  import { MAX_RECENT_DAYS, parseRecentDays } from './recent';
  import type { ViewerMode } from './viewerPref';

  let { folders, scanning, scanFolder, scan, onstop, onadd, onremove, onrescan, onemail, page, onpage, viewerMode, onviewer, recentDays, onrecentdays, analysis, people, onanalysispause, onanalysisretry }: {
    folders: FolderInfo[]; scanning: boolean; scanFolder: number | null; scan: ScanState; onstop: () => void; onadd: () => void; onremove: (id: number) => Promise<void>;
    onrescan: (id: number, full: boolean) => void; onemail: () => void; page: SettingsPage; onpage: (p: SettingsPage) => void;
    viewerMode: ViewerMode; onviewer: (m: ViewerMode) => void;
    recentDays: number; onrecentdays: (n: number) => void;
    analysis: AnalysisStatus | null; people: AnalysisStatus | null; onanalysispause: (paused: boolean) => void; onanalysisretry: () => void;
  } = $props();

  const pages: { id: SettingsPage; label: string }[] = [
    { id: 'folders', label: 'Photo folders' }, { id: 'viewing', label: 'Viewing' }, { id: 'browsing', label: 'Browsing' },
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
  const shownPages = $derived(debug ? [...pages, { id: 'developer' as SettingsPage, label: 'Developer' }] : pages);
  async function openData() {
    devError = '';
    try { await openDataFolder(); } catch (e) { devError = String(e); }
  }
</script>

<div class="settings">
  <nav aria-label="Settings">
    {#each shownPages as p (p.id)}
      <button class:on={page === p.id} onclick={() => onpage(p.id)}>{p.label}</button>
    {/each}
  </nav>
  <div class="page">
    {#if page === 'folders'}
      <PhotoFolders {folders} {scanning} {scanFolder} {scan} {onstop} {onadd} {onremove} {onrescan} />
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
    {:else if page === 'analysis'}
      <AnalysisSettings status={analysis} {people} onpause={onanalysispause} onretry={onanalysisretry} />
    {:else if page === 'email'}
      <h2>Email</h2>
      <p>Set up the account Pixie uses to send photos.</p>
      <button onclick={onemail}>Email account…</button>
    {:else if page === 'developer' && debug}
      <h2>Developer</h2>
      <p>Debug builds only.</p>
      <button onclick={openData}>Open data folder</button>
      {#if devError}<p class="err">{devError}</p>{/if}
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
  .days { width: 4.5em; font: inherit; padding: 2px 4px; }
</style>
