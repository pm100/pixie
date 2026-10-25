<script lang="ts">
  /** "This is…": say who a group or some faces are by typing a name. People already named that match what is typed are
   *  listed to click; Enter commits what was typed (a name somebody has means the same person), Esc closes. */
  let { people, label = 'This is…', onpick, onnew, small = false, disabled = false, candidates, onignore, alignRight = false }: {
    /** The people it can be (named, not hidden, not the one it already is). */
    people: { id: number; name: string }[];
    label?: string;
    onpick: (personId: number) => void;
    /** A name nobody has: a new person. */
    onnew: (name: string) => void;
    small?: boolean;
    disabled?: boolean;
    /** Asked when the box opens: who it looks like most, best first. They are listed first, above everyone. */
    candidates?: () => Promise<{ id: number; name: string }[]>;
    /** Offers "Ignore" at the bottom of the list: these are nobody to name. */
    onignore?: () => void;
    /** The box opens leftwards from the button (for a button at the right edge of the window). */
    alignRight?: boolean;
  } = $props();

  let open = $state(false);
  let draft = $state('');
  let root = $state<HTMLElement | null>(null);
  /** The match chosen with the arrow keys (-1: none, so Enter uses what was typed). */
  let active = $state(-1);
  let likely = $state<{ id: number; name: string }[]>([]);

  const text = $derived(draft.trim());
  const fits = (p: { name: string }) => text === '' || p.name.toLowerCase().includes(text.toLowerCase());
  const likelyShown = $derived(likely.filter(fits));
  /** What the arrow keys walk through: the likely ones, then everyone. */
  const matches = $derived([...likelyShown, ...people.filter(fits)]);
  const exact = $derived(people.find((p) => p.name.toLowerCase() === text.toLowerCase()) ?? null);

  function show() {
    open = !open;
    likely = [];
    if (open && candidates) candidates().then((c) => { if (open) likely = c.filter((x) => people.some((p) => p.id === x.id)); }).catch(() => {});
  }
  function close() {
    open = false;
    draft = '';
    active = -1;
  }
  /** The row after the names: "Ignore". */
  const ignoreRow = $derived(onignore ? matches.length : -2);
  function move(by: number) {
    const last = onignore ? matches.length : matches.length - 1;
    if (last < 0) return;
    active = Math.min(last, Math.max(0, active + by)); // from none, Down is the first and Up the first too
    queueMicrotask(() => root?.querySelector('.list button.on, .ignore.on')?.scrollIntoView({ block: 'nearest' }));
  }
  function ignore() {
    close();
    onignore?.();
  }
  function pick(id: number) {
    close();
    onpick(id);
  }
  function commit() {
    if (active === ignoreRow) return ignore();
    if (active >= 0 && matches[active]) return pick(matches[active].id);
    const name = text; // closing clears the draft, so take what was typed first
    if (name === '') return;
    const same = exact;
    close();
    if (same) onpick(same.id);
    else onnew(name);
  }
  const focus = (el: HTMLInputElement) => el.focus();
</script>

<svelte:window
  onclick={(e) => { if (open && root && !e.composedPath().includes(root)) close(); }}
/>

<span class="thisis" bind:this={root}>
  <button class="trigger" class:small {disabled} aria-haspopup="dialog" aria-expanded={open} onclick={show}>{label} <span aria-hidden="true">▾</span></button>
  {#if open}
    <div class="menu" class:right={alignRight}>
      <input
        use:focus
        bind:value={draft}
        oninput={() => (active = -1)}
        placeholder="Who is this? Type a name, then Enter"
        aria-label="Who is this?"
        autocomplete="off"
        onkeydown={(e) => {
          if (e.key === 'Enter') { e.preventDefault(); commit(); }
          else if (e.key === 'ArrowDown') { e.preventDefault(); move(1); }
          else if (e.key === 'ArrowUp') { e.preventDefault(); move(-1); }
          else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); }
        }}
      />
      {#if matches.length > 0}
        <div class="list">
          {#each matches as p, i (i)}
            {#if i === likelyShown.length && likelyShown.length > 0}<hr />{/if}
            <button class:on={i === active} class:likely={i < likelyShown.length} tabindex="-1" onclick={() => pick(p.id)}>{p.name}</button>
          {/each}
        </div>
      {/if}
      {#if onignore}
        <hr />
        <button class="ignore" class:on={active === ignoreRow} tabindex="-1" onclick={ignore} title="These are nobody to name: leave them out of the lists">Ignore</button>
      {/if}
    </div>
  {/if}
</span>

<style>
  .thisis { position: relative; display: inline-block; }
  .trigger { font: inherit; font-size: 13px; }
  .trigger.small { font-size: 12px; padding: 1px 6px; }
  .menu { position: absolute; z-index: 40; top: calc(100% + 4px); left: 0; width: 260px; padding: 6px; border: 1px solid #c3c8d4; border-radius: 8px; background: #fff; color: #222; box-shadow: 0 6px 20px rgba(0, 0, 0, .18); display: flex; flex-direction: column; gap: 4px; }
  .menu.right { left: auto; right: 0; }
  .menu hr { border: 0; border-top: 1px solid #e3e6ee; margin: 2px 0; width: 100%; }
  .ignore { text-align: left; font: inherit; font-size: 13px; border: 0; background: none; padding: 5px 8px; border-radius: 5px; cursor: pointer; color: #6b4a00; }
  .ignore:hover, .ignore.on { background: #fff0c9; }
  input { font: inherit; font-size: 13px; padding: 5px 7px; border: 1px solid #c3c8d4; border-radius: 6px; width: 100%; box-sizing: border-box; }
  .list { display: flex; flex-direction: column; max-height: 220px; overflow: auto; }
  .list button { text-align: left; font: inherit; font-size: 13px; border: 0; background: none; padding: 5px 8px; border-radius: 5px; cursor: pointer; color: inherit; }
  .list hr { border: 0; border-top: 1px solid #e3e6ee; margin: 4px 0; width: 100%; }
  .list button.likely { font-weight: 600; }
  .list button:hover, .list button:focus-visible, .list button.on { background: #e3ecff; }
  .list button.on { outline: 1px solid #9db6ee; }
</style>
