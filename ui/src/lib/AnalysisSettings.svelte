<script lang="ts">
  import { analysisFraction, analysisLine, type AnalysisStatus } from './analysis';

  let { status, onpause, onretry }: { status: AnalysisStatus | null; onpause: (paused: boolean) => void; onretry: () => void } = $props();
  const fraction = $derived(status ? analysisFraction(status) : null);
  const working = $derived(status !== null && status.models === 'ready' && status.total > 0 && status.done < status.total);
</script>

<h2>Search</h2>
<p>Pixie studies your photos on this computer so you can search them by what they show, such as “dog on a beach”.
  Nothing leaves your computer except the one-time download of the search model.</p>
{#if status === null}
  <p>Checking…</p>
{:else}
  <p class:err={status.models === 'error'}>{analysisLine(status)}</p>
  {#if fraction !== null}<progress value={fraction} max="1"></progress>{/if}
  <div class="row">
    {#if status.models === 'error'}
      <button onclick={onretry}>Try again</button>
    {:else if working || status.paused}
      <button onclick={() => onpause(!status!.paused)}>{status.paused ? 'Resume' : 'Pause'}</button>
    {/if}
  </div>
{/if}

<style>
  progress { display: block; width: 320px; max-width: 100%; margin: 4px 0 10px; }
  .row { display: flex; gap: 8px; }
  .err { color: #b3261e; }
</style>
