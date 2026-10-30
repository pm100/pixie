<script lang="ts">
  /** The Print tool: choose paper and layout, see the first page, then print the photos shown in Tools. Printing opens the
   *  system's print dialog (the printer and the number of copies are chosen there). */
  import { tick } from 'svelte';
  import { printPage, type Photo } from './api';
  import { plural } from './format';
  import { grid, PAPERS, PER_PAGE, pageSize, sheets, type Paper, type PerPage } from './print';
  import PrintSheet from './PrintSheet.svelte';

  let { photos }: { photos: Photo[] } = $props();

  let paper = $state<Paper>('a4');
  let landscape = $state(false);
  let perPage = $state<PerPage>(1);
  let fit = $state<'fill' | 'whole'>('fill');
  let margin = $state(10);
  let captions = $state(false);
  let printing = $state(false);
  let error = $state('');

  const size = $derived(pageSize(paper, landscape));
  const cells = $derived(grid(perPage, landscape));
  const margins = $derived(Math.min(30, Math.max(0, Number.isFinite(margin) ? margin : 0)));
  const byId = $derived(new Map(photos.map((p) => [p.id, p])));
  const pages = $derived(sheets(photos.map((p) => p.id), perPage).map((ids) => ids.map((id) => byId.get(id)!)));
  /** The preview is drawn at the real size and shrunk to this many pixels wide (96 pixels to an inch). */
  const PREVIEW_PX = 260;
  const zoom = $derived(PREVIEW_PX / (size.w * 3.7795));

  /** Moves the printed pages to the end of <body>, so the rest of the app can be hidden while printing. */
  const toBody = (node: HTMLElement) => { document.body.appendChild(node); return { destroy: () => node.remove() }; };

  async function print() {
    if (printing || pages.length === 0) return;
    error = '';
    printing = true;
    await tick();
    try {
      // Wait for the pictures: a page printed before they have loaded comes out empty.
      const imgs = Array.from(document.querySelectorAll<HTMLImageElement>('.printout img'));
      await Promise.all(imgs.map((i) => i.decode().catch(() => {})));
      await printPage();
    } catch (e) {
      error = String(e);
    }
    printing = false;
  }
</script>

<svelte:head>{#if printing}{@html `<style>@page { size: ${size.w}mm ${size.h}mm; margin: 0 }</style>`}{/if}</svelte:head>

<div class="print">
  <p class="hint">{plural(photos.length, 'photo')} shown here.</p>
  <label>Paper
    <select bind:value={paper} title="The size of the paper">{#each Object.entries(PAPERS) as [id, p] (id)}<option value={id}>{p.label}</option>{/each}</select>
  </label>
  <label>Orientation
    <select bind:value={landscape} title="Whether the paper is tall or wide"><option value={false}>Portrait</option><option value={true}>Landscape</option></select>
  </label>
  <label>Photos per page
    <select bind:value={perPage} title="How many photos go on each page">{#each PER_PAGE as n (n)}<option value={n}>{n}</option>{/each}</select>
  </label>
  <label>Fit
    <select bind:value={fit} title="Fill each space by cropping the photo, or show all of it"><option value="fill">Fill the space (crop)</option><option value="whole">Show the whole photo</option></select>
  </label>
  <label>Margin (mm)
    <input type="number" min="0" max="30" bind:value={margin} title="The space around the edge of each page, in millimetres" />
  </label>
  <label class="check"><input type="checkbox" bind:checked={captions} /> Print the file name and date</label>

  {#if pages.length > 0}
    <div class="preview" style="width:{PREVIEW_PX}px;height:{size.h * 3.7795 * zoom}px" aria-label="Preview of the first page">
      <div style="zoom:{zoom}"><PrintSheet photos={pages[0]} w={size.w} h={size.h} cols={cells.cols} rows={cells.rows} margin={margins} {fit} {captions} /></div>
    </div>
    <p class="hint">{plural(pages.length, 'page')}. The first is shown above.</p>
  {/if}
  <button class="primary go" title="Open the system print dialog, where you choose the printer" onclick={print} disabled={printing || pages.length === 0}>{printing ? 'Printing…' : 'Print…'}</button>
  {#if error}<span class="err" role="alert">{error}</span>{/if}
</div>

{#if printing}
  <div class="printout" use:toBody>
    {#each pages as page, i (i)}
      <PrintSheet photos={page} w={size.w} h={size.h} cols={cells.cols} rows={cells.rows} margin={margins} {fit} {captions} />
    {/each}
  </div>
{/if}

<style>
  .print { padding: 12px 14px; font-size: 14px; display: flex; flex-direction: column; align-items: flex-start; gap: 8px; }
  .hint { margin: 0; color: #777; font-size: 13px; }
  label { display: flex; flex-direction: column; gap: 2px; align-self: stretch; }
  label.check { flex-direction: row; align-items: center; gap: 6px; }
  select, input[type='number'] { padding: 3px 6px; }
  .preview { overflow: hidden; border: 1px solid #c9ceda; box-shadow: 0 1px 4px rgba(0, 0, 0, .15); background: #fff; }
  .go { padding: 5px 16px; }
  .err { color: #b00020; font-size: 13px; }
  /* The printed pages sit outside the app; on screen they are not shown, and when printing they are all that is. */
  .printout { display: none; }
  @media print {
    :global(body > *:not(.printout)) { display: none !important; }
    .printout { display: block; }
  }
</style>
