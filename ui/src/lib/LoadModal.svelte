<script lang="ts">
  import FlowStage from './FlowStage.svelte';
  import type { Line } from './loadFlow';
  import { readFraction, scanText, type ScanState } from './scanProgress';
  import { thumbUrl } from './url';

  type Sample = { id: number; mtime: number; size: number; name: string };

  let { phase, scan, folder, strip, lines, notice = '', onstop, onbackground, onretry, onresume }: {
    phase: 'scanning' | 'analysing';
    scan: ScanState;
    /** The folder being read, if known. */
    folder: string | null;
    /** The photos read most recently, oldest first. */
    strip: Sample[];
    lines: Line[];
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

<FlowStage label={phase === 'scanning' ? 'Getting to know your photos' : 'Pictures loaded, now analysing'}>
  {#if phase === 'scanning'}
    <h1>Getting to know your photos…</h1>
    <p class="sub">{scanText(scan)}{folder ? ` · ${folder}` : ''}</p>
    {#if notice}<p class="err" role="alert">{notice}</p>{/if}
    <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}>
      <div style="width: {fraction === null ? 4 : Math.round(fraction * 100)}%"></div>
    </div>
    <div class="strip" aria-hidden="true">
      {#each slots as s (s.rank)}
        <div class="slot r{s.rank}">
          {#if s.item && !broken.has(s.item.id)}
            <img alt="" src={thumbUrl('grid', s.item.id, s.item.mtime, s.item.size)} onerror={() => hide(s.item!.id)} />
          {/if}
        </div>
      {/each}
    </div>
    <p class="name">{newest?.name ?? ' '}</p>
    <div class="actions"><button onclick={onstop}>Stop</button></div>
  {:else}
    <h1>Pictures loaded — now analysing</h1>
    <p class="sub">Pixie is now looking at each photo so you can search them and find the people in them. This all happens on your computer; nothing is uploaded.</p>
    {#if notice}<p class="err" role="alert">{notice}</p>{/if}
    {#each lines as l (l.key)}
      <div class="line">
        <div class="head"><b>{l.title}</b><span class="t">{l.text}</span></div>
        {#if l.fraction !== null}
          <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(l.fraction * 100)}><div style="width: {Math.round(l.fraction * 100)}%"></div></div>
        {/if}
        <div class="why">{l.why}</div>
        {#if l.error}
          <div class="err" role="alert">{l.error} <button onclick={onretry}>Retry</button></div>
        {:else if l.paused}
          <div class="err" role="status">Paused. <button onclick={onresume}>Resume</button></div>
        {/if}
      </div>
    {/each}
    <p class="note">This can take a while for a big library. You don't have to wait here.</p>
    <div class="actions"><button class="primary" onclick={onbackground}>Carry on in the background</button></div>
  {/if}
</FlowStage>

<style>
  .strip { display: flex; align-items: center; justify-content: center; gap: 8px; height: 150px; margin: 18px 0 6px; }
  .slot { background: #dde2ee; border-radius: 6px; overflow: hidden; flex: none; }
  .slot img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .r0 { width: 150px; height: 106px; box-shadow: 0 2px 8px rgba(0, 0, 0, .25); order: 3; }
  .r1 { width: 60px; height: 80px; opacity: .7; order: 2; }
  .r3 { width: 40px; height: 56px; opacity: .45; order: 1; }
  .r2 { width: 60px; height: 80px; opacity: .7; order: 4; }
  .r4 { width: 40px; height: 56px; opacity: .45; order: 5; }
  .name { text-align: center; color: #777; font-size: 12px; margin: 0; min-height: 1.4em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line { margin-bottom: 16px; }
  .head { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .t { color: #555; white-space: nowrap; }
  .why { color: #777; font-size: 12px; margin-top: 4px; }
  .err { margin-top: 6px; color: #b00020; font-size: 13px; }
  .err button { margin-left: 8px; padding: 2px 10px; }
  .note { color: #666; font-size: 13px; }
</style>
