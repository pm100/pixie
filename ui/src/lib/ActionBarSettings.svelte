<script lang="ts">
  /** Settings, Browsing: which buttons the action bars show. Anything not ticked is still under More. */
  import { BAR_ACTIONS, DEFAULT_BAR, toggled } from './actionBar';

  let { chosen, onchange }: { chosen: string[]; onchange: (next: string[]) => void } = $props();
</script>

<h3>Action bar</h3>
<p class="hint">The buttons beside Picked in Browse, and under a photo's details. Everything else is under More.</p>
<ul>
  {#each BAR_ACTIONS as a (a.id)}
    <li>
      <label class:soon={!a.built}>
        <input type="checkbox" checked={chosen.includes(a.id)} disabled={!a.built} onchange={(e) => onchange(toggled(chosen, a.id, e.currentTarget.checked))} />
        {a.label}{#if !a.built}<small>soon</small>{/if}
      </label>
    </li>
  {/each}
</ul>
<p><button onclick={() => onchange([...DEFAULT_BAR])} title="Put the action bar back to the standard set of buttons" disabled={chosen.join() === DEFAULT_BAR.join()}>Use the usual buttons</button></p>

<style>
  h3 { margin: 18px 0 4px; font-size: 15px; }
  .hint { color: #666; font-size: 13px; margin: 0 0 6px; }
  ul { list-style: none; margin: 0 0 8px; padding: 0; }
  li label { display: inline-flex; align-items: center; gap: 6px; padding: 2px 0; cursor: pointer; }
  li label.soon { color: #9a9aa4; cursor: default; }
  small { margin-left: 4px; font-size: 11px; color: #b0b0b8; }
</style>
