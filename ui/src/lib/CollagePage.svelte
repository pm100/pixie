<script lang="ts">
  /** The page of the Collage tool, drawn to scale in the middle of the screen. Frames can be selected, moved, resized, cropped and given a
   *  photo from the list; snap lines show while a frame is moved. The component only draws and forwards what the pointer does: the rules are in
   *  collageFrames.ts and collageSnap.ts, and every change goes out through `onchange` (the parent keeps the collage). */
  import type { Photo } from './api';
  import { focusFromDrag, frameDpi, LOW_DPI, overflow } from './collage';
  import { pageMm, type CollageDoc, type CollageFrame } from './collageDoc';
  import { fillSpace } from './collageFill';
  import { moveRect, resizeRect, swapPhotos, topFrameAt, type Handle, type Rect } from './collageFrames';
  import { snapMove, snapResize, type Guide, type SnapContext } from './collageSnap';
  import { thumbUrl } from './url';

  let { doc, photos, selected, active, onselect, onchange, ondrop }: {
    doc: CollageDoc;
    /** The photos the frames show, by id (a photo that is not here is missing). */
    photos: Map<number, Photo>;
    selected: string[];
    /** The tool is on screen (the keys act only then). */
    active: boolean;
    onselect: (ids: string[]) => void;
    onchange: (frames: CollageFrame[]) => void;
    /** A photo from the list was dropped at a place on the page (fractions), on this frame if there is one. `replace`: Ctrl was held, so
     *  it takes that frame's place instead of being added on top of it. */
    ondrop: (photoId: number, at: { x: number; y: number }, onFrame: string | null, replace: boolean) => void;
  } = $props();

  const PAD = 28;
  const HANDLES: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];
  const HANDLE_AT: Record<Handle, [number, number]> = { nw: [0, 0], n: [0.5, 0], ne: [1, 0], e: [1, 0.5], se: [1, 1], s: [0.5, 1], sw: [0, 1], w: [0, 0.5] };
  const HANDLE_CURSOR: Record<Handle, string> = { nw: 'nwse-resize', se: 'nwse-resize', ne: 'nesw-resize', sw: 'nesw-resize', n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', w: 'ew-resize' };

  let boxW = $state(0);
  let boxH = $state(0);
  let pageEl: HTMLDivElement;

  const mm = $derived(pageMm(doc));
  const aspect = $derived(mm.w / mm.h);
  const pageW = $derived(Math.max(60, Math.min(boxW - 2 * PAD, (boxH - 2 * PAD) * aspect)));
  const pageH = $derived(pageW / aspect);
  const pxPerMm = $derived(pageW / mm.w);
  const shadowPx = $derived(0.015 * Math.min(pageW, pageH));

  /** The frame being worked on in crop mode. */
  let cropId = $state<string | null>(null);
  let guides = $state<Guide[]>([]);
  /** Where the frame being moved or resized will settle when it is let go (shown as an outline), if that is not where it is. */
  let landing = $state<Rect | null>(null);
  let snapTo: { id: string; rect: Rect } | null = null;
  const same = (a: Rect, b: Rect) => Math.abs(a.x - b.x) + Math.abs(a.y - b.y) + Math.abs(a.w - b.w) + Math.abs(a.h - b.h) < 1e-6;

  type Drag =
    | { kind: 'move'; x: number; y: number; ids: string[]; starts: Map<string, Rect> }
    | { kind: 'resize'; x: number; y: number; id: string; handle: Handle; start: Rect }
    | { kind: 'pan'; x: number; y: number; id: string; fx: number; fy: number };
  let drag: Drag | null = null;

  const frameById = (id: string) => doc.frames.find((f) => f.id === id);
  const photoOf = (f: CollageFrame) => { const p = photos.get(f.photo); return p && !p.missing ? p : null; };
  const sizeOf = (f: Rect) => ({ w: f.w * pageW, h: f.h * pageH });
  const bandOf = (f: Rect) => { const s = sizeOf(f); return Math.min(doc.border * pxPerMm, (Math.min(s.w, s.h) - 1) / 2); };
  const radiusOf = (f: Rect) => { const s = sizeOf(f); return Math.min(doc.corner * pxPerMm, Math.min(s.w, s.h) / 2); };

  /** Where the photo sits inside its frame, so that what shows is what the saved picture shows (the same crop rule as the app's). */
  function photoStyle(f: CollageFrame, p: Photo): string {
    const s = sizeOf(f);
    const band = bandOf(f);
    const [iw, ih] = [Math.max(1, s.w - 2 * band), Math.max(1, s.h - 2 * band)];
    if (!p.width || !p.height) return `left:0;top:0;width:100%;height:100%;object-fit:cover;object-position:${f.fx * 100}% ${f.fy * 100}%`;
    const scale = Math.max(iw / p.width, ih / p.height) * f.zoom;
    const [w, h] = [p.width * scale, p.height * scale];
    return `left:${-(w - iw) * f.fx}px;top:${-(h - ih) * f.fy}px;width:${w}px;height:${h}px`;
  }

  const lowDpi = (f: CollageFrame, p: Photo): number | null => (p.width && p.height ? frameDpi(p.width, p.height, f, mm) : null);

  function toFrac(e: { clientX: number; clientY: number }): { x: number; y: number } {
    const r = pageEl.getBoundingClientRect();
    return { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height };
  }

  function snapContext(skip: string[]): SnapContext {
    return { others: doc.frames.filter((f) => !skip.includes(f.id)), pageMm: mm, marginMm: doc.gap / 2, page: doc.snap && doc.snapPage, frames: doc.snap && doc.snapFrames, thresholdPx: 6, pagePx: { w: pageW, h: pageH } };
  }

  const replace = (id: string, patch: Partial<CollageFrame>) => onchange(doc.frames.map((f) => (f.id === id ? { ...f, ...patch } : f)));

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    const at = toFrac(e);
    const hit = topFrameAt(doc.frames, at.x, at.y);
    if (!hit) { onselect([]); cropId = null; return; }
    pageEl.setPointerCapture(e.pointerId);
    if (cropId === hit.id) { drag = { kind: 'pan', x: e.clientX, y: e.clientY, id: hit.id, fx: hit.fx, fy: hit.fy }; return; }
    cropId = null;
    let ids = selected;
    if (e.shiftKey) {
      ids = selected.includes(hit.id) ? selected.filter((i) => i !== hit.id) : [...selected, hit.id];
      onselect(ids);
      if (!ids.includes(hit.id)) return;
    } else if (!selected.includes(hit.id)) {
      ids = [hit.id];
      onselect(ids);
    }
    drag = { kind: 'move', x: e.clientX, y: e.clientY, ids, starts: new Map(ids.map((i) => [i, { ...frameById(i)! }])) };
  }

  function downHandle(e: PointerEvent, handle: Handle, f: CollageFrame) {
    if (e.button !== 0) return;
    e.stopPropagation();
    pageEl.setPointerCapture(e.pointerId);
    drag = { kind: 'resize', x: e.clientX, y: e.clientY, id: f.id, handle, start: { x: f.x, y: f.y, w: f.w, h: f.h } };
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const [dx, dy] = [(e.clientX - drag.x) / pageW, (e.clientY - drag.y) / pageH];
    const snapping = !e.altKey && doc.snap && (doc.snapPage || doc.snapFrames);
    if (drag.kind === 'move') {
      const starts = [...drag.starts.values()];
      if (starts.length === 1) {
        // The photo follows the pointer; where it will settle once let go is only shown, and happens on release (see `up`).
        const r = moveRect(starts[0], dx, dy);
        const moving = drag.ids[0];
        let target = r;
        guides = [];
        if (!e.altKey) {
          // A photo moved into a free space fills it, keeping its shape (and so its crop); over another photo it just moves, and lines up.
          const filled = doc.snap && doc.snapFill ? fillSpace(r, doc.frames.filter((f) => f.id !== moving), mm, doc.gap, { x: r.x + r.w / 2, y: r.y + r.h / 2 }) : null;
          if (filled) target = filled;
          else if (snapping) ({ rect: target, guides } = snapMove(r, snapContext(drag.ids)));
        }
        snapTo = { id: moving, rect: target };
        landing = same(target, r) ? null : target;
        replace(moving, { x: r.x, y: r.y, w: r.w, h: r.h });
      } else {
        // a group moves as one, kept on the page
        const gx = Math.min(Math.max(dx, -Math.min(...starts.map((s) => s.x))), 1 - Math.max(...starts.map((s) => s.x + s.w)));
        const gy = Math.min(Math.max(dy, -Math.min(...starts.map((s) => s.y))), 1 - Math.max(...starts.map((s) => s.y + s.h)));
        const ids = drag.ids, from = drag.starts;
        onchange(doc.frames.map((f) => (ids.includes(f.id) ? { ...f, x: from.get(f.id)!.x + gx, y: from.get(f.id)!.y + gy } : f)));
      }
    } else if (drag.kind === 'resize') {
      const r = resizeRect(drag.start, drag.handle, dx, dy, aspect, e.shiftKey);
      let target = r;
      guides = [];
      if (snapping) ({ rect: target, guides } = snapResize(r, drag.handle, snapContext([drag.id])));
      snapTo = { id: drag.id, rect: target };
      landing = same(target, r) ? null : target;
      replace(drag.id, r);
    } else {
      const f = frameById(drag.id);
      const p = f && photoOf(f);
      if (!f || !p?.width || !p.height) return;
      const s = sizeOf(f), band = bandOf(f);
      const next = focusFromDrag({ fx: drag.fx, fy: drag.fy }, { dx: e.clientX - drag.x, dy: e.clientY - drag.y }, overflow(p.width, p.height, s.w - 2 * band, s.h - 2 * band, f.zoom));
      replace(drag.id, next);
    }
  }

  function up(e: PointerEvent) {
    const d = drag;
    const settle = snapTo;
    drag = null;
    snapTo = null;
    guides = [];
    landing = null;
    // Ctrl held when a frame is let go over another: the two exchange photos (without it, frames just overlap)
    if (d?.kind === 'move' && d.ids.length === 1 && e.ctrlKey) {
      const at = toFrac(e);
      const under = topFrameAt(doc.frames.filter((f) => f.id !== d.ids[0]), at.x, at.y);
      if (under) {
        const start = d.starts.get(d.ids[0])!;
        onchange(swapPhotos(doc.frames, d.ids[0], under.id).map((f) => (f.id === d.ids[0] ? { ...f, ...start } : f)));
        return;
      }
    }
    // Let go: the frame settles where the outline showed.
    if (settle && (d?.kind === 'move' || d?.kind === 'resize')) replace(settle.id, settle.rect);
  }

  function doubleClick(e: MouseEvent) {
    const at = toFrac(e);
    const hit = topFrameAt(doc.frames, at.x, at.y);
    if (!hit || !photoOf(hit)) return;
    cropId = hit.id;
    onselect([hit.id]);
  }

  /** The wheel zooms the photo in crop mode. It needs a listener that may cancel the scroll, which Svelte's own `onwheel` is not. */
  function wheelZoom(node: HTMLElement) {
    const on = (e: WheelEvent) => {
      const f = cropId ? frameById(cropId) : undefined;
      if (!f) return;
      e.preventDefault();
      replace(f.id, { zoom: Math.min(4, Math.max(1, f.zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1))) });
    };
    node.addEventListener('wheel', on, { passive: false });
    return { destroy: () => node.removeEventListener('wheel', on) };
  }

  function dragOver(e: DragEvent) {
    if (e.dataTransfer?.types.includes('application/x-pixie-photo')) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; }
  }

  function dropped(e: DragEvent) {
    const id = Number(e.dataTransfer?.getData('application/x-pixie-photo'));
    if (!Number.isFinite(id) || !e.dataTransfer?.getData('application/x-pixie-photo')) return;
    e.preventDefault();
    const at = toFrac(e);
    ondrop(id, at, topFrameAt(doc.frames, at.x, at.y)?.id ?? null, e.ctrlKey || e.metaKey);
  }

  // Leaving crop mode: the key, or the frame going away.
  $effect(() => { if (cropId && !frameById(cropId)) cropId = null; });
</script>

<svelte:window onkeydown={(e) => { if (active && e.key === 'Escape' && cropId) { cropId = null; e.stopPropagation(); } }} />

<div class="surround" bind:clientWidth={boxW} bind:clientHeight={boxH}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="page" bind:this={pageEl} use:wheelZoom style="width:{pageW}px;height:{pageH}px;background:{doc.background}" role="application" aria-label="The collage page"
       title="Drag a photo to move it, or a handle to resize it. Double-click a photo to crop it inside its frame."
       onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} ondblclick={doubleClick} ondragover={dragOver} ondrop={dropped}>
    {#each doc.frames as f (f.id)}
      {@const p = photoOf(f)}
      {@const band = bandOf(f)}
      <div class="frame" style="left:{f.x * pageW}px;top:{f.y * pageH}px;width:{f.w * pageW}px;height:{f.h * pageH}px;border-radius:{radiusOf(f)}px;padding:{band}px;background:{doc.borderColour};{doc.shadow ? `box-shadow:${shadowPx}px ${shadowPx}px ${2 * shadowPx}px rgba(0,0,0,.35)` : ''}">
        <div class="inner">
          {#if p}
            <img alt="" draggable="false" src={thumbUrl('view', p.id, p.mtime, p.size)} style={photoStyle(f, p)} />
          {:else}
            <div class="gone">Photo missing</div>
          {/if}
        </div>
        {#if p && (lowDpi(f, p) ?? LOW_DPI) < LOW_DPI}
          <span class="low" title="This photo has few pixels for its frame: it will print at about {Math.round(lowDpi(f, p) ?? 0)} dots per inch, so it may look soft. A smaller page or frame would look sharper.">Low resolution</span>
        {/if}
      </div>
    {/each}

    {#each selected as id (id)}
      {@const f = frameById(id)}
      {#if f}
        <div class="sel" class:crop={cropId === id} style="left:{f.x * pageW}px;top:{f.y * pageH}px;width:{f.w * pageW}px;height:{f.h * pageH}px;border-radius:{radiusOf(f)}px">
          {#if cropId === id}
            <span class="hint">Drag to move the photo, scroll to zoom, Esc to finish</span>
          {:else if selected.length === 1}
            {#each HANDLES as h (h)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <i class="handle" aria-hidden="true" style="left:{HANDLE_AT[h][0] * 100}%;top:{HANDLE_AT[h][1] * 100}%;cursor:{HANDLE_CURSOR[h]}" onpointerdown={(e) => downHandle(e, h, f)}></i>
            {/each}
          {/if}
        </div>
      {/if}
    {/each}

    {#if landing}
      <div class="landing" style="left:{landing.x * pageW}px;top:{landing.y * pageH}px;width:{landing.w * pageW}px;height:{landing.h * pageH}px;border-radius:{radiusOf(landing)}px"></div>
    {/if}

    {#each guides as g, i (i)}
      <div class="guide" class:v={g.axis === 'x'} style={g.axis === 'x' ? `left:${g.at * pageW}px;top:${g.from * pageH}px;height:${(g.to - g.from) * pageH}px` : `top:${g.at * pageH}px;left:${g.from * pageW}px;width:${(g.to - g.from) * pageW}px`}>
        {#if g.gapMm !== undefined}<span>{Math.round(g.gapMm * 10) / 10} mm</span>{/if}
      </div>
    {/each}

    {#if doc.frames.length === 0}
      <p class="empty">Drag photos here from the list, or choose Start with selected.</p>
    {/if}
  </div>
</div>

<style>
  .surround { flex: 1; min-height: 0; min-width: 0; display: grid; place-items: center; background: #e9ecf3; overflow: hidden; }
  .page { position: relative; flex: none; box-shadow: 0 2px 12px rgba(0, 0, 0, .3); touch-action: none; user-select: none; overflow: visible; }
  .frame { position: absolute; box-sizing: border-box; overflow: hidden; }
  .inner { position: relative; width: 100%; height: 100%; overflow: hidden; }
  .inner img { position: absolute; max-width: none; pointer-events: none; user-select: none; }
  .gone { display: grid; place-items: center; width: 100%; height: 100%; background: #d6d9e0; color: #666; font-size: 12px; text-align: center; }
  .low { position: absolute; left: 4px; bottom: 4px; background: rgba(176, 0, 32, .85); color: #fff; font-size: 11px; padding: 1px 5px; border-radius: 3px; pointer-events: none; }
  .sel { position: absolute; box-sizing: border-box; outline: 2px solid #2f6fed; outline-offset: -1px; pointer-events: none; }
  .sel.crop { outline-style: dashed; }
  .hint { position: absolute; left: 4px; top: 4px; background: rgba(0, 0, 0, .65); color: #fff; font-size: 11px; padding: 1px 6px; border-radius: 3px; white-space: nowrap; }
  .handle { position: absolute; width: 10px; height: 10px; margin: -5px 0 0 -5px; box-sizing: border-box; background: #fff; border: 2px solid #2f6fed; pointer-events: auto; }
  .landing { position: absolute; box-sizing: border-box; border: 2px dashed #2f6fed; background: rgba(47, 111, 237, .12); pointer-events: none; }
  .guide { position: absolute; pointer-events: none; border: 0 dashed #e0245e; }
  .guide.v { width: 0; border-left-width: 1px; }
  .guide:not(.v) { height: 0; border-top-width: 1px; }
  .guide span { position: absolute; left: 3px; top: 3px; font-size: 10px; color: #e0245e; background: #fff; padding: 0 3px; white-space: nowrap; }
  .empty { position: absolute; inset: 0; display: grid; place-items: center; margin: 0; padding: 0 20px; color: #999; font-size: 13px; text-align: center; pointer-events: none; }
</style>
