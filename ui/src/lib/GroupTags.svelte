<script lang="ts">
  /** Tags for a whole set of photos (the Tags tool): the tags every one of them has, each with an x, and a + to give them all another. */
  import { tagUsage, untagPhotos, type TagUse } from './api';
  import { plural } from './format';
  import { createLatest } from './latest';
  import TagAs from './TagAs.svelte';
  import { tagsOnAll } from './userTags';

  let { ids, onchange }: {
    /** The photos the tags apply to. */
    ids: number[];
    /** Tags were added or removed (the tag named): the tag lists need reading again. */
    onchange: (tag: string) => void;
  } = $props();

  let usage = $state<TagUse[]>([]);
  let rev = $state(0);
  const latest = createLatest();
  $effect(() => {
    rev;
    const list = ids;
    const token = latest.next();
    if (list.length === 0) { usage = []; return; }
    tagUsage(list).then((u) => { if (latest.isCurrent(token)) usage = u; }).catch(() => {});
  });
  const onAll = $derived(tagsOnAll(usage, ids.length));
  const changed = (tag: string) => { rev++; onchange(tag); };
  async function remove(name: string) {
    try { await untagPhotos(ids, name); } catch { /* shown again as it was */ }
    changed(name);
  }
</script>

<p class="hint">Changes here apply to all {plural(ids.length, 'photo')}.</p>
<div class="chips">
  <span class="label">Common tags:</span>
  {#each onAll as t (t)}<span class="chip">{t}<button class="x" aria-label="Take {t} off all of them" onclick={() => remove(t)}>×</button></span>{/each}
  {#if onAll.length === 0}<span class="hint">none</span>{/if}
  <TagAs {ids} onchange={changed} />
</div>

<style>
  .label { font-size: 13px; color: #555; }
  .hint { margin: 0; color: #777; font-size: 13px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; margin-top: 10px; }
  .chip { padding: 1px 8px; border-radius: 4px; background: #e3ecff; color: #2a3f7a; font-size: 12px; }
  .x { border: 0; background: none; padding: 0 0 0 5px; color: inherit; opacity: .7; cursor: pointer; line-height: 1; }
  .x:hover { opacity: 1; }
</style>
