<script module lang="ts">
  export type SettingsPage = 'folders' | 'viewing' | 'email' | 'about';
</script>

<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import type { FolderInfo } from './api';
  import PhotoFolders from './PhotoFolders.svelte';
  import type { ViewerMode } from './viewerPref';

  let { folders, scanning, onadd, onremove, onrescan, onemail, page, onpage, viewerMode, onviewer }: {
    folders: FolderInfo[]; scanning: boolean; onadd: () => void; onremove: (id: number) => Promise<void>;
    onrescan: (id: number) => void; onemail: () => void; page: SettingsPage; onpage: (p: SettingsPage) => void;
    viewerMode: ViewerMode; onviewer: (m: ViewerMode) => void;
  } = $props();

  const pages: { id: SettingsPage; label: string }[] = [
    { id: 'folders', label: 'Photo folders' }, { id: 'viewing', label: 'Viewing' },
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
  .settings { display: grid; grid-template-columns: 200px minmax(0, 1fr); height: 100%; background: #fff; }
  nav { background: #fafafb; border-right: 1px solid #e4e4e8; padding: 12px 8px; display: grid; align-content: start; gap: 2px; }
  nav button { text-align: left; border: 0; background: none; font: inherit; padding: 7px 12px; border-radius: 4px; cursor: pointer; }
  nav button.on { background: #e3ecff; font-weight: 600; }
  .page { padding: 20px 28px; overflow-y: auto; }
  .opt { display: block; margin: 6px 0; }
</style>
