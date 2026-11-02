<script lang="ts">
  import type { StripItem } from './stripFeed';
  import { clipTitle } from './clipTitle';
  import { thumbUrl } from './url';

  // The photos Pixie has just looked at, going by: the newest in the middle, older ones fading away either side, and the newest one's
  // name under them. Shown while Pixie loads (the Add folder popup, and the Processing screen).
  let { strip }: {
    /** The photos read most recently, oldest first. */
    strip: StripItem[];
  } = $props();

  // Newest in the middle, older ones fading away either side: [rank 3, rank 1, rank 0, rank 2, rank 4].
  const ranks = [3, 1, 0, 2, 4];
  const slots = $derived(ranks.map((rank) => ({ rank, item: strip[strip.length - 1 - rank] ?? null })));
  const newest = $derived(strip[strip.length - 1] ?? null);
  let broken = $state(new Set<number>());
  const hide = (id: number) => (broken = new Set([...broken, id]));
</script>

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
<p class="name" use:clipTitle>{newest?.name ?? ' '}</p>

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
</style>
