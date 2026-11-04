<script lang="ts">
  /** A colour choice: a button showing the colour that opens a small pop-up with ready-made colours and, below them, the continuous palette
   *  (a square for shade and brightness, a slider for the hue, and the colour's code). */
  import { hexToHsv, hsvToHex, normaliseHex, type Hsv } from './colour';

  let { value, onchange, label, disabled = false }: {
    value: string;
    onchange: (hex: string) => void;
    /** What the colour is for, e.g. "Border colour": names the controls for screen readers and in the tooltips. */
    label: string;
    disabled?: boolean;
  } = $props();

  const COLOURS: { hex: string; name: string }[] = [
    ['#ffffff', 'White'], ['#f3ead8', 'Cream'], ['#d9d9d9', 'Light grey'], ['#9a9a9a', 'Grey'], ['#4a4a4a', 'Dark grey'], ['#000000', 'Black'],
    ['#f8c9c4', 'Light red'], ['#fbe0b8', 'Light orange'], ['#fcf0a6', 'Light yellow'], ['#c3e6cb', 'Light green'], ['#bcd3fb', 'Light blue'], ['#d9c3e8', 'Light purple'],
    ['#d93025', 'Red'], ['#f28c28', 'Orange'], ['#f6c90e', 'Yellow'], ['#2e9d4f', 'Green'], ['#2f6fed', 'Blue'], ['#8e44ad', 'Purple'],
    ['#8c1d14', 'Dark red'], ['#a45a0c', 'Brown'], ['#8a7300', 'Olive'], ['#145c2d', 'Dark green'], ['#173f99', 'Navy'], ['#e86aa6', 'Pink'],
  ].map(([hex, name]) => ({ hex, name }));
  const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

  let open = $state(false);
  let hsv = $state<Hsv>({ h: 0, s: 0, v: 0 });
  let typed = $state('');
  let root = $state<HTMLSpanElement | null>(null);
  let btn = $state<HTMLButtonElement | null>(null);
  /** Where the pop-up floats (fixed, so it sits over whatever is behind and never grows its parent). */
  let at = $state({ left: 0, top: 0 });
  const POP_W = 196;
  const POP_H = 250;

  function toggle() {
    open = !open;
    if (open) {
      take(value);
      const r = btn?.getBoundingClientRect();
      if (r) {
        const below = r.bottom + 4;
        at = {
          left: Math.max(8, Math.min(r.left, window.innerWidth - POP_W - 8)),
          top: below + POP_H > window.innerHeight - 8 ? Math.max(8, r.top - 4 - POP_H) : below,
        };
      }
    }
  }
  /** The palette and the code follow `hex` (the colour in use, or one just chosen). */
  function take(hex: string) {
    const h = hexToHsv(hex);
    // (a grey has no hue of its own: keep the slider where it was)
    hsv = { h: h.s === 0 ? hsv.h : h.h, s: h.s, v: h.v };
    typed = hex;
  }
  function choose(hex: string) {
    take(hex);
    onchange(hex);
  }
  function mix(next: Hsv) {
    hsv = next;
    const hex = hsvToHex(next);
    typed = hex;
    onchange(hex);
  }

  let dragging = false;
  function pointAt(e: PointerEvent & { currentTarget: HTMLElement }) {
    const r = e.currentTarget.getBoundingClientRect();
    const s = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const v = 1 - Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    mix({ ...hsv, s, v });
  }
  function squareKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.1 : 0.02;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    const m = d[e.key];
    if (!m) return;
    e.preventDefault();
    mix({ ...hsv, s: Math.min(1, Math.max(0, hsv.s + m[0])), v: Math.min(1, Math.max(0, hsv.v + m[1])) });
  }
  function typedHex() {
    const hex = normaliseHex(typed);
    if (hex) choose(hex);
    else typed = value;
  }
</script>

<svelte:window
  onpointerdown={(e) => { if (open && root && !root.contains(e.target as Node)) open = false; }}
  onkeydown={(e) => { if (open && e.key === 'Escape') { e.stopPropagation(); open = false; } }}
/>

<span class="picker" bind:this={root}>
  <button class="cur" bind:this={btn} style="background:{value}" {disabled} aria-expanded={open} aria-label={`${label}: ${value}`} title={`${label}: choose a colour`} onclick={toggle}></button>
  {#if open}
    <div class="pop" style="left:{at.left}px;top:{at.top}px" role="dialog" aria-label={label}>
      <div class="grid">
        {#each COLOURS as c (c.hex)}
          <button class="sw" class:on={same(c.hex, value)} style="background:{c.hex}" aria-pressed={same(c.hex, value)} aria-label={c.name} title={`${label}: ${c.name}`} onclick={() => choose(c.hex)}></button>
        {/each}
      </div>
      <div
        class="square"
        style="background-color:hsl({hsv.h} 100% 50%)"
        role="slider"
        tabindex="0"
        aria-label={`${label}: shade and brightness`}
        aria-valuetext={value}
        aria-valuenow={Math.round(hsv.v * 100)}
        title={`${label}: click or drag to choose the shade and brightness; the arrow keys move it`}
        onpointerdown={(e) => { dragging = true; e.currentTarget.setPointerCapture(e.pointerId); pointAt(e); }}
        onpointermove={(e) => { if (dragging) pointAt(e); }}
        onpointerup={() => (dragging = false)}
        onkeydown={squareKey}
      >
        <span class="knob" style="left:{hsv.s * 100}%;top:{(1 - hsv.v) * 100}%"></span>
      </div>
      <input class="hue" type="range" min="0" max="360" step="1" value={hsv.h} aria-label={`${label}: hue`} title={`${label}: slide to change the hue`} oninput={(e) => mix({ ...hsv, h: e.currentTarget.valueAsNumber })} />
      <div class="code">
        <span class="chip" style="background:{value}"></span>
        <input type="text" bind:value={typed} maxlength="7" spellcheck="false" aria-label={`${label}: colour code`} title="The colour's code, like #2f6fed; type one and press Enter" onchange={typedHex} onkeydown={(e) => { if (e.key === 'Enter') typedHex(); }} />
      </div>
    </div>
  {/if}
</span>

<style>
  .picker { display: inline-block; }
  button.cur { box-sizing: border-box; width: 40px; height: 24px; min-width: 0; padding: 0; border: 1px solid #888; border-radius: 4px; cursor: pointer; }
  button.cur:disabled { opacity: .4; cursor: default; }
  .pop { position: fixed; z-index: 2000; width: 196px; box-sizing: border-box; padding: 8px; display: flex; flex-direction: column; gap: 8px; border: 1px solid #bbb; border-radius: 8px; background: #fff; box-shadow: 0 6px 20px rgba(0, 0, 0, .3); }
  .grid { display: grid; grid-template-columns: repeat(6, 1fr); gap: 4px; }
  .sw { box-sizing: border-box; width: 100%; aspect-ratio: 1; min-width: 0; padding: 0; border: 1px solid #aaa; border-radius: 4px; cursor: pointer; }
  .sw.on { outline: 2px solid #2f6fed; outline-offset: 1px; }
  .square { position: relative; height: 110px; border-radius: 4px; cursor: crosshair; touch-action: none; background-image: linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent); }
  .square:focus-visible { outline: 2px solid #2f6fed; }
  .knob { position: absolute; width: 12px; height: 12px; margin: -6px 0 0 -6px; border: 2px solid #fff; border-radius: 50%; box-shadow: 0 0 0 1px rgba(0, 0, 0, .6); pointer-events: none; }
  .hue { width: 100%; margin: 0; appearance: none; height: 12px; border-radius: 6px; background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00); }
  .hue::-webkit-slider-thumb { appearance: none; width: 14px; height: 14px; border-radius: 50%; border: 2px solid #fff; background: transparent; box-shadow: 0 0 0 1px rgba(0, 0, 0, .6); }
  .code { display: flex; align-items: center; gap: 6px; }
  .chip { width: 22px; height: 22px; border: 1px solid #aaa; border-radius: 4px; }
  .code input { flex: 1; min-width: 0; padding: 3px 6px; font-family: monospace; }
</style>
