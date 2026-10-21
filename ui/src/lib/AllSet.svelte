<script lang="ts">
  import FlowStage from './FlowStage.svelte';
  import type { allSetSummary } from './loadFlow';

  let { summary, onpeople, onbrowse, onadd }: {
    summary: ReturnType<typeof allSetSummary>; onpeople: () => void; onbrowse: () => void; onadd: () => void;
  } = $props();

  const nothing = $derived(summary.rows.length === 0);
</script>

<FlowStage label={summary.title}>
  <h1>{summary.title}</h1>
  <p class="sub">{summary.detail}</p>
  {#if summary.problems.length > 0}
    <div class="problems" role="alert">
      <b>{summary.problems.length === 1 ? 'One folder could not be read:' : 'Some folders could not be read:'}</b>
      {#each summary.problems as p (p)}<div>{p}</div>{/each}
    </div>
  {/if}

  {#if !nothing}
    <div class="list">
      {#each summary.rows as r (r.name)}
        <div class="row"><span>{r.name}</span><span class="n">{r.text}</span></div>
      {/each}
    </div>
    {#if summary.showPeopleBox}
      <div class="people"><b>Faces found.</b> Pixie spotted people in your photos. Open the <b>People</b> tab to give them names, and then you can browse by person.</div>
    {/if}
    <p class="note">Pixie now carries on working quietly in the background (search, tags and faces). You can use it straight away.</p>
  {/if}

  <div class="actions">
    {#if nothing}
      <button onclick={onbrowse}>Not now</button>
      <button class="primary" onclick={onadd}>Add another folder…</button>
    {:else}
      {#if summary.showPeopleBox}<button onclick={onpeople}>Open People</button>{/if}
      <button class="primary" onclick={onbrowse}>Start browsing</button>
    {/if}
  </div>
</FlowStage>

<style>
  .list { border: 1px solid #d3d8e4; border-radius: 8px; overflow: hidden; background: #fff; margin-bottom: 14px; }
  .row { display: flex; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #eef0f6; }
  .row:last-child { border-bottom: 0; }
  .n { color: #555; }
  .people { background: #e3ecff; border-radius: 8px; padding: 10px 12px; margin-bottom: 14px; color: #1a3f9a; }
  .problems { background: #fdecea; border-radius: 8px; padding: 10px 12px; margin-bottom: 14px; color: #b00020; font-size: 13px; }
  .note { color: #666; font-size: 13px; }
</style>
