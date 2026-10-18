<script module lang="ts">
  export type SettingsPage = 'folders' | 'viewing' | 'browsing' | 'email' | 'about';
</script>

<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import type { FolderInfo } from './api';
  import PhotoFolders from './PhotoFolders.svelte';
  import { MAX_RECENT_DAYS, parseRecentDays } from './recent';
  import type { ViewerMode } from './viewerPref';

  let { folders, scanning, onadd, onremove, onrescan, onemail, page, onpage, viewerMode, onviewer, recentDays, onrecentdays }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void; onremove: (id: number) => Promise<void>;
    onrescan: (id: number) => void; onemail: () => void; page: SettingsPage; onpage: (p: SettingsPage) => void;
    viewerMode: ViewerMode; onviewer: (m: ViewerMode) => void;
    recentDays: number; onrecentdays: (n: number) => void;
  } = $props();

  const pages: { id: SettingsPage; label: string }[] = [
    { id: 'folders', label: 'Photo folders' }, { id: 'viewing', label: 'Viewing' }, { id: 'browsing', label: 'Browsing' },
    { id: 'email', label: 'Email' }, { id: 'about', label: 'About' },
  ];
  let version = $state('');
  onMount(() => { getVersion().then((v) => (version = v)).catch(() => {}); });
</script>

<div class="settings">
  <nav aria-label="Settings">
    {#each pages as p (p.id)}
      <button class:on={page === p.id} onclick={() => onpage(p.id)}>{p.label}</button>
    {/each}
  </nav>
  <div class="page">
    {#if page === 'folders'}
      <PhotoFolders {folders} {scanning} {onadd} {onremove} {onrescan} />
    {:else if page === 'viewing'}
      <h2>Viewing</h2>
      <p>Double-click or Enter on an item opens it in:</p>
      <label class="opt"><input type="radio" name="viewer" checked={viewerMode === 'window'} onchange={() => onviewer('window')} /> The full window</label>
      <label class="opt"><input type="radio" name="viewer" checked={viewerMode === 'popout'} onchange={() => onviewer('popout')} /> A separate window (for a second monitor)</label>
    {:else if page === 'browsing'}
      <h2>Browsing</h2>
      <p>The "recent" shortcut at the top of the date list shows items from the last
        <input class="days" type="number" min="1" max={MAX_RECENT_DAYS} step="1" value={recentDays} aria-label="Days counted as recent"
               onchange={(e) => { const n = Number(e.currentTarget.value); onrecentdays(n); e.currentTarget.value = String(parseRecentDays(n)); }} /> days.</p>
    {:else if page === 'email'}
      <h2>Email</h2>
      <p>Set up the account Pixie uses to send photos.</p>
      <button onclick={onemail}>Email account…</button>
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
  .days { width: 4.5em; font: inherit; padding: 2px 4px; }
</style>
