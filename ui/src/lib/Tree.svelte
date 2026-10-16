<script lang="ts">
  import { untrack } from 'svelte';
  import type { DayGroup, Folder } from './api';
  import { basename } from './format';
  import { activeKeys, buildDateTree, type TreeNode } from './tree';

  let { groups, folders, activeGroup, folderId, mapActive, ongroup, onfolder, onmap }: {
    groups: DayGroup[];
    folders: Folder[];
    activeGroup: number;
    folderId: number | null;
    mapActive: boolean;
    ongroup: (group: number) => void;
    onfolder: (id: number | null) => void;
    onmap: () => void;
  } = $props();

  const tree = $derived(buildDateTree(groups));
  const active = $derived(activeKeys(tree, activeGroup));
  let open = $state<Record<string, boolean>>({});

  // Follow the grid: open the path to whatever is at the top of the scroll.
  $effect(() => {
    const keys = active;
    untrack(() => { for (const k of keys) open[k] = true; });
  });

  const toggle = (n: TreeNode) => (open[n.key] = !open[n.key]);
</script>

{#snippet branch(n: TreeNode, depth: number)}
  <div class="row" class:active={active.has(n.key) && n.children.length === 0}
       style:padding-left="{8 + depth * 14}px">
    {#if n.children.length}
      <button class="twisty" onclick={() => toggle(n)} aria-label={open[n.key] ? 'Collapse' : 'Expand'}>{open[n.key] ? '▾' : '▸'}</button>
    {:else}
      <span class="twisty"></span>
    {/if}
    <button class="label" onclick={() => ongroup(n.firstGroup)}>{n.label}</button>
    <span class="n">{n.count}</span>
  </div>
  {#if open[n.key]}
    {#each n.children as c (c.key)}
      {@render branch(c, depth + 1)}
    {/each}
  {/if}
{/snippet}

<nav>
  <h3>Dates</h3>
  {#each tree as n (n.key)}
    {@render branch(n, 0)}
  {:else}
    <p class="none">No photos yet.</p>
  {/each}

  <h3>Folders</h3>
  <button class="item" class:active={folderId === null} onclick={() => onfolder(null)}>All photos</button>
  {#each folders as f (f.id)}
    <button class="item" class:active={folderId === f.id} title={f.path} onclick={() => onfolder(f.id)}>{basename(f.path) || f.path}</button>
  {/each}

  <h3>Map</h3>
  <button class="item" class:active={mapActive} onclick={onmap}>Map</button>
</nav>

<style>
  nav { padding: 6px 4px 16px; font-size: 14px; }
  h3 { margin: 14px 8px 4px; font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: #777; }
  .row { display: flex; align-items: center; gap: 2px; padding-right: 8px; border-radius: 4px; }
  .row.active { background: #e3ecff; }
  .twisty { width: 18px; flex: none; padding: 0; border: 0; background: none; cursor: pointer; color: #555; }
  .label { flex: 1; text-align: left; padding: 3px 4px; border: 0; background: none; cursor: pointer; font: inherit; }
  .n { color: #888; font-size: 12px; }
  .item { display: block; width: 100%; text-align: left; padding: 3px 12px; border: 0; background: none; cursor: pointer; font: inherit; border-radius: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item.active { background: #e3ecff; }
  .none { margin: 6px 12px; color: #888; }
</style>
