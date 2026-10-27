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

<FlowStage label="What Pixie does with your photos" title="What should Pixie do with your photos?"
           sub="Pixie can study your photos to help you find them. Choose what you would like:">
  <AnalysisChoices {options} onchange={onoptions} />
  <p class="note">All of this happens on your computer; nothing is uploaded. You can always change it later in <b>Settings → Picture analysis</b>.</p>

  {#snippet footer()}
    <button onclick={onback}>Back</button>
    <button class="primary" onclick={onstart}>Start</button>
  {/snippet}
</FlowStage>

<style>
  .note { color: #666; font-size: 13px; margin: 14px 0 0; }
</style>
