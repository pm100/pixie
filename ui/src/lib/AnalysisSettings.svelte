<script lang="ts">
  import { analysisFraction, analysisLine, type AnalysisStatus } from './analysis';
  import { peopleLine } from './people';

  let { status, people, onpause, onretry }: { status: AnalysisStatus | null; people: AnalysisStatus | null; onpause: (paused: boolean) => void; onretry: () => void } = $props();
  const fraction = $derived(status ? analysisFraction(status) : null);
  const peopleFraction = $derived(people ? analysisFraction(people) : null);
  const unfinished = (s: AnalysisStatus | null) => s !== null && s.models === 'ready' && s.total > 0 && s.done < s.total;
  const working = $derived(unfinished(status) || unfinished(people));
  const paused = $derived((status?.paused ?? false) || (people?.paused ?? false));
  const failed = $derived(status?.models === 'error' || people?.models === 'error');
</script>

<h2>Picture analysis</h2>
<p>Pixie studies your photos on this computer. It adds <b>Auto tags</b>, labels such as Beach, Museum or Dog that you can
  browse by in the left-hand list. Nothing leaves your computer except the one-time download of the picture-analysis model.</p>
{#if status === null}
  <p>Checking…</p>
{:else}
  <p class:err={status.models === 'error'}>{analysisLine(status)}</p>
  {#if fraction !== null}<progress value={fraction} max="1"></progress>{/if}
  {#if people}
    <h3>People</h3>
    <p>Pixie also looks for faces and groups them into people. The face models are a one-time download too.</p>
    <p class:err={people.models === 'error'}>{peopleLine(people)}</p>
    {#if peopleFraction !== null}<progress value={peopleFraction} max="1"></progress>{/if}
  {/if}
  <div class="row">
    {#if failed}
      <button onclick={onretry}>Try again</button>
    {:else if working || paused}
      <button onclick={() => onpause(!paused)}>{paused ? 'Resume' : 'Pause'}</button>
    {/if}
  </div>
{/if}

<style>
  progress { display: block; width: 320px; max-width: 100%; margin: 4px 0 10px; }
  .row { display: flex; gap: 8px; }
  .err { color: #b3261e; }
</style>
