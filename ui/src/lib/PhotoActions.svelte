<script lang="ts">
  /** A row of buttons that act on some photos: the ones chosen in Settings, Browsing (Email, Print and Tag… to begin with), and More for everything else. One row sits beside Picked
   *  in Browse (acting on the picked photos), another under the details (acting on the chosen photo, whose tags are right there, so no Tag…). A button opens its tool in the tab; Tag…
   *  is a popover so the grid stays in view. When there is no room the last buttons fold into More. */
  import { BAR_ACTIONS, split } from './actionBar';
  import GroupTags from './GroupTags.svelte';
  import { dismiss, placeBeside } from './popover';
  import TagAs from './TagAs.svelte';
  import { GROUPS, toolById, worksOnWholeLibrary } from './tools';

  let { source, ids, chosen, limit = Infinity, showTag = false, alignRight = false, unavailable = {}, onopen, ontagchange = () => {} }: {
    /** Which photos the row acts on: the picked ones, or the chosen one. */
    source: 'picked' | 'current';
    /** Those photos' ids (for the tags). */
    ids: number[];
    /** The actions chosen for the bar (Settings, Browsing). */
    chosen: string[];
    /** How many buttons fit; the rest of the chosen ones go under More. */
    limit?: number;
    /** Tag… is a button on this row (it is always under More). */
    showTag?: boolean;
    /** The popovers open towards the left (the row is at the right edge of the window). */
    alignRight?: boolean;
    /** Actions that cannot be used on these photos right now, each with the reason (shown as its tooltip); they are greyed out. */
    unavailable?: Record<string, string>;
    /** Open a tool in its tab, working on those photos. */
    onopen: (toolId: string, source: 'picked' | 'current') => void;
    /** Something was tagged (with the tag named): the tag lists need reading again. */
    ontagchange?: (tag: string) => void;
  } = $props();

  const what = $derived(source === 'picked' ? 'the picked photos' : 'this photo');
  const offered = (id: string) => id !== 'tag' || showTag;
  const shown = $derived(split(chosen, limit, offered).direct);
  const label = (id: string) => BAR_ACTIONS.find((a) => a.id === id)?.label ?? id;

  let open = $state<'tag' | 'more' | null>(null);
  let moreBtn: HTMLElement | undefined = $state();
  /** The popover is fixed to the window (so a pane it sits in cannot clip it), under the button that opened it. */
  let popStyle = $state('');
  function placeUnder(el: HTMLElement) { popStyle = placeBeside(el, { alignRight }); }
  function toggle(which: 'tag' | 'more', e: MouseEvent) {
    if (open === which) { open = null; return; }
    placeUnder(e.currentTarget as HTMLElement);
    open = which;
  }

  /** Everything not shown as a button, by tab, for More: the ones not built yet are greyed. */
  const more = $derived(GROUPS.filter((g) => !worksOnWholeLibrary(g.id))
    .map((g) => ({
      ...g,
      actions: BAR_ACTIONS.filter((a) => !shown.includes(a.id) && (a.id === 'tag' ? 'organize' : toolById(a.id)?.group) === g.id),
    }))
    .filter((g) => g.actions.length > 0));
  function go(id: string) {
    if (id === 'tag') {
      if (moreBtn) placeUnder(moreBtn);
      open = 'tag';
    } else {
      open = null;
      onopen(id, source);
    }
  }
</script>

<span class="actions" role="group" aria-label="Actions on {what}" use:dismiss={{ active: open !== null, close: () => (open = null), closeOnMove: true }}>
  {#each shown as id (id)}
    {#if id === 'tag'}
      <button class="act" aria-haspopup="dialog" aria-expanded={open === 'tag'} title="Add tags to these photos, or take them off" onclick={(e) => toggle('tag', e)}>Tag…</button>
    {:else}
      <button class="act" disabled={id in unavailable} onclick={() => go(id)} title={unavailable[id] ?? `${label(id)}: ${what}`}>{label(id)}</button>
    {/if}
  {/each}
  <button class="act" bind:this={moreBtn} aria-haspopup="menu" aria-expanded={open === 'more'} title="More things to do with these photos" onclick={(e) => toggle('more', e)}>More ▾</button>
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
  {#if open === 'more'}
    <div class="pop menu" style={popStyle} role="menu" aria-label="More tools">
      {#each more as g (g.id)}
        <div role="group" aria-label={g.label}>
          <div class="gh" aria-hidden="true">{g.label}</div>
          {#each g.actions as a (a.id)}
            <button role="menuitem" class:soon={!a.built || a.id in unavailable} aria-disabled={!a.built || a.id in unavailable} disabled={!a.built || a.id in unavailable} title={unavailable[a.id]} onclick={() => go(a.id)}>{a.label}{#if !a.built}<small>soon</small>{/if}</button>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</span>

<style>
  .actions { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 6px; font-size: 13px; }
  .act { border: 1px solid #b8bdca; background: #fff; border-radius: 4px; padding: 2px 10px; font: inherit; font-size: 13px; cursor: pointer; }
  .act:disabled { opacity: .5; cursor: default; }
  .pop { position: fixed; z-index: 60; min-width: 240px; background: #fff; border: 1px solid #c5c8d2; border-radius: 6px; box-shadow: 0 6px 20px rgba(0, 0, 0, .18); padding: 6px 4px; overflow-y: auto; }
  .menu { min-width: 220px; padding: 4px 0; }
  .menu .gh { margin: 6px 14px 2px; font-size: 11px; letter-spacing: .05em; text-transform: uppercase; color: #7a7a86; font-weight: 600; }
  .menu button { display: block; width: 100%; text-align: left; border: 0; border-radius: 0; background: none; font: inherit; padding: 5px 14px; cursor: pointer; }
  .menu button:hover:not(:disabled) { background: #2f6fed; color: #fff; }
  .menu button.soon { color: #9a9aa4; cursor: default; }
  .menu small { margin-left: 6px; font-size: 11px; color: #b0b0b8; }
</style>
