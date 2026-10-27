<script lang="ts">
  /** The second screen of the first run: what Pixie does with the photos (People, Auto tags, Duplicates), then Start. */
  import AnalysisChoices from './AnalysisChoices.svelte';
  import type { AnalysisOptions } from './analysisOptions';
  import FlowStage from './FlowStage.svelte';

  let { options, onoptions, onback, onstart }: {
    options: AnalysisOptions; onoptions: (next: AnalysisOptions) => void;
    onback: () => void; onstart: () => void;
  } = $props();
</script>

<FlowStage label="What Pixie does with your photos" title="What should Pixie do with your photos?">
  <p class="intro">Pixie can study your photos to help you find them. All of this happens on your computer; nothing is sent to the cloud.</p>
  <p class="choose">Choose what you would like:</p>
  <AnalysisChoices {options} onchange={onoptions} />
  <p class="note">You can always change it later in <b>Settings → Picture analysis</b>.</p>

  {#snippet footer()}
    <button onclick={onback}>Back</button>
    <button class="primary" onclick={onstart}>Start</button>
  {/snippet}
</FlowStage>

<style>
  .intro { margin: 0 0 14px; text-align: left; }
  .choose { margin: 0 0 12px; text-align: left; font-weight: 600; }
  .note { color: #666; font-size: 13px; margin: 14px 0 0; }
</style>
