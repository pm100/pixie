<script lang="ts">
  import AnalysisChoices from './AnalysisChoices.svelte';
  import type { AnalysisOptions } from './analysisOptions';
  import { locationsLine } from './analysisView';

  let { options, onoptions, locations = null }: {
    /** Which analyses are on, and a change to that. */
    options: AnalysisOptions; onoptions: (next: AnalysisOptions) => void;
    /** How many of the library's items have no location, of how many (said once some have none). */
    locations?: { without: number; total: number } | null;
  } = $props();
  const noLocations = $derived(locationsLine(locations));
</script>

<h2>Picture analysis</h2>
<p>Pixie studies your photos on this computer. Nothing leaves your computer except the one-time download of the
  picture-analysis and face models.</p>
<AnalysisChoices {options} onchange={onoptions} />
<p class="hint">Turning one off stops that work and keeps what was already found. Turn it on again and it carries on.</p>
<p class="hint">While Pixie is working, the <b>Processing</b> button at the top right shows how far along it is, and lets you pause it.</p>
{#if noLocations}<p class="locations">{noLocations}</p>{/if}

<h3>What each one is for</h3>
<ul class="about">
  <li><b>Auto tags</b>: labels such as Beach, Museum or Dog, which you can browse by in the left-hand list.</li>
  <li><b>People</b>: finds the faces and groups them into people, so you can name them and browse by person.</li>
</ul>
<p class="hint">Pixie always looks for duplicate files, because Sync needs it to recognise copies. Extra copies are hidden from Browse, and Clean up &gt; Duplicates lists them so you can tidy them up.</p>

<style>
  .hint { color: #666; max-width: 60ch; }
  .locations { margin: 8px 0 12px; color: #555; font-size: 13px; }
  .about { color: #555; font-size: 13px; padding-left: 18px; max-width: 560px; }
  .about li { margin-bottom: 4px; }
</style>
