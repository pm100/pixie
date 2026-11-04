<script lang="ts">
  /** The Collage settings pop-up: how to start a collage, the page, and how the photos look on it. It opens by itself the first time (with an
   *  empty page) and from the gear above the page. Changes apply at once; Done closes it. */
  import type { CollageTemplate } from './api';
  import { defaultDpi } from './collage';
  import { docDpi, pageMm, type CollageDoc } from './collageDoc';
  import { paperGroups } from './collagePapers';
  import { plural } from './format';

  let { doc, templates, locale, photoCount, onchange, onstart, ondone }: {
    doc: CollageDoc;
    templates: CollageTemplate[];
    locale: string;
    /** How many photos a template would arrange: the ones on the page, or all the Working-on photos while the page is empty. */
    photoCount: number;
    onchange: (d: CollageDoc) => void;
    onstart: (how: 'template' | 'blank', templateId: string | null) => void;
    ondone: () => void;
  } = $props();

  const groups = $derived(paperGroups(locale));
  const set = (patch: Partial<CollageDoc>) => onchange({ ...doc, ...patch });
  const size = $derived(pageMm(doc));
  const usual = $derived(defaultDpi(size.w, size.h));
  let choosing = $state(false);
  const num = (e: Event & { currentTarget: HTMLInputElement }, lo: number, hi: number, fallback: number) => {
    const v = e.currentTarget.valueAsNumber;
    return Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : fallback;
  };
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); ondone(); } }} />

<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) ondone(); }}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="cs-title">
    <h3 id="cs-title">Collage settings</h3>

    <section>
      <h4>Start from</h4>
      <p class="note">{photoCount === 0 ? 'There are no photos to use.' : `A template arranges ${plural(photoCount, 'photo')}: the ones on the page, or all the photos in the list while the page is empty. For other arrangements, add photos and press Layout above the page.`}</p>
      <div class="row">
        <button class:on={choosing} aria-pressed={choosing} disabled={photoCount === 0} onclick={() => (choosing = !choosing)} title="Choose one of the ready-made layouts to put the photos in">A template</button>
        <button onclick={() => onstart('blank', null)} title="Start with an empty page and add photos by hand">Blank page</button>
      </div>
      {#if choosing}
        <div class="templates" role="group" aria-label="Templates">
          {#each templates as t (t.id)}
            <button class="tpl" class:on={doc.template === t.id} aria-label={t.label} title={`${t.label}: put the photos in this layout`} onclick={() => onstart('template', t.id)}>
              <span class="mini">{#each t.cells as c, i (i)}<i style="left:{c.x * 100}%;top:{c.y * 100}%;width:{c.w * 100}%;height:{c.h * 100}%"></i>{/each}</span>
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <section>
      <h4>Page</h4>
      <label>Paper
        <select value={doc.paper} onchange={(e) => set({ paper: e.currentTarget.value })} title="The size of the page the collage is made for">
          {#each groups as g (g.label)}
            <optgroup label={g.label}>{#each g.sizes as p (p.id)}<option value={p.id}>{p.label}</option>{/each}</optgroup>
          {/each}
          <option value="custom">Custom size</option>
        </select>
      </label>
      {#if doc.paper === 'custom'}
        <label>Width (cm)
          <input type="number" min="1" max="100" step="0.1" value={doc.customW} onchange={(e) => set({ customW: num(e, 1, 100, doc.customW) })} title="The width of the page in centimetres, up to 100" />
        </label>
        <label>Height (cm)
          <input type="number" min="1" max="100" step="0.1" value={doc.customH} onchange={(e) => set({ customH: num(e, 1, 100, doc.customH) })} title="The height of the page in centimetres, up to 100" />
        </label>
      {/if}
      <label class="check"><input type="checkbox" checked={doc.landscape} onchange={(e) => set({ landscape: e.currentTarget.checked })} title="Make the page wide instead of tall" /> Landscape</label>
      <label>Resolution (dpi)
        <select value={docDpi(doc)} onchange={(e) => set({ dpi: Number(e.currentTarget.value) === usual ? null : Number(e.currentTarget.value) })} title="How many pixels are saved for each inch of the page: 300 for a print, 150 is enough for a large poster">
          <option value={150}>150</option><option value={200}>200</option><option value={300}>300</option>
        </select>
      </label>
      <label>Background
        <input type="color" value={doc.background} onchange={(e) => set({ background: e.currentTarget.value })} title="The colour of the page behind and between the photos" />
      </label>
    </section>

    <section>
      <h4>Photos</h4>
      <label>Gap (mm)
        <input type="number" min="0" max="50" step="0.5" value={doc.gap} onchange={(e) => set({ gap: num(e, 0, 50, doc.gap) })} title="The space left between photos when a template or Pixie lays them out, and the room kept next to a photo when snapping" />
      </label>
      <label>Corner rounding (mm)
        <input type="number" min="0" max="100" step="0.5" value={doc.corner} onchange={(e) => set({ corner: num(e, 0, 100, doc.corner) })} title="How round the corners of each photo are, in millimetres" />
      </label>
      <label>Border (mm)
        <input type="number" min="0" max="50" step="0.5" value={doc.border} onchange={(e) => set({ border: num(e, 0, 50, doc.border) })} title="The width of the border around each photo, in millimetres; 0 for none" />
      </label>
      <label>Border colour
        <input type="color" value={doc.borderColour} onchange={(e) => set({ borderColour: e.currentTarget.value })} title="The colour of the border around each photo" />
      </label>
      <label class="check"><input type="checkbox" checked={doc.shadow} onchange={(e) => set({ shadow: e.currentTarget.checked })} title="Give each photo a soft shadow, which helps when photos overlap" /> Shadow</label>
    </section>

    <section>
      <h4>Snapping</h4>
      <label class="check"><input type="checkbox" checked={doc.snapFill} onchange={(e) => set({ snapFill: e.currentTarget.checked })} title="When you move a photo into a free space, it grows or shrinks to fill that space, keeping its shape and crop" /> Fill the space a photo is moved into</label>
      <label class="check"><input type="checkbox" checked={doc.snapPage} onchange={(e) => set({ snapPage: e.currentTarget.checked })} title="Snap photos to the edges, margin and middle of the page while you move them" /> To the page edges and middle</label>
      <label class="check"><input type="checkbox" checked={doc.snapFrames} onchange={(e) => set({ snapFrames: e.currentTarget.checked })} title="Snap photos to the edges and middles of other photos, and to equal gaps, while you move them" /> To other photos</label>
    </section>

    <div class="done"><button class="primary" onclick={ondone} title="Close the settings">Done</button></div>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 1500; display: grid; place-items: center; background: rgba(0, 0, 0, .4); }
  .dlg { width: min(520px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow-y: auto; padding: 18px 20px; border-radius: 8px; background: #fff; color: #111; box-shadow: 0 8px 30px rgba(0, 0, 0, .35); font-size: 14px; display: flex; flex-direction: column; gap: 4px; }
  h3 { margin: 0 0 6px; font-size: 16px; }
  h4 { margin: 10px 0 4px; font-size: 13px; color: #555; text-transform: uppercase; letter-spacing: .03em; }
  section { display: flex; flex-direction: column; gap: 6px; }
  .note { margin: 0; color: #777; font-size: 13px; }
  .row { display: flex; gap: 6px; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 2px; }
  label.check { flex-direction: row; align-items: center; gap: 6px; }
  select, input[type='number'] { padding: 3px 6px; }
  input[type='color'] { width: 48px; height: 28px; padding: 0; }
  button.on { border-color: #2f6fed; box-shadow: 0 0 0 1px #2f6fed; }
  .templates { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 4px; }
  .tpl { padding: 3px; line-height: 0; }
  .mini { position: relative; display: block; width: 28px; height: 36px; background: #fff; }
  .mini i { position: absolute; box-sizing: border-box; border: 1px solid #fff; background: #7a8398; }
  .done { display: flex; justify-content: flex-end; margin-top: 12px; }
</style>
