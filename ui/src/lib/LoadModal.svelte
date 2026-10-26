<script lang="ts">
  import FlowStage from './FlowStage.svelte';
  import type { AnalysisView } from './analysisView';
  import { readFraction, scanText, type ScanState } from './scanProgress';
  import type { StripItem } from './stripFeed';
  import { thumbUrl } from './url';

  type Sample = StripItem;

  let { phase, scan, folder, strip, view, notice = '', onstop, onbackground, onretry, onresume }: {
    phase: 'scanning' | 'analysing';
    scan: ScanState;
    /** The folder being read, if known. */
    folder: string | null;
    /** The photos read most recently, oldest first. */
    strip: Sample[];
    /** The one bar and the counters of the analysing step. */
    view: AnalysisView;
    /** A problem to show, such as a folder that could not be read; the scan carries on with the others. */
    notice?: string;
    onstop: () => void; onbackground: () => void; onretry: () => void; onresume: () => void;
  } = $props();

  // Newest in the middle, older ones fading away either side: [rank 3, rank 1, rank 0, rank 2, rank 4].
  const ranks = [3, 1, 0, 2, 4];
  const slots = $derived(ranks.map((rank) => ({ rank, item: strip[strip.length - 1 - rank] ?? null })));
  const newest = $derived(strip[strip.length - 1] ?? null);
  const fraction = $derived(readFraction(scan));
  let broken = $state(new Set<number>());
  const hide = (id: number) => (broken = new Set([...broken, id]));
</script>

<FlowStage label="Getting to know your photos">
  <h1>Getting to know your photos…</h1>
  <p class="sub">Pixie is looking at each photo. It adds Auto tags (labels such as Beach or Museum that you can browse by) and finds the people in them. This all happens on your computer; nothing is uploaded.</p>
  {#if notice}<p class="err" role="alert">{notice}</p>{/if}
  <!-- The pictures go by the whole time: first as the files are found, then as each is looked at. -->
  <div class="strip" aria-hidden="true">
    {#each slots as s (s.rank)}
      <div class="slot r{s.rank}">
        {#if s.item && s.item.kind === 'video'}
          <!-- A video has no thumbnail: a film tile stands in for it. -->
          <div class="video"><svg viewBox="0 0 24 24" width="40%" height="40%" aria-hidden="true"><path d="M8 5v14l11-7z" fill="currentColor" /></svg></div>
        {:else if s.item && !broken.has(s.item.id)}
          <img alt="" src={thumbUrl('grid', s.item.id, s.item.mtime, s.item.size)} onerror={() => hide(s.item!.id)} />
        {/if}
      </div>
    {/each}
  </div>
  <p class="name">{newest?.name ?? ' '}</p>
  <!-- One screen from the first moment: the files are worked on as the walk finds them. -->
  <div class="overall">
    <div class="head"><b>{phase === 'scanning' ? `${scanText(scan)}${folder ? ` · ${folder}` : ''}` : view.headline}</b></div>
    {#if phase === 'scanning' || view.fraction === null}
      <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100"><div style="width: {phase === 'scanning' && fraction !== null ? Math.round(fraction * 100) : 4}%"></div></div>
    {:else}
      <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(view.fraction * 100)}><div style="width: {Math.round(view.fraction * 100)}%"></div></div>
    {/if}
  </div>
  <dl class="counters">
    {#each view.counters as c (c.key)}
      <div class="counter" class:done={c.done}><dt>{c.label}</dt><dd>{c.text}</dd></div>
    {/each}
  </dl>
  {#each view.problems as p (p.key + p.kind)}
    {#if p.kind === 'error'}
      <div class="err" role="alert">{p.text} <button onclick={onretry}>Retry</button></div>
    {:else}
      <div class="err" role="status">{p.text} <button onclick={onresume}>Resume</button></div>
    {/if}
  {/each}
  <p class="note">This can take a while for a big library. You don't have to wait here.</p>
  {#snippet footer()}
    {#if phase === 'scanning'}
      <button onclick={onstop}>Stop</button>
    {/if}
    <button class="primary" onclick={onbackground}>Carry on in the background</button>
  {/snippet}
</FlowStage>

<style>
  .strip { display: flex; align-items: center; justify-content: center; gap: 8px; height: 150px; margin: 18px 0 6px; }
  .slot { background: #dde2ee; border-radius: 6px; overflow: hidden; flex: none; }
  .slot img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .video { width: 100%; height: 100%; display: grid; place-items: center; background: #2b3550; color: #dfe6fb; }
  .r0 { width: 150px; height: 106px; box-shadow: 0 2px 8px rgba(0, 0, 0, .25); order: 3; }
  .r1 { width: 60px; height: 80px; opacity: .7; order: 2; }
  .r3 { width: 40px; height: 56px; opacity: .45; order: 1; }
  .r2 { width: 60px; height: 80px; opacity: .7; order: 4; }
  .r4 { width: 40px; height: 56px; opacity: .45; order: 5; }
  .name { text-align: center; color: #777; font-size: 12px; margin: 0; min-height: 1.4em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .overall { margin: 4px 0 14px; }
  .head { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .counters { margin: 0 0 14px; display: grid; gap: 4px; }
  .counter { display: flex; justify-content: space-between; gap: 12px; }
  .counter dt { font-weight: 600; }
  .counter dd { margin: 0; color: #555; white-space: nowrap; }
  .counter.done dd { color: #1d6b2a; }
  .counter.done dd::after { content: ' ✓'; }
  .err { margin-top: 6px; color: #b00020; font-size: 13px; }
  .err button { margin-left: 8px; padding: 2px 10px; }
  .note { color: #666; font-size: 13px; }
</style>
