<script lang="ts">
  /** A row of buttons that act on some photos: Email, Print, optionally Tag…, and More. One row sits beside Picked in Browse (acting on the picked photos), another in the
   *  details pane (acting on the chosen photo, which has its tags right there, so no Tag…). Email and Print open the tool in its tab; Tag… is a popover so the grid stays in
   *  view; More lists every other tool. */
  import GroupTags from './GroupTags.svelte';
  import TagAs from './TagAs.svelte';
  import { GROUPS, TOOLS } from './tools';

  let { source, count, ids, label = '', showTag = false, alignRight = false, onopen, ontagchange = () => {} }: {
    /** Which photos the row acts on: the picked ones, or the chosen one. */
    source: 'picked' | 'current';
    /** How many photos there are to act on; the buttons are off when none. */
    count: number;
    /** Those photos' ids (for the tags). */
    ids: number[];
    /** Words before the buttons, for example "3 picked:". */
    label?: string;
    /** Show the Tag… button. */
    showTag?: boolean;
    /** The popovers open towards the left (the row is at the right edge of the window). */
    alignRight?: boolean;
    /** Open a tool in its tab, working on those photos. */
    onopen: (toolId: string, source: 'picked' | 'current') => void;
    /** Something was tagged (with the tag named): the tag lists need reading again. */
    ontagchange?: (tag: string) => void;
  } = $props();

  const usable = $derived(count > 0);
  const what = $derived(source === 'picked' ? 'the picked photos' : 'this photo');

  let open = $state<'tag' | 'more' | null>(null);
  let root: HTMLElement | undefined = $state();
  /** The popover is fixed to the window (so a pane it sits in cannot clip it), under the button that opened it. */
  let popStyle = $state('');
  function toggle(which: 'tag' | 'more', e: MouseEvent) {
    if (open === which) { open = null; return; }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    popStyle = alignRight ? `top:${Math.round(r.bottom + 4)}px;right:${Math.round(window.innerWidth - r.right)}px` : `top:${Math.round(r.bottom + 4)}px;left:${Math.round(r.left)}px`;
    open = which;
  }
  $effect(() => { if (!usable) open = null; });
  $effect(() => {
    if (!open) return;
    const close = () => (open = null);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => { window.removeEventListener('resize', close); window.removeEventListener('scroll', close, true); };
  });
  function away(e: MouseEvent) { if (open && root && e.target instanceof Node && !root.contains(e.target)) open = null; }
  function keys(e: KeyboardEvent) { if (e.key === 'Escape' && open) { open = null; e.stopPropagation(); } }

  /** Every tool without a button of its own, by tab: the ones not built yet are greyed. */
  const more = GROUPS.filter((g) => g.id !== 'cleanup')
    .map((g) => ({ ...g, tools: TOOLS.filter((t) => t.group === g.id && !['email', 'print', 'tags'].includes(t.id)) }))
    .filter((g) => g.tools.length > 0);
  const go = (id: string) => { open = null; onopen(id, source); };
</script>

<svelte:window onclick={away} onkeydown={keys} />

<span class="actions" role="group" aria-label="Actions on {what}" bind:this={root}>
  {#if label}<span class="who">{label}</span>{/if}
  <button class="act" disabled={!usable} onclick={() => go('email')} title="Email {what}">Email</button>
  <button class="act" disabled={!usable} onclick={() => go('print')} title="Print {what}">Print</button>
  {#if showTag}
    <button class="act" disabled={!usable} aria-haspopup="dialog" aria-expanded={open === 'tag'} onclick={(e) => toggle('tag', e)}>Tag…</button>
    {#if open === 'tag'}
      <div class="pop" style={popStyle} role="dialog" aria-label="Tags">
        {#if ids.length === 1}
          <!-- One photo: straight to finding or creating a tag. -->
          <TagAs {ids} inline onchange={ontagchange} ondone={() => (open = null)} />
        {:else}
          <GroupTags {ids} onchange={ontagchange} />
        {/if}
      </div>
    {/if}
  {/if}
  <button class="act" disabled={!usable} aria-haspopup="menu" aria-expanded={open === 'more'} onclick={(e) => toggle('more', e)}>More ▾</button>
  {#if open === 'more'}
    <div class="pop menu" style={popStyle} role="menu" aria-label="More tools">
      {#each more as g (g.id)}
        <h4>{g.label}</h4>
        {#each g.tools as t (t.id)}
          <button role="menuitem" class:soon={!t.built} aria-disabled={!t.built} disabled={!t.built} onclick={() => go(t.id)}>{t.label}{#if !t.built}<small>soon</small>{/if}</button>
        {/each}
      {/each}
      <div class="sep"></div>
      <button role="menuitem" onclick={() => go('email')}>All tools…</button>
    </div>
  {/if}
</span>

<style>
  .actions { display: inline-flex; align-items: center; gap: 6px; padding: 2px 6px; background: #eef2fb; border: 1px solid #c9d6f5; border-radius: 6px; font-size: 13px; }
  .who { color: #555; }
  .act { border: 1px solid #b8bdca; background: #fff; border-radius: 4px; padding: 2px 10px; font: inherit; font-size: 13px; cursor: pointer; }
  .act:disabled { opacity: .5; cursor: default; }
  .pop { position: fixed; z-index: 60; min-width: 240px; background: #fff; border: 1px solid #c5c8d2; border-radius: 6px; box-shadow: 0 6px 20px rgba(0, 0, 0, .18); padding: 6px 4px; }
  .menu { min-width: 220px; padding: 4px 0; }
  .menu h4 { margin: 6px 14px 2px; font-size: 11px; letter-spacing: .05em; text-transform: uppercase; color: #7a7a86; font-weight: 600; }
  .menu button { display: block; width: 100%; text-align: left; border: 0; border-radius: 0; background: none; font: inherit; padding: 5px 14px; cursor: pointer; }
  .menu button:hover:not(:disabled) { background: #2f6fed; color: #fff; }
  .menu button.soon { color: #9a9aa4; cursor: default; }
  .menu small { margin-left: 6px; font-size: 11px; color: #b0b0b8; }
  .sep { height: 1px; background: #e4e4e8; margin: 4px 0; }
</style>
