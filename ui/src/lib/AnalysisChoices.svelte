<script lang="ts">
  /** The three background analyses, each with a check box: used by the welcome screen and by Settings, Picture analysis. */
  import type { AnalysisOptions } from './analysisOptions';

  let { options, onchange }: { options: AnalysisOptions; onchange: (next: AnalysisOptions) => void } = $props();

  const set = (key: keyof AnalysisOptions, on: boolean) => onchange({ ...options, [key]: on });
</script>

<div class="choices">
  <label class="choice">
    <input type="checkbox" checked={options.faces} onchange={(e) => set('faces', e.currentTarget.checked)} />
    <span><b>People</b> <span class="d">Finds the faces in your photos, so you can name the people in them.</span></span>
  </label>
  <label class="choice">
    <input type="checkbox" checked={options.tags} onchange={(e) => set('tags', e.currentTarget.checked)} />
    <span><b>Auto tags</b> <span class="d">Labels each photo by what it shows, such as Beach or Museum, so you can browse by them.</span></span>
  </label>
  <label class="choice">
    <input type="checkbox" checked={options.dupes} onchange={(e) => set('dupes', e.currentTarget.checked)} />
    <span><b>Duplicates</b> <span class="d">Finds files that are copies of each other, and shows just one of them.</span></span>
  </label>
</div>

<style>
  .choices { display: grid; gap: 8px; margin: 6px 0 8px; }
  .choice { display: flex; align-items: flex-start; gap: 10px; cursor: pointer; line-height: 1.35; }
  .choice input { margin: 3px 0 0; flex: none; }
  .d { color: #555; font-size: 13px; }
</style>
