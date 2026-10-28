<script lang="ts">
  /** "Tag as…": give some photos a tag. Type to find one of your tags, or to make a new one. */
  import { tagPhotos, tagUsage, type TagUse } from './api';
  import { enterChoice, tagAsRows } from './userTags';

  let { ids, onchange, floating = true }: {
    /** The photos to tag. */
    ids: number[];
    /** The box opens over what is below the + (as beside the tag pills); false: it opens in the flow, as in a narrow column. */
    floating?: boolean;
    /** Something was tagged (with the tag named): the tag lists need reading again. */
    onchange: (tag: string) => void;
  } = $props();

  let open = $state(false);
  let query = $state('');
  let usage = $state<TagUse[]>([]);
  let error = $state('');
  let box = $state<HTMLDivElement | null>(null);
  let field = $state<HTMLInputElement | null>(null);

  const list = $derived(tagAsRows(usage, ids.length, query));

  /** Where the floating box goes. It is fixed to the window (so the pane it sits in cannot clip it or scroll sideways) and opens to
   *  the left of the +, over the divider and the photos, since the + is near the right edge of the window. */
  let dropStyle = $state('');
  let plus = $state<HTMLButtonElement | null>(null);
  const BOX_WIDTH = 240;
  function place() {
    if (!plus) return;
    const r = plus.getBoundingClientRect();
    dropStyle = `top:${Math.round(r.bottom + 4)}px;left:${Math.max(8, Math.round(r.right - BOX_WIDTH))}px;width:${BOX_WIDTH}px`;
  }

  async function show() {
    place();
    open = true;
    query = '';
    error = '';
    try { usage = await tagUsage(ids); } catch (e) { error = String(e); }
    field?.focus();
  }
  const close = () => { open = false; query = ''; };

  async function apply(name: string) {
    let stored: string;
    try {
      stored = await tagPhotos(ids, name);
    } catch (e) {
      error = String(e);
      return;
    }
    close();
    onchange(stored);
  }

  /** Enter: see `enterChoice`. */
  function enter() {
    const name = enterChoice(usage, ids.length, query);
    if (name !== null) void apply(name);
  }

  // A click anywhere else closes the box.
  $effect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => { if (box && !box.contains(e.target as Node)) close(); };
    window.addEventListener('mousedown', away);
    // The box is fixed to the window, so it would be left behind if the pane scrolled or the window changed size.
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => {
      window.removeEventListener('mousedown', away);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
    };
  });
</script>

<div class="tagas" class:floating bind:this={box}>
  <button bind:this={plus} class="plus" aria-label="Add a tag" title="Add a tag" aria-expanded={open} disabled={ids.length === 0} onclick={() => (open ? close() : void show())}>+</button>
  {#if open}
    <div class="drop" style={floating ? dropStyle : undefined}>
      <input bind:this={field} type="text" autocomplete="off" placeholder="Find or create a tag" aria-label="Find or create a tag" bind:value={query}
             onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); enter(); } else if (e.key === 'Escape') { e.stopPropagation(); close(); } }} />
      {#if error}<p class="err" role="alert">{error}</p>{/if}
      <ul>
        {#each list.rows as r (r.name)}
          <li><button class="row" disabled={r.all} onclick={() => void apply(r.name)}><span class="name"># {r.name}</span><span class="note">{r.all ? 'on all' : r.on > 0 ? `on ${r.on}` : ''}</span></button></li>
        {/each}
        {#if list.more > 0}<li class="more">{list.more} more. Keep typing to narrow the list.</li>{/if}
        {#if list.create}<li><button class="row create" onclick={() => void apply(list.create!)}>+ Create tag “{list.create}”</button></li>{/if}
        {#if list.rows.length === 0 && !list.create}<li class="more">No tags yet. Type a name to make one.</li>{/if}
      </ul>
    </div>
  {/if}
</div>

<style>
  .tagas { display: inline-flex; flex-direction: column; align-items: flex-start; gap: 6px; }
  .tagas.floating .drop { position: fixed; z-index: 50; box-sizing: border-box; }
  /* A round + that sits in the row of tag pills, shaped like one. */
  .plus { width: 22px; height: 22px; padding: 0; border: 1px dashed #9aa3b8; border-radius: 11px; background: none; color: #5a6378; font: inherit; line-height: 1; cursor: pointer; }
  .plus:hover:not(:disabled), .plus[aria-expanded="true"] { border-color: #2f6fed; color: #2f6fed; }
  .plus:disabled { opacity: .5; cursor: default; }
  .drop { align-self: stretch; border: 1px solid #d6d9e2; border-radius: 6px; padding: 6px; background: #fff; box-shadow: 0 4px 16px rgba(20, 28, 50, .14); display: flex; flex-direction: column; gap: 4px; }
  input { width: 100%; box-sizing: border-box; padding: 5px 8px; }
  ul { list-style: none; margin: 0; padding: 0; }
  .row { display: flex; justify-content: space-between; gap: 8px; width: 100%; text-align: left; border: 0; background: none; font: inherit; padding: 5px 4px; border-radius: 4px; cursor: pointer; }
  .row:hover:not(:disabled) { background: #eef2fb; }
  .row:disabled { opacity: .55; cursor: default; }
  .name { min-width: 0; overflow-wrap: anywhere; }
  .note { color: #777; font-size: 12px; flex: none; }
  .create { color: #2b5fd9; font-weight: 600; }
  .more { color: #777; font-size: 12px; padding: 4px; }
  .err { color: #b00020; font-size: 13px; margin: 0; }
</style>
