<script lang="ts">
  import { onMount } from 'svelte';
  import { flatten, matches, type Notice } from './licences';

  let { onback }: { onback: () => void } = $props();

  // The list is large (every licence text), so it is loaded when this page is opened, not with the app.
  let all = $state<Notice[]>([]);
  let failed = $state(false);
  let query = $state('');
  let open = $state<number | null>(null);
  onMount(() => {
    import('../generated/third-party.json').then((m) => (all = flatten(m.default.groups))).catch(() => (failed = true));
  });
  const shown = $derived(all.filter((n) => matches(n, query)));
</script>

<button class="back" onclick={onback}>← About</button>
<h2>Open-source software in Pixie</h2>
<p class="hint">Pixie is built on the software below, each under its own licence, and downloads two models on first use. Thank you to everyone who made it. {all.length > 0 ? `${all.length.toLocaleString()} entries.` : ''}</p>
<input type="search" placeholder="Find by name or licence" aria-label="Find by name or licence" bind:value={query} />
{#if failed}<p class="err">The list could not be loaded.</p>{/if}
<ul>
  {#each shown as n (n.id)}
    <li>
      <button class="row" aria-expanded={open === n.id} onclick={() => (open = open === n.id ? null : n.id)}>
        <span class="name">{n.name} {n.version}</span><span class="lic">{n.license}</span>
      </button>
      {#if open === n.id}
        <div class="detail">
          {#if n.note}<p>{n.note}</p>{/if}
          <p class="url">{n.url}</p>
          <pre>{n.text}</pre>
        </div>
      {/if}
    </li>
  {/each}
</ul>
{#if all.length > 0 && shown.length === 0}<p class="hint">Nothing matches.</p>{/if}

<style>
  .back { border: 0; background: none; color: #2b5fd9; padding: 0; margin-bottom: 8px; cursor: pointer; font: inherit; }
  .hint { color: #666; font-size: 13px; }
  input { width: 100%; max-width: 420px; padding: 6px 9px; margin-bottom: 8px; }
  ul { list-style: none; padding: 0; margin: 0; max-width: 760px; }
  li { border-bottom: 1px solid #ececf0; }
  .row { display: flex; justify-content: space-between; gap: 12px; width: 100%; text-align: left; border: 0; background: none; font: inherit; padding: 5px 2px; cursor: pointer; }
  .row:hover { background: #f3f6fd; }
  .name { min-width: 0; overflow-wrap: anywhere; }
  .lic { color: #666; font-size: 13px; text-align: right; flex: none; max-width: 45%; overflow-wrap: anywhere; }
  .detail { padding: 2px 4px 10px; }
  .detail p { margin: 0 0 6px; }
  .url { color: #666; font-size: 13px; user-select: text; overflow-wrap: anywhere; }
  pre { margin: 0; padding: 8px; background: #f6f6f8; border-radius: 4px; font-size: 12px; white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; }
  .err { color: #b00020; }
</style>
