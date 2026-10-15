<script lang="ts">
  import { printPage } from './api';
  let error = $state('');
  async function go() {
    error = '';
    try { await printPage(); } catch (e) { error = String(e); }
  }
</script>

<div class="controls">
  <button onclick={go}>Print 4×6 test sheet</button>
  <span>Print to PDF and to a real printer at "100% / actual size", then measure the grid squares.</span>
  {#if error}<span class="err">{error}</span>{/if}
</div>

<div class="sheet">
  <div class="frame"></div>
  <div class="label tl">TL</div><div class="label tr">TR</div>
  <div class="label bl">BL</div><div class="label br">BR</div>
  <div class="note">Each grid square is 1in × 1in.<br />Frame is inset 0.25in from every edge.</div>
</div>

<style>
  .controls { display: flex; gap: 12px; align-items: center; padding: 8px 0; }
  .err { color: #b00020; }
  .sheet {
    position: relative; width: 4in; height: 6in; background-color: #fff; color: #000;
    background-image:
      linear-gradient(to right, #7aa7ff 1px, transparent 1px),
      linear-gradient(to bottom, #7aa7ff 1px, transparent 1px);
    background-size: 1in 1in;
    outline: 1px solid #aaa; box-sizing: border-box;
  }
  .frame { position: absolute; inset: 0.25in; border: 2px solid #e00; box-sizing: border-box; }
  .label { position: absolute; font: 700 12px monospace; }
  .tl { top: .3in; left: .3in; } .tr { top: .3in; right: .3in; }
  .bl { bottom: .3in; left: .3in; } .br { bottom: .3in; right: .3in; }
  .note { position: absolute; top: 2.4in; left: .4in; font: 12px system-ui; }

  @media print {
    @page { size: 4in 6in; margin: 0; }
    :global(header), .controls { display: none !important; }
    :global(body), :global(main), :global(section) { margin: 0 !important; padding: 0 !important; height: auto !important; }
    .sheet { outline: none; print-color-adjust: exact; -webkit-print-color-adjust: exact; }
  }
</style>
