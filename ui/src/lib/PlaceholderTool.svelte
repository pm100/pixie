<script lang="ts">
  /** What a tool that is not built yet will look like: a short description and its options, all switched off. */
  import type { ToolDef } from './tools';

  let { tool }: { tool: ToolDef } = $props();
</script>

<div class="ph">
  <p class="soon">Not built yet</p>
  <p class="blurb">{tool.blurb}</p>
  <div class="mock" aria-hidden="true">
    {#each tool.mock ?? [] as m (m.label)}
      <div class="field"><span class="lbl">{m.label}</span><span class="box">{m.value}</span></div>
    {/each}
  </div>
  <button disabled>{tool.action}</button>
</div>

<style>
  .ph { padding: var(--page-pad); font-size: 14px; display: flex; flex-direction: column; align-items: flex-start; gap: 10px; color: #777; }
  .soon { margin: 0; font-size: 11px; letter-spacing: .05em; text-transform: uppercase; color: #8a6d00; background: #fff7d6; border-radius: 4px; padding: 2px 8px; }
  .blurb { margin: 0; color: #555; line-height: 1.45; }
  .mock { align-self: stretch; display: flex; flex-direction: column; gap: 8px; }
  .field { display: flex; flex-direction: column; gap: 3px; }
  .lbl { color: #888; }
  .box { border: 1px solid #d3d6df; border-radius: 4px; padding: 5px 8px; color: #999; background: #f7f7fa; }
  button { font: inherit; padding: 5px 16px; }
</style>
