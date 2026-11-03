<script lang="ts">
  import { untrack } from 'svelte';
  import CloudFetch from './CloudFetch.svelte';
  import { holdEstimate, view, type ActivitySnapshot, type CloudSource, type Row } from './activity';

  // Everything Pixie is doing in the background, one row each, with how far along it is, how fast, and how long is left. The same
  // screen is the pill's page and the middle of the Add folder popup (`compact`: no padding of its own, the popup has its own).
  let { snapshot, onpause, onretry, scanPaused = false, showFinished = false, compact = false }: {
    snapshot: ActivitySnapshot | null;
    /** Pause (true) or Resume (false) everything: the analysis, and a folder scan that is running. */
    onpause: (paused: boolean) => void;
    onretry: () => void;
    /** A folder scan was stopped by Pause and has not been started again: the button offers Resume. */
    scanPaused?: boolean;
    /** The list of finished runs under the running ones. Hidden for now (it was not found useful); the code is kept. */
    showFinished?: boolean;
    compact?: boolean;
  } = $props();

  // The time left is lost now and then (no speed to judge it by for a spell): keep the last one while work goes on.
  let held = $state<number | null>(null);
  $effect(() => {
    const running = (snapshot?.tasks ?? []).some((t) => t.state === 'running' || t.state === 'waiting');
    const next = holdEstimate(untrack(() => held), snapshot?.eta_secs ?? null, running);
    if (next !== untrack(() => held)) held = next;
  });
  const v = $derived(view(snapshot, held));
  // The drive's mark stays when the list of files being fetched is empty for a moment: the last drive named is kept.
  let lastCloud = $state<CloudSource>('other');
  $effect(() => {
    const named = v.running.find((r) => r.kind === 'cloud_wait' && r.files.length > 0 && r.source !== null && r.source !== 'other');
    if (named?.source && named.source !== untrack(() => lastCloud)) lastCloud = named.source;
  });
  // Paused: everything open is paused, something is held while other work goes on, or a scan or sync that Pause stopped is waiting to be resumed.
  const paused = $derived(v.pill === 'paused' || v.held || scanPaused);
  const canToggle = $derived(v.running.length > 0 || scanPaused);
  const overall = $derived(scanPaused && v.pill === null ? 'Paused.' : v.overall);
  // The running area never scrolls: it has room for this many rows (a cloud row, with its list of files, takes the room of two) and says
  // how many more there are. The Add folder popup scrolls as a whole and shows them all.
  const BUDGET = 5;
  const weight = (r: Row) => (r.kind === 'cloud_wait' ? 2 : 1);
  const shown = $derived.by(() => {
    if (compact) return v.running;
    const out: Row[] = [];
    let used = 0;
    for (const r of v.running) {
      if (out.length > 0 && used + weight(r) > BUDGET) break;
      out.push(r);
      used += weight(r);
    }
    return out;
  });
  const hidden = $derived(v.running.length - shown.length);
  const percent = (r: Row) => (r.fraction === null ? null : Math.round(r.fraction * 100));
</script>

<section class="act" class:compact aria-label="What Pixie is doing">
  <div class="top">
    <p class="overall" role="status">{overall}</p>
    <!-- Always here, Pause at the right edge: nothing below moves them. -->
    <div class="actions">
      {#if v.pill === 'error'}<button onclick={onretry} title="Try again what went wrong, such as getting the models.">Retry</button>{/if}
      <button disabled={!canToggle} onclick={() => onpause(!paused)} title={!canToggle ? 'Nothing is running to pause.' : paused ? 'Carry on with everything that was paused: the background work, the sync, and looking through the folders.' : 'Pause the background work, including the sync and looking through the folders. What was found so far is kept, and Resume carries on.'}>{paused ? 'Resume' : 'Pause'}</button>
    </div>
  </div>

  <!-- The running rows have an area of their own that never scrolls; the finished list (when shown) has a fixed height below it. -->
  <div class="running">
  {#if v.running.length > 0}
    <ul class="rows">
      {#each shown as r (r.kind)}
        <li class={r.state}>
          {#if r.source === null}
            <div class="head"><span class="name">{r.label}</span><span class="counts">{r.counts}</span></div>
            <div class="bar" class:unknown={percent(r) === null} role="progressbar" aria-label={r.label} aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent(r) ?? undefined}>
              <i style={percent(r) === null ? undefined : `width: ${percent(r)}%`}></i>
            </div>
          {:else}
            <!-- A cloud wait has no count or bar (the number of files keeps changing): a picture of files coming down from the cloud. -->
            <CloudFetch source={r.files.length > 0 ? r.source : lastCloud} active={r.files.length > 0} />
          {/if}
          <div class="meta"><span class="explain" title={r.explainFull}>{r.explain}</span></div>
          {#if r.place !== null}<div class="place" title={r.placeFull ? `Working through ${r.placeFull}` : ''}>{r.place || ' '}</div>{/if}
          {#if r.kind === 'cloud_wait'}
            <!-- The big files being fetched now. The space is kept whatever is in the list, so the rows below do not move. -->
            <ul class="files">
              {#each r.files as f, i (i)}<li><span class="fname" title={f.name}>{f.name}</span><span class="fdetail">{f.detail}</span></li>{/each}
              {#if r.moreFiles > 0}<li class="fmore">…and {r.moreFiles} more</li>{/if}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
    {#if hidden > 0}<p class="more">…and {hidden} more. They are listed here as the others finish.</p>{/if}
  {/if}
  </div>

  {#if showFinished}
    <h4>Finished</h4>
    <div class="finished">
      {#if v.finished.length > 0}
        <ul class="done">
          {#each v.finished as r (r.key)}
            <li><span class="name">{r.label}</span><span class="counts">{r.counts}</span><span class="explain">{r.explain}</span></li>
          {/each}
        </ul>
      {:else}
        <p class="none">Nothing has finished yet.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .act { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: var(--page-pad); box-sizing: border-box; }
  .act.compact { padding: 0; }
  .running { flex: 1; min-height: 0; overflow: hidden; }
  .finished { flex: none; height: 120px; overflow-y: auto; }
  .none { margin: 0; font-size: 13px; color: #777; }
  /* in the Add folder popup the whole popup scrolls, so the running rows are not given a box of their own */
  .act.compact { height: auto; }
  .compact .running { overflow: visible; }
  .compact .explain { white-space: normal; }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 32px; margin: 0 0 12px; }
  .overall { margin: 0; font-weight: 600; }
  ul { list-style: none; margin: 0; padding: 0; }
  .rows > li { margin-bottom: 10px; }
  .files { list-style: none; margin: 2px 0 0; padding: 0; min-height: 5.6em; font-size: 12px; color: #555; }
  .files li { display: flex; justify-content: space-between; gap: 12px; line-height: 1.4; }
  .fname { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fdetail { flex: none; color: #777; }
  .fmore { color: #777; }
  .head { display: flex; justify-content: space-between; gap: 12px; min-height: 1.4em; }
  .meta { display: flex; gap: 14px; min-height: 1.4em; }
  .place { min-height: 1.4em; font-size: 12px; color: #666; font-family: ui-monospace, Consolas, 'Courier New', monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more { margin: 0; font-size: 12px; color: #777; }
  .name { font-weight: 500; }
  .counts, .meta { color: #555; font-size: 13px; }
  .bar { position: relative; height: 6px; margin: 5px 0 3px; border-radius: 3px; background: #dbe3f7; overflow: hidden; }
  .bar i { display: block; height: 100%; background: #2f6fed; transition: width .3s; }
  .bar.unknown i { position: absolute; width: 35%; animation: slide 1.3s ease-in-out infinite; }
  .paused .bar i { background: #b59a3c; }
  .error .bar i { background: #b00020; }
  .explain { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: #666; }
  .error .explain { color: #b00020; }
  .actions { display: flex; gap: 8px; flex: none; }
  .actions button:last-child { min-width: 84px; } /* Pause and Resume are the same size */
  h4 { margin: 18px 0 6px; font-size: 13px; color: #555; }
  .done li { display: grid; grid-template-columns: minmax(0, 3fr) minmax(0, 3fr) minmax(0, 2fr); align-items: baseline; gap: 12px; padding: 2px 0; font-size: 13px; }
  .done .name { font-weight: 500; }
  .done .explain { margin: 0; font-size: inherit; color: #555; min-height: 0; }
  @keyframes slide { 0% { left: -35%; } 100% { left: 100%; } }
  @media (prefers-reduced-motion: reduce) { .bar.unknown i { animation-duration: 4s; } }
</style>
