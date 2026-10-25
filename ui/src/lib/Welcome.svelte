<script lang="ts">
  import AppIcon from './AppIcon.svelte';
  import FlowStage from './FlowStage.svelte';
  import { addLabel, startLabel } from './loadFlow';
  import { countText, listedRows, lookingText, tickedPaths, type Row } from './suggestions';

  let { rows, searching, looking, ontoggle, onadd, onskip, onstart, adding = false, canSearchMore = false, onsearch, oncancel }: {
    rows: Row[];
    /** Used to add folders later (from Settings): no welcome, nothing ticked to start with, and the wider search of the
     *  computer only when asked for. */
    adding?: boolean;
    /** (adding) The wider search has not been run yet: offer it. */
    canSearchMore?: boolean;
    onsearch?: () => void; oncancel?: () => void;
    /** Pixie is still sniffing for photo folders (drives included): a spinner covers the list while it fills in. */
    searching: boolean;
    /** What it is sniffing right now, if known. */
    looking: string | null;
    ontoggle: (path: string) => void; onadd: () => void; onskip: () => void; onstart: () => void;
  } = $props();

  // One list. Folders already inside another suggestion, and folders with nothing in them, are not shown.
  const shown = $derived(listedRows(rows));
  const start = $derived(adding ? addLabel(tickedPaths(rows).length) : startLabel(tickedPaths(rows).length));
</script>

<FlowStage label={adding ? 'Add folders' : 'Welcome to Pixie'}>
  {#if adding}
    <h1 class="adding">Add folders</h1>
    <p class="sub left">Pick the folders to add. Pixie never moves or changes your files.</p>
  {:else}
    <div class="hero">
      <AppIcon size={104} />
      <h1>Welcome to Pixie</h1>
      <p class="sub">Pixie shows the photos and videos already on your computer. It never moves or changes them. Pick where to look:</p>
    </div>
  {/if}

  <h2>{adding ? 'Folders you could add' : 'Suggested folders'}</h2>
  <div class="listwrap" aria-busy={searching}>
    <div class="list">
      {#each shown as r (r.path)}
        <label class="row">
          <input type="checkbox" checked={r.ticked} onchange={() => ontoggle(r.path)} />
          <span class="what"><b>{r.name}</b><span class="path">{r.path}</span></span>
          <span class="n">{r.note ?? countText(r.count, r.kind)}</span>
        </label>
      {/each}
      {#if shown.length === 0 && !searching}
        <p class="none">{adding ? 'No other folders to suggest. Choose one below, or search the rest of this computer.' : 'Pixie did not find any photo folders. Add one below.'}</p>
      {/if}
    </div>
    {#if searching && !adding}
      <div class="cover" role="status">
        <span class="spinner" aria-hidden="true"></span>
        <span class="msg">Searching your computer…</span>
        {#if looking}<span class="where">{lookingText(looking)}</span>{/if}
      </div>
    {/if}
  </div>

  {#if adding && searching && looking}
    <p class="searching" role="status"><span class="spinner small" aria-hidden="true"></span>Searching… {lookingText(looking)}</p>
  {/if}
  <p>
    <button onclick={onadd}>{adding ? 'Choose a folder…' : 'Add another folder…'}</button>
    {#if adding && canSearchMore && !searching}<button onclick={onsearch}>Search the rest of this computer</button>{/if}
  </p>
  {#if !adding}<p class="note">You can add, remove or change these any time in <b>Settings → Photo folders</b>.</p>{/if}

  {#snippet footer()}
    {#if adding}
      <button onclick={oncancel}>Cancel</button>
      <button class="primary" disabled={start.disabled} onclick={onstart}>{start.label}</button>
    {:else}
      <button onclick={onskip}>Skip for now</button>
      <button class="primary" disabled={start.disabled || searching} onclick={onstart}>{start.label}</button>
    {/if}
  {/snippet}
</FlowStage>

<style>
  .hero { text-align: center; }
  h1.adding { margin: 0 0 4px; font-size: 22px; }
  .sub.left { text-align: left; margin: 0 0 12px; }
  .searching { display: flex; align-items: center; gap: 8px; margin: 8px 0 0; color: #555; font-size: 13px; }
  .spinner.small { width: 14px; height: 14px; }
  .hero :global(svg) { display: block; margin: 0 auto 10px; filter: drop-shadow(0 3px 6px rgba(0, 0, 0, .2)); }
  h2 { margin: 6px 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; color: #777; }
  .listwrap { position: relative; min-height: 190px; border-radius: 8px; }
  .list { border: 1px solid #d3d8e4; border-radius: 8px; overflow: hidden; background: #fff; min-height: 190px; }
  .row { display: flex; gap: 10px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #eef0f6; cursor: pointer; }
  .row:last-child { border-bottom: 0; }
  .what { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .path { color: #777; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .n { color: #555; font-size: 13px; white-space: nowrap; }
  .none { margin: 0; padding: 16px 12px; color: #666; }
  /* The spinner sits in front of the list; the list fills in behind it. */
  .cover {
    position: absolute; inset: 0; z-index: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px;
    border-radius: 8px; background: rgba(255, 255, 255, .8); backdrop-filter: blur(1px);
  }
  .spinner { width: 56px; height: 56px; border: 6px solid #d5ddf3; border-top-color: #2f6fed; border-radius: 50%; animation: spin .9s linear infinite; }
  .msg { font-size: 17px; font-weight: 600; color: #222; }
  .where { font-size: 13px; color: #666; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 3s; } }
  .note { color: #666; font-size: 13px; }
</style>
