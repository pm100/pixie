<script lang="ts">
  import FlowStage from './FlowStage.svelte';
  import { startLabel } from './loadFlow';
  import { countText, tickedPaths, type Row } from './suggestions';

  let { rows, ontoggle, onadd, onskip, onstart }: {
    rows: Row[]; ontoggle: (path: string) => void; onadd: () => void; onskip: () => void; onstart: () => void;
  } = $props();

  const found = $derived(rows.filter((r) => r.kind === 'known'));
  const further = $derived(rows.filter((r) => r.kind === 'further'));
  const start = $derived(startLabel(tickedPaths(rows).length));
</script>

{#snippet group(list: Row[])}
  <div class="list">
    {#each list as r (r.path)}
      <label class="row" class:inc={r.included_in !== null}>
        <input type="checkbox" checked={r.ticked} disabled={r.included_in !== null} onchange={() => ontoggle(r.path)} />
        <span class="what"><b>{r.name}</b><span class="path">{r.path}</span></span>
        <span class="n">{r.included_in !== null ? `included in ${r.included_in}` : (r.note ?? countText(r.count, r.kind))}</span>
      </label>
    {/each}
  </div>
{/snippet}

<FlowStage label="Welcome to Pixie">
  <h1>Welcome to Pixie</h1>
  <p class="sub">Pixie shows the photos and videos already on your computer. It never moves or changes them. Pick where to look:</p>

  {#if found.length > 0}
    <h2>We found these</h2>
    {@render group(found)}
  {:else}
    <p class="sub">Pixie did not find the usual photo folders. Add one below.</p>
  {/if}
  {#if further.length > 0}
    <h2>Look a little further</h2>
    {@render group(further)}
  {/if}

  <p><button onclick={onadd}>Add another folder…</button></p>
  <p class="note">You can add, remove or change these any time in <b>Settings → Photo folders</b>.</p>

  <div class="actions">
    <button onclick={onskip}>Skip for now</button>
    <button class="primary" disabled={start.disabled} onclick={onstart}>{start.label}</button>
  </div>
</FlowStage>

<style>
  h2 { margin: 14px 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; color: #777; }
  .list { border: 1px solid #d3d8e4; border-radius: 8px; overflow: hidden; background: #fff; }
  .row { display: flex; gap: 10px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #eef0f6; cursor: pointer; }
  .row:last-child { border-bottom: 0; }
  .row.inc { color: #888; cursor: default; }
  .what { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .path { color: #777; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #555; font-size: 13px; white-space: nowrap; }
  .note { color: #666; font-size: 13px; }
</style>
