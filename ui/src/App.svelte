<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { addFolder, cancelScan, countPhotos, listFolders, startScan } from './lib/api';
  import Grid from './lib/Grid.svelte';
  import Viewer from './lib/Viewer.svelte';
  import PrintTest from './lib/PrintTest.svelte';

  let total = $state(0), version = $state(0);
  let scanning = $state(false), seen = $state(0), message = $state('');
  let openIndex = $state<number | null>(null);
  let view = $state<'library' | 'print'>('library');

  async function refresh() { total = await countPhotos(); version++; }

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const id = await addFolder(dir);
    message = '';
    scanning = true; seen = 0; // before the call, so a fast scan-done cannot be overwritten
    try {
      await startScan(id);
    } catch (e) {
      message = String(e);
      if (!/already running/.test(message)) scanning = false; // busy => the other scan is still going
    }
  }

  onMount(() => {
    refresh();
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      if (!fs.length) return;
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number }>('scan-progress', (e) => { seen = e.payload.seen; countPhotos().then((n) => (total = n)); }),
      listen('scan-done', () => { scanning = false; refresh(); }),
      listen<{ message: string }>('scan-error', (e) => { message = e.payload.message; /* scan-done always follows */ }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<main>
  <header>
    <strong>Pixie</strong>
    <button onclick={chooseFolder}>Add photo folder…</button>
    <button onclick={() => (view = view === 'print' ? 'library' : 'print')}>Print test</button>
    {#if scanning}
      <span>Scanning… {seen} photos</span>
      <button onclick={() => cancelScan()}>Stop</button>
    {:else}
      <span>{total} photos</span>
    {/if}
    {#if message}<span class="err">{message}</span>{/if}
  </header>
  <section>
    {#if view === 'print'}
      <PrintTest />
    {:else if total === 0 && !scanning}
      <p class="empty">Add a folder of photos to get started.</p>
    {:else}
      <Grid {total} {version} onopen={(i) => (openIndex = i)} />
    {/if}
  </section>
  {#if openIndex !== null}
    <Viewer index={openIndex} {total} onclose={() => (openIndex = null)} onnav={(i) => (openIndex = i)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }
  section { flex: 1; min-height: 0; padding: 8px; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
</style>
