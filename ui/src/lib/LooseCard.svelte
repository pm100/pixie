<script lang="ts">
  /** A face that fits no group, shown like any other card on the To name screen: its picture, how many photos, "This is…". */
  import { suggestForFaces } from './api';
  import { toCandidates, type FaceTile } from './people';
  import { faceUrl } from './url';
  import ThisIs from './ThisIs.svelte';

  let { tile, people, onopen, onassign, onname, onignore }: {
    tile: FaceTile;
    /** Who it can be given to. */
    people: { id: number; name: string }[];
    /** Show the whole photo this face is in. */
    onopen: () => void;
    onassign: (personId: number) => void;
    onname: (name: string) => void;
    onignore: () => void;
  } = $props();
</script>

<section class="card">
  <button class="samples" onclick={onopen} aria-label="See the whole photo">
    <img alt="" loading="lazy" decoding="async" src={faceUrl(tile.id, undefined, 320)} />
  </button>
  <div class="head"><span class="line">1 photo</span></div>
  <div class="actions">
    <ThisIs {people} onpick={onassign} onnew={onname} onignore={onignore} candidates={() => suggestForFaces([tile.id]).then(toCandidates)} />
  </div>
</section>

<style>
  .card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1px solid #d3d8e4; border-radius: 10px; background: #fafbfe; min-width: 0; }
  .samples { display: grid; grid-template-columns: 1fr; padding: 0; border: 0; background: none; cursor: pointer; }
  .samples img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 6px; background: #dde2ee; }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .line { color: #666; font-size: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
</style>
