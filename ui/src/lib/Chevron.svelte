<script lang="ts">
  // The fold/unfold arrow in front of a heading: points right when folded, down when open.
  let { open }: { open: boolean } = $props();
</script>

<svg class="chev" class:open width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
  <path d="M5.5 3.5 10.5 8 5.5 12.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
</svg>

<style>
  .chev { flex: none; color: #666; transition: transform .1s; }
  .chev.open { transform: rotate(90deg); }
</style>
