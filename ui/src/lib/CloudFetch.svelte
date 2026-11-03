<script lang="ts">
  import type { CloudSource } from './activity';

  // A cloud (with the mark of the drive on it), an arrow pulsing towards a laptop: files coming down from the cloud to this computer.
  // The marks are simple drawings of the drives, not their official artwork.
  // `active`: files are being fetched right now; the arrows pulse only then, and rest while the drive has nothing for us.
  let { source, active = true }: { source: CloudSource; active?: boolean } = $props();

  const NAMES: Record<CloudSource, string> = { onedrive: 'OneDrive', icloud: 'iCloud', google_drive: 'Google Drive', other: 'your cloud drive' };
  const text = $derived(`Fetching files from ${NAMES[source]} to this computer`);
  const CLOUD = 'M7 15a4 4 0 0 1-.5-7.97A5.5 5.5 0 0 1 17 6.2 4.4 4.4 0 0 1 17.5 15Z';
</script>

<div class="fetch" class:idle={!active} role="img" aria-label={text} title={text}>
  <svg class="cloud" viewBox="0 0 72 48" aria-hidden="true">
    <g transform="scale(3)"><path d={CLOUD} class="body" /></g>
    {#if source === 'onedrive'}
      <g transform="translate(23 20) scale(1.1)">
        <path d={CLOUD} fill="#28a8ea" transform="translate(4 -1)" />
        <path d={CLOUD} fill="#0f6cbd" transform="scale(.8) translate(-2 3)" />
      </g>
    {:else if source === 'icloud'}
      <g transform="translate(24 21) scale(.95)"><path d={CLOUD} fill="#3b9cf5" /></g>
    {:else if source === 'google_drive'}
      <g transform="translate(26 20)">
        <polygon points="6,0 14,0 20,11 12,11" fill="#ffba00" />
        <polygon points="6,0 12,11 8,18 0,18" fill="#0da960" />
        <polygon points="8,18 12,11 20,11 20,18" fill="#4285f4" />
      </g>
    {/if}
  </svg>
  <svg class="arrow" viewBox="0 0 60 24" aria-hidden="true">
    <path class="chev c1" d="M8 5l8 7-8 7" />
    <path class="chev c2" d="M26 5l8 7-8 7" />
    <path class="chev c3" d="M44 5l8 7-8 7" />
  </svg>
  <svg class="laptop" viewBox="0 0 72 48" aria-hidden="true">
    <rect x="14" y="7" width="44" height="29" rx="3" class="screen" />
    <path d="M6 40h60l-5 5H11Z" class="base" />
  </svg>
</div>

<style>
  .fetch { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 4px 0; }
  svg { display: block; flex: none; }
  .cloud, .laptop { width: 72px; height: 48px; }
  .arrow { width: 60px; height: 24px; }
  .cloud .body { fill: #eaf1fb; stroke: #9db4d8; stroke-width: .5; }
  .screen { fill: #f4f7fd; stroke: #7b8aa8; stroke-width: 2.5; }
  .base { fill: #b7c2d8; stroke: #7b8aa8; stroke-width: 1.5; stroke-linejoin: round; }
  .chev { fill: none; stroke: #2f6fed; stroke-width: 3.5; stroke-linecap: round; stroke-linejoin: round; opacity: .25; animation: pulse 1.5s ease-in-out infinite; }
  .c2 { animation-delay: .25s; }
  .c3 { animation-delay: .5s; }
  .idle .chev { animation: none; opacity: .25; }
  @keyframes pulse { 0%, 100% { opacity: .2; } 40% { opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .chev { animation: none; opacity: .8; } }
</style>
