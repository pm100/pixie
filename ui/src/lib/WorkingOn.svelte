<script lang="ts">
  /** "Working on": which photos a tool acts on. Shared by Send, Create and Organize, so the choice carries from one tab to the next. */
  import type { UserTagCount } from './api';
  import { plural } from './format';
  import { dismiss } from './popover';

  let { source = $bindable(), pickedCount, hasPhoto, userTags, count, onclear, error = '' }: {
    /** `picked`, `current` (the photo chosen in Browse), or `tag:<name>`. */
    source: string;
    pickedCount: number;
    /** A photo is chosen in Browse, so "Current" can be used. */
    hasPhoto: boolean;
    userTags: UserTagCount[];
    /** How many photos the choice gives. */
    count: number;
    onclear: () => void;
    error?: string;
  } = $props();

  const onTag = $derived(source.startsWith('tag:'));
  const tagName = $derived(onTag ? source.slice(4) : '');

  // The tag segment opens a small list of your tags (a plain select would list its own label as a choice).
  let listOpen = $state(false);
  $effect(() => { if (userTags.length === 0) listOpen = false; });
  const choose = (name: string) => { source = `tag:${name}`; listOpen = false; };
</script>

<div class="bar">
  <span class="lbl" id="working-on-label">Working on:</span>
  <div class="seg" role="group" aria-labelledby="working-on-label">
    <button class="opt" aria-pressed={source === 'picked'} class:on={source === 'picked'} onclick={() => (source = 'picked')}>★ Picked ({pickedCount.toLocaleString()})</button>
    <button class="opt" aria-pressed={source === 'current'} class:on={source === 'current'} disabled={!hasPhoto}
            title={hasPhoto ? 'The photo chosen in Browse' : 'Choose a photo in Browse first'} onclick={() => (source = 'current')}>Current</button>
    <span class="tagpick" use:dismiss={{ active: listOpen, close: () => (listOpen = false) }}>
      <button class="opt" class:on={onTag} aria-pressed={onTag} aria-haspopup="listbox" aria-expanded={listOpen} disabled={userTags.length === 0}
              title={userTags.length === 0 ? 'You have no tags yet' : 'Work on the photos with one of your tags'} onclick={() => (listOpen = !listOpen)}>{onTag ? tagName : 'A tag…'} ▾</button>
      {#if listOpen}
        <ul class="list" role="listbox" aria-label="Your tags">
          {#each userTags as t (t.name)}
            <li><button role="option" aria-selected={source === `tag:${t.name}`} onclick={() => choose(t.name)}><span class="n">{t.name}</span><span class="c">{t.count.toLocaleString()}</span></button></li>
          {/each}
        </ul>
      {/if}
    </span>
  </div>
  <span class="count">{plural(count, 'photo')}</span>
  {#if source === 'picked'}
    <button class="clear" onclick={onclear} disabled={pickedCount === 0} title="Take everything off Picked (the photos stay in the library)">Clear picks</button>
  {/if}
  {#if error}<span class="err" role="alert">{error}</span>{/if}
</div>

<style>
  .bar { display: flex; gap: 12px; align-items: center; padding: 6px 14px; border-bottom: 1px solid #eee; font-size: 14px; }
  .lbl { color: #555; }
  .seg { display: flex; border: 1px solid #b8bdca; border-radius: 6px; }  /* not overflow: hidden, which would clip the tag list */
  .opt { padding: 3px 12px; background: #fff; border: 0; border-right: 1px solid #b8bdca; border-radius: 0; font: inherit; color: #1d1d22; cursor: pointer; }
  .opt:last-child { border-right: 0; }
  .seg > .opt:first-child { border-radius: 5px 0 0 5px; }
  .tagpick > .opt { border-radius: 0 5px 5px 0; }
  .opt.on { background: #2f6fed; color: #fff; }
  .opt:disabled { color: #9a9aa4; background: #f4f4f7; cursor: default; }
  .tagpick { position: relative; display: flex; }
  .tagpick > .opt { border-right: 0; }
  .list { position: absolute; z-index: 20; left: 0; top: calc(100% + 4px); min-width: 180px; max-height: 260px; overflow-y: auto; margin: 0; padding: 4px 0; list-style: none; background: #fff; border: 1px solid #c5c8d2; border-radius: 6px; box-shadow: 0 6px 20px rgba(0, 0, 0, .18); }
  .list button { display: flex; justify-content: space-between; gap: 12px; width: 100%; text-align: left; border: 0; border-radius: 0; background: none; font: inherit; padding: 4px 12px; cursor: pointer; }
  .list button:hover { background: #2f6fed; color: #fff; }
  .list button[aria-selected="true"] { font-weight: 600; }
  .list .c { opacity: .65; font-variant-numeric: tabular-nums; }
  .count { font-size: 13px; font-weight: 600; color: #333; font-variant-numeric: tabular-nums; }
  .clear { border: 1px solid #c9ceda; background: #fff; border-radius: 4px; padding: 3px 10px; font: inherit; cursor: pointer; }
  .clear:disabled { opacity: .5; cursor: default; }
  .err { color: #b00020; font-size: 13px; }
</style>
