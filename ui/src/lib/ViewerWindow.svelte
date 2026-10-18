<script lang="ts">
  import { emit, listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { POPOUT_CLOSE, POPOUT_CLOSED, POPOUT_MOVED, POPOUT_READY, POPOUT_SHOW, parseShow, type ShowMessage } from './popout';
  import Viewer from './Viewer.svelte';

  let show = $state<ShowMessage | null>(null);
  let follow = $state(true);

  onMount(() => {
    const un = listen(POPOUT_SHOW, (e) => {
      const m = parseShow(e.payload);
      if (m && (!m.follow || follow)) show = m; // selection-driven updates only while Follow is on
    });
    const unClose = listen(POPOUT_CLOSE, () => close());
    const bye = () => { void emit(POPOUT_CLOSED); };
    window.addEventListener('beforeunload', bye); // the OS close button destroys the page without cleanup
    // Ask for the photo only once the listener is registered; an answer sent earlier would be lost.
    void un.then(() => emit(POPOUT_READY));
    return () => { un.then((f) => f()); unClose.then((f) => f()); window.removeEventListener('beforeunload', bye); bye(); };
  });

  function nav(i: number) {
    if (!show) return;
    show = { ...show, index: i };
    if (follow) void emit(POPOUT_MOVED, { index: i, total: show.total, view: show.view });
  }
  const close = () => { void getCurrentWindow().close(); };
</script>

{#if show}
  <!-- A new listing remounts the viewer so its neighbour cache (keyed by position) cannot show the old one's photos. -->
  {#key JSON.stringify(show.view)}
    <Viewer index={show.index} total={show.total} view={show.view} onclose={close} onnav={nav} filmstrip={false} />
  {/key}
  <label class="follow"><input type="checkbox" bind:checked={follow} /> Follow my selection</label>
{:else}
  <p class="wait">Waiting for a photo…</p>
{/if}

<style>
  .follow { position: fixed; z-index: 2100; left: 16px; bottom: 10px; color: #eee; font: 13px system-ui, sans-serif; text-shadow: 0 0 4px #000; }
  .wait { color: #888; text-align: center; margin-top: 30vh; font-family: system-ui, sans-serif; }
</style>
