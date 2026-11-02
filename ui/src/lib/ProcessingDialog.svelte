<script lang="ts">
  import { onMount } from 'svelte';
  import ActivityScreen from './ActivityScreen.svelte';
  import PhotoStrip from './PhotoStrip.svelte';
  import type { ActivitySnapshot } from './activity';
  import type { StripItem } from './stripFeed';

  // The Processing screen as a modal page over the window, opened from the pill. Escape, the X or Close puts it away.
  let { snapshot, strip, scanPaused = false, onpause, onretry, onclose }: {
    snapshot: ActivitySnapshot | null;
    /** The photos Pixie has just looked at, going by. */
    strip: StripItem[];
    scanPaused?: boolean;
    onpause: (paused: boolean) => void;
    onretry: () => void;
    onclose: () => void;
  } = $props();

  let closeBtn: HTMLButtonElement;
  onMount(() => closeBtn.focus());
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onclose(); } }} />

<!-- Modal: a click on the dimmed window behind does nothing. Escape, the X and Close put it away. -->
<div class="scrim" role="presentation">
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="proc-title">
    <header>
      <h3 id="proc-title">Processing</h3>
      <button class="x" onclick={onclose} aria-label="Close" title="Close this and go back to what you were doing. Pixie keeps working in the background.">✕</button>
    </header>
    <div class="body">
      <PhotoStrip {strip} />
      <ActivityScreen {snapshot} {onpause} {onretry} {scanPaused} />
    </div>
    <footer>
      <button bind:this={closeBtn} onclick={onclose} title="Close this and go back to what you were doing. Pixie keeps working in the background.">Close</button>
    </footer>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 1400; display: grid; place-items: center; background: rgba(0, 0, 0, .4); }
  .dlg { display: flex; flex-direction: column; width: min(640px, calc(100vw - 32px)); height: min(640px, calc(100vh - 40px)); border-radius: 8px; background: #fff; color: #111; box-shadow: 0 8px 30px rgba(0, 0, 0, .35); }
  header { display: flex; align-items: center; justify-content: space-between; padding: 14px 20px 8px; }
  h3 { margin: 0; font-size: 16px; }
  .x { padding: 2px 9px; }
  .body { flex: 1; min-height: 0; display: flex; flex-direction: column; padding: 4px 20px 8px; }
  .body :global(.act) { flex: 1; height: auto; padding: 0; }
  footer { display: flex; justify-content: flex-end; padding: 10px 20px 14px; border-top: 1px solid #e6e9f2; }
</style>
