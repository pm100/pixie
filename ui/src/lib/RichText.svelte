<script lang="ts">
  import { parseRich } from './richText';

  /** An answer with its numbered steps, points and bold names laid out. Everything is shown as text, never as HTML. */
  let { text }: { text: string } = $props();
  const blocks = $derived(parseRich(text));
</script>

{#each blocks as b, i (i)}
  {#if b.kind === 'p'}
    <p>{#each b.spans as s, j (j)}{#if s.bold}<b>{s.text}</b>{:else}{s.text}{/if}{/each}</p>
  {:else}
    {@const items = b.items}
    {#if b.kind === 'steps'}
      <ol>{#each items as item, k (k)}<li>{#each item as s, j (j)}{#if s.bold}<b>{s.text}</b>{:else}{s.text}{/if}{/each}</li>{/each}</ol>
    {:else}
      <ul>{#each items as item, k (k)}<li>{#each item as s, j (j)}{#if s.bold}<b>{s.text}</b>{:else}{s.text}{/if}{/each}</li>{/each}</ul>
    {/if}
  {/if}
{/each}

<style>
  p { margin: 0 0 6px; }
  p:last-child, ol:last-child, ul:last-child { margin-bottom: 0; }
  ol, ul { margin: 2px 0 6px; padding-left: 22px; }
  li { margin: 3px 0; }
  b { font-weight: 600; }
</style>
