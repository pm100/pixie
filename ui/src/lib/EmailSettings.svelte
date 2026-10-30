<script lang="ts">
  import { LIMIT_CHOICES_MB } from './emailSize';

  let { limitMb, onlimit }: { limitMb: number; onlimit: (mb: number) => void } = $props();
  // A limit set from elsewhere (after a message was too big) may not be one of the usual choices: it is listed too.
  const choices = $derived([...new Set([...LIMIT_CHOICES_MB, limitMb])].sort((a, b) => a - b));
</script>

<h2>Email</h2>
<p>Most email services limit the size of one message. When photos are bigger than the limit, Pixie offers to send them smaller or in several emails.</p>
<label class="row">Size limit of one message:
  <select value={limitMb} onchange={(e) => onlimit(Number(e.currentTarget.value))}>
    {#each choices as mb (mb)}<option value={mb}>{mb} MB</option>{/each}
  </select>
</label>
<p class="hint">Sending makes attachments about a third bigger, so photos totalling {Math.floor((limitMb * 3) / 4)} MB fit in a {limitMb} MB limit.</p>

<style>
  .row { display: flex; align-items: center; gap: 8px; margin: 0 0 8px; }
  .hint { color: #666; font-size: 13px; }
</style>
