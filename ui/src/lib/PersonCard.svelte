<script lang="ts">
  import { faceCardLine, type FacePerson } from './people';
  import { faceUrl } from './url';
  import ThisIs from './ThisIs.svelte';

  let { person, people, onopen, onassign, onname }: {
    person: FacePerson;
    /** Who a group can be given to. */
    people: { id: number; name: string }[];
    onopen: () => void;
    onassign: (personId: number) => void;
    onname: (name: string) => void;
  } = $props();

  const group = $derived(person.name === null);
  /** The name being edited (the pencil), or null when it is not being edited. */
  let editing = $state<string | null>(null);
  const focus = (el: HTMLInputElement) => { el.focus(); el.select(); };
  function commit() {
    const text = (editing ?? '').trim();
    editing = null;
    if (text !== '' && text !== person.name) onname(text); // a name somebody has joins the two
  }
  /** A group is taken to be one person, so one picture is enough. */
  const shownFaces = $derived(person.samples.slice(0, 1));
</script>

<section class="card">
  <button class="samples" onclick={onopen} aria-label={`See all ${person.faces} faces${person.name ? ' of ' + person.name : ''}`}>
    {#each shownFaces as f (f)}<img alt="" loading="lazy" decoding="async" src={faceUrl(f, undefined, 320)} />{/each}
  </button>
  <div class="head">
    {#if editing !== null}
      <input class="rename" use:focus value={editing} aria-label="Name" oninput={(e) => (editing = e.currentTarget.value)} onblur={commit}
             onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); commit(); } else if (e.key === 'Escape') { e.preventDefault(); editing = null; } }} />
    {:else if person.name}
      <strong class="name">{person.name}</strong>
      <button class="pencil" onclick={() => (editing = person.name)} aria-label={`Rename ${person.name}`} title="Rename">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M11.5 2.5l2 2L5 13l-3 1 1-3 8.5-8.5z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>
      </button>
    {:else}<span class="line">{faceCardLine(person)}</span>{/if}
  </div>
  {#if person.name}<div class="line">{faceCardLine(person)}</div>{/if}
  <div class="actions">
    {#if group}
      <ThisIs {people} onpick={onassign} onnew={onname} />
    {/if}
  </div>
</section>

<style>
  .card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1px solid #d3d8e4; border-radius: 10px; background: #fafbfe; min-width: 0; }
  .samples { display: grid; grid-template-columns: 1fr; gap: 4px; padding: 0; border: 0; background: none; cursor: pointer; }
  .samples img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 6px; background: #dde2ee; }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .name { font-size: 15px; }
  .pencil { display: inline-flex; padding: 3px; border: 0; background: none; color: #667; border-radius: 4px; cursor: pointer; }
  .pencil:hover { background: #e3ecff; color: #1a3f9a; }
  .rename { font: inherit; font-size: 14px; padding: 2px 6px; border: 1px solid #9db6ee; border-radius: 6px; width: 100%; box-sizing: border-box; }
  .line { color: #666; font-size: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  button { font: inherit; font-size: 13px; }
</style>
