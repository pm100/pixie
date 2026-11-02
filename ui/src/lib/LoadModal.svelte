<script lang="ts">
  import ActivityScreen from './ActivityScreen.svelte';
  import FlowStage from './FlowStage.svelte';
  import type { ActivitySnapshot } from './activity';
  import PhotoStrip from './PhotoStrip.svelte';
  import type { StripItem } from './stripFeed';

  type Sample = StripItem;

  let { strip, snapshot, scanPaused = false, notice = '', onbackground, onpause, onretry }: {
    /** The photos read most recently, oldest first. */
    strip: Sample[];
    /** Everything running in the background: the same rows as the Processing screen. */
    snapshot: ActivitySnapshot | null;
    /** A folder scan was stopped by Pause and has not been started again: the Pause button offers Resume. */
    scanPaused?: boolean;
    /** A problem to show, such as a folder that could not be read; the scan carries on with the others. */
    notice?: string;
    /** Pause (true) or Resume (false) everything: the button is at the top of the progress section, as on the Processing screen. */
    onpause: (paused: boolean) => void; onbackground: () => void; onretry: () => void;
  } = $props();

</script>

<FlowStage label="Getting to know your photos">
  <h1>Getting to know your photos…</h1>
  <p class="sub">Pixie is looking at each photo. It adds Auto tags (labels such as Beach or Museum that you can browse by) and finds the people in them. This all happens on your computer; nothing is sent to the cloud.</p>
  {#if notice}<p class="err" role="alert">{notice}</p>{/if}
  <!-- The pictures go by the whole time: first as the files are found, then as each is looked at. -->
  <PhotoStrip {strip} />
  <!-- One screen from the first moment: the files are worked on as the walk finds them. -->
  <ActivityScreen {snapshot} {onpause} {onretry} {scanPaused} compact />
  {#snippet footer()}
    <span class="note foot">This can take a while for a big library. You don't have to wait here.</span>
    <button class="primary" onclick={onbackground} title="Close this window; the work carries on in the background">Carry on in the background</button>
  {/snippet}
</FlowStage>

<style>
  .err { margin-top: 6px; color: #b00020; font-size: 13px; }
  .note { color: #666; font-size: 13px; }
  .foot { margin-right: auto; }
</style>
