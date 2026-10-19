<script lang="ts">
  import { onMount } from 'svelte';

  let { title, message, confirmLabel, danger = false, onconfirm, oncancel }: {
    title: string; message: string; confirmLabel: string; danger?: boolean;
    onconfirm: () => void; oncancel: () => void;
  } = $props();

  let cancelBtn: HTMLButtonElement;
  onMount(() => cancelBtn.focus()); // the safe choice has the focus
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); oncancel(); } }} />

<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
  <div class="dlg" role="alertdialog" aria-modal="true" aria-labelledby="cd-title" aria-describedby="cd-msg">
    <h3 id="cd-title">{title}</h3>
    <p id="cd-msg">{message}</p>
    <div class="row">
      <button bind:this={cancelBtn} onclick={oncancel}>Cancel</button>
      <button class:danger onclick={onconfirm}>{confirmLabel}</button>
    </div>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 1500; display: grid; place-items: center; background: rgba(0, 0, 0, .4); }
  .dlg { width: min(420px, calc(100vw - 32px)); padding: 18px 20px; border-radius: 8px; background: #fff; color: #111; box-shadow: 0 8px 30px rgba(0, 0, 0, .35); }
  h3 { margin: 0 0 8px; font-size: 16px; }
  p { margin: 0 0 16px; color: #333; }
  .row { display: flex; justify-content: flex-end; gap: 8px; }
  .danger { color: #b00020; }
</style>
