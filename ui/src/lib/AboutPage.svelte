<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import AppIcon from './AppIcon.svelte';
  import { buildInfo } from './api';
  import Licences from './Licences.svelte';
  import { buildLine } from './licences';

  let version = $state('');
  let build = $state('');
  let showLicences = $state(false);
  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {});
    buildInfo().then((b) => (build = buildLine(b))).catch(() => {});
  });
</script>

{#if showLicences}
  <Licences onback={() => (showLicences = false)} />
{:else}
  <div class="head">
    <AppIcon size={128} />
    <div>
      <div class="title"><b>Pixie</b> {version}</div>
      {#if build}<div class="build">{build}</div>{/if}
    </div>
  </div>
  <p>A photo and video manager designed to be powerful but easy to use.</p>
  <p>Copyright © 2026 JollyGoodSW LLC and Paul Moore. Released under the MIT licence.</p>
  <p><button onclick={() => (showLicences = true)}>Open-source software in Pixie…</button></p>
{/if}

<style>
  .head { display: flex; align-items: center; gap: 20px; margin: 4px 0 16px; }
  .title { font-size: 32px; line-height: 1.2; }
  .build { color: #666; font-size: 13px; user-select: text; }
</style>
