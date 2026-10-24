<script lang="ts">
  import { faceCardLine, type FacePerson } from './people';
  import { faceUrl } from './url';
  import ThisIs from './ThisIs.svelte';

  let { person, people, onopen, onassign, onname, onhide, onunhide }: {
    person: FacePerson;
    /** Who a group can be given to. */
    people: { id: number; name: string }[];
    onopen: () => void;
    onassign: (personId: number) => void;
    onname: (name: string) => void;
    onhide: () => void;
    onunhide: () => void;
  } = $props();

  const group = $derived(person.name === null);
  /** A group is taken to be one person, so one picture is enough. */
  const shownFaces = $derived(person.samples.slice(0, 1));
</script>

<section class="card" class:dim={person.hidden}>
  <button class="samples" onclick={onopen} aria-label={`See all ${person.faces} faces${person.name ? ' of ' + person.name : ''}`}>
    {#each shownFaces as f (f)}<img alt="" loading="lazy" decoding="async" src={faceUrl(f, undefined, 320)} />{/each}
  </button>
  <div class="head">
    {#if person.name}<strong class="name">{person.name}</strong>{:else}<span class="line">{faceCardLine(person)}</span>{/if}
  </div>
  {#if person.name}<div class="line">{faceCardLine(person)}{person.hidden ? ' · hidden' : ''}</div>{/if}
  <div class="actions">
    {#if group && !person.hidden}
      <ThisIs {people} onpick={onassign} onnew={onname} />
    {/if}
    {#if person.hidden}
      <button onclick={onunhide}>Unhide</button>
    {:else}
      <button onclick={onhide} title="Leave this person out of the lists">Hide</button>
    {/if}
  </div>
</section>

<style>
  .card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1px solid #d3d8e4; border-radius: 10px; background: #fafbfe; min-width: 0; }
  .card.dim { opacity: .65; }
  .samples { display: grid; grid-template-columns: 1fr; gap: 4px; padding: 0; border: 0; background: none; cursor: pointer; }
  .samples img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 6px; background: #dde2ee; }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .name { font-size: 15px; }
  .line { color: #666; font-size: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  button { font: inherit; font-size: 13px; }
</style>
