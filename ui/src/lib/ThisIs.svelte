<script lang="ts">
  /** "This is…": say who a group or some faces are by typing a name. People already named that match what is typed are
   *  listed to click; Enter commits what was typed (a name somebody has means the same person), Esc closes. */
  let { people, label = 'This is…', onpick, onnew, small = false }: {
    /** The people it can be (named, not hidden, not the one it already is). */
    people: { id: number; name: string }[];
    label?: string;
    onpick: (personId: number) => void;
    /** A name nobody has: a new person. */
    onnew: (name: string) => void;
    small?: boolean;
  } = $props();

  let open = $state(false);
  let draft = $state('');
  let root = $state<HTMLElement | null>(null);

  const text = $derived(draft.trim());
  const matches = $derived(text === '' ? people : people.filter((p) => p.name.toLowerCase().includes(text.toLowerCase())));
  const exact = $derived(people.find((p) => p.name.toLowerCase() === text.toLowerCase()) ?? null);

  function close() {
    open = false;
    draft = '';
  }
  function pick(id: number) {
    close();
    onpick(id);
  }
  function commit() {
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
  <button class="trigger" class:small aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = !open)}>{label} <span aria-hidden="true">▾</span></button>
  {#if open}
    <div class="menu">
      <input
        use:focus
        bind:value={draft}
        placeholder="Who is this? Type a name, then Enter"
        aria-label="Who is this?"
        autocomplete="off"
        onkeydown={(e) => {
          if (e.key === 'Enter') { e.preventDefault(); commit(); }
          else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); }
        }}
      />
      {#if matches.length > 0}
        <div class="list">
          {#each matches as p (p.id)}
            <button onclick={() => pick(p.id)}>{p.name}</button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</span>

<style>
  .thisis { position: relative; display: inline-block; }
  .trigger { font: inherit; font-size: 13px; }
  .trigger.small { font-size: 12px; padding: 1px 6px; }
  .menu { position: absolute; z-index: 40; top: calc(100% + 4px); left: 0; width: 260px; padding: 6px; border: 1px solid #c3c8d4; border-radius: 8px; background: #fff; color: #222; box-shadow: 0 6px 20px rgba(0, 0, 0, .18); display: flex; flex-direction: column; gap: 4px; }
  input { font: inherit; font-size: 13px; padding: 5px 7px; border: 1px solid #c3c8d4; border-radius: 6px; width: 100%; box-sizing: border-box; }
  .list { display: flex; flex-direction: column; max-height: 220px; overflow: auto; }
  .list button { text-align: left; font: inherit; font-size: 13px; border: 0; background: none; padding: 5px 8px; border-radius: 5px; cursor: pointer; color: inherit; }
  .list button:hover, .list button:focus-visible { background: #e3ecff; }
</style>
