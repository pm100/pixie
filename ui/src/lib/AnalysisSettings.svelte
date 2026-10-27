<script lang="ts">
  import type { AnalysisView } from './analysisView';

  let { view, note = null, onpause, onretry }: { view: AnalysisView; note?: string | null; onpause: (paused: boolean) => void; onretry: () => void } = $props();
</script>

<h2>Picture analysis</h2>
<p>Pixie studies your photos on this computer. Nothing leaves your computer except the one-time download of the
  picture-analysis and face models.</p>

{#if note}<p class="note">{note}</p>{/if}
<div class="overall">
  <div class="head"><b>{view.headline}</b></div>
  {#if view.fraction !== null}<progress value={view.fraction} max="1"></progress>{/if}
</div>
<dl class="counters">
  {#each view.counters as c (c.key)}
    <div class="counter"><dt>{c.label}</dt><dd>{c.text}</dd></div>
  {/each}
</dl>
{#each view.problems.filter((p) => p.kind === 'error') as p (p.key)}
  <p class="err" role="alert">{p.text}</p>
{/each}

<div class="row">
  {#if view.failed}
    <button onclick={onretry}>Try again</button>
  {:else if view.working || view.paused}
    <button onclick={() => onpause(!view.paused)}>{view.paused ? 'Resume' : 'Pause'}</button>
  {/if}
</div>

<h3>What each one is for</h3>
<ul class="about">
  <li><b>Auto tags</b>: labels such as Beach, Museum or Dog, which you can browse by in the left-hand list.</li>
  <li><b>People</b>: finds the faces and groups them into people, so you can name them and browse by person.</li>
  <li><b>Duplicates</b>: finds files that are in your folders more than once, so you can tidy them up on the Duplicates tab. Extra copies are hidden from Browse.</li>
</ul>

<style>
  .note { margin: 8px 0 0; color: #555; font-size: 13px; }
  .overall { margin: 8px 0 10px; max-width: 420px; }
  progress { display: block; width: 100%; margin: 4px 0 0; }
  .counters { margin: 0 0 12px; display: grid; gap: 4px; max-width: 420px; }
  .counter { display: flex; justify-content: space-between; gap: 12px; }
  .counter dt { font-weight: 600; }
  .counter dd { margin: 0; color: #555; }
  .row { display: flex; gap: 8px; margin-bottom: 14px; }
  .err { color: #b3261e; }
  .about { color: #555; font-size: 13px; padding-left: 18px; max-width: 560px; }
  .about li { margin-bottom: 4px; }
</style>
