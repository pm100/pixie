<script lang="ts">
  import type { BgProgress } from './backgroundProgress';

  // The small indicator at the top right of the window while Pixie works in the background: what it is doing, how far
  // along it is, and, on click, the details (Settings → Picture analysis). It is simply absent when nothing is running.
  let { progress, onclick }: { progress: BgProgress | null; onclick: () => void } = $props();
  const percent = $derived(progress && progress.fraction !== null ? Math.round(progress.fraction * 100) : null);
</script>

{#if progress}
  <button class="bg {progress.state}" title={progress.detail} aria-label={`${progress.text}. ${progress.detail}`} {onclick}>
    {#if progress.state === 'working'}<span class="spin" aria-hidden="true"></span>{:else if progress.state === 'error'}<span aria-hidden="true">⚠</span>{:else}<span aria-hidden="true">⏸</span>{/if}
    <span class="t">{progress.text}</span>
    <span class="bar" class:unknown={percent === null} role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent ?? undefined}>
      <i style={percent === null ? undefined : `width: ${percent}%`}></i>
    </span>
  </button>
{/if}

<style>
  .bg { display: inline-flex; align-items: center; gap: 7px; padding: 2px 10px; border: 1px solid #b8c6ee; border-radius: 12px; background: #eaf0ff; color: #1a3f9a; font: inherit; font-size: 12px; cursor: pointer; }
  .bg:hover { background: #dce6ff; }
  .bg.paused { border-color: #d8c58a; background: #fff6dc; color: #7a5b00; }
  .bg.error { border-color: #e3a3a8; background: #fdecea; color: #b00020; }
  .spin { width: 11px; height: 11px; border: 2px solid #b8c6ee; border-top-color: #2f6fed; border-radius: 50%; animation: spin .9s linear infinite; }
  .bar { position: relative; width: 70px; height: 5px; border-radius: 3px; background: #c9d6f7; overflow: hidden; }
  .bar i { display: block; height: 100%; background: #2f6fed; transition: width .3s; }
  .bar.unknown i { position: absolute; width: 35%; animation: slide 1.3s ease-in-out infinite; }
  .paused .bar, .error .bar { display: none; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes slide { 0% { left: -35%; } 100% { left: 100%; } }
  @media (prefers-reduced-motion: reduce) { .spin, .bar.unknown i { animation-duration: 4s; } }
</style>
