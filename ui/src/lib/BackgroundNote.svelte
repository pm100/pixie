<script lang="ts">
  import FlowStage from './FlowStage.svelte';

  // Shown when the user chooses "Carry on in the background": what is still going on, how to tell when it has finished,
  // and what is worth doing meanwhile.
  let { scanning, onpeople, onbrowse }: { scanning: boolean; onpeople: () => void; onbrowse: () => void } = $props();
</script>

<FlowStage label="Pixie is still working">
  <h1>Pixie is still working</h1>
  <p class="sub">You can start using Pixie now. It carries on in the background, and what you see updates as it goes.</p>

  <h2>What is happening</h2>
  <ul>
    {#if scanning}<li><b>Reading your folders.</b> New photos and videos appear in Browse as they are found.</li>{/if}
    <li><b>Preparing previews.</b> Thumbnails fill in as you scroll.</li>
    <li><b>Understanding your photos.</b> Pixie adds Auto tags (labels such as Beach or Museum) as it goes, so the list on the left fills in.</li>
    <li><b>Finding faces.</b> People appear in the People tab as they are found.</li>
  </ul>

  <h2>How to tell when it has finished</h2>
  <p>Look at the progress indicator at the <b>top right</b> of the window. It shows what Pixie is doing and how far along it is, and it disappears when everything is done. Click it for details.</p>

  <h2>What to do next</h2>
  <ul>
    <li>Open the <b>People</b> tab to give the people Pixie finds their names. You can start before it has finished.</li>
    <li>Pick an <b>Auto tag</b> on the left in Browse to see all the photos with that label, for example Beach or Museum.</li>
  </ul>

  {#snippet footer()}
    <button onclick={onpeople} title="Go to the People tab, to name the faces found">Open People</button>
    <button class="primary" onclick={onbrowse}>Start browsing</button>
  {/snippet}
</FlowStage>

<style>
  h2 { margin: 16px 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; color: #777; }
  p { margin: 0 0 6px; }
  ul { margin: 0; padding-left: 20px; }
  li { margin: 4px 0; }
</style>
