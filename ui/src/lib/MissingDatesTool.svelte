<script lang="ts">
  import { dateSummary, type DateSummary } from './api';
  import { ODD_YEARS, withBeforeYear, withOddYear, type DateRules } from './dateRules';
  import { plural } from './format';

  let { rules, onrules, active, onshow }: {
    rules: DateRules; onrules: (r: DateRules) => void;
    /** This tool is the one on show: its numbers are read again when it opens. */
    active: boolean;
    /** Show the items without a date in Browse. */
    onshow: () => void;
  } = $props();

  const WHERE: Record<number, string> = {
    1904: 'where QuickTime and many video files start counting',
    1970: 'where Unix time starts',
    1980: 'where DOS time starts',
    2000: 'the date many older cameras show until their clock is set',
  };

  // What the rules do to the library. Read when the tool opens and after each change (the change is saved and applied before `rules` is replaced).
  let summary = $state<DateSummary | null>(null);
  let error = $state('');
  $effect(() => {
    rules;
    if (!active) return;
    let live = true;
    dateSummary()
      .then((s) => { if (live) { summary = s; error = ''; } })
      .catch((e) => { if (live) error = String(e); });
    return () => { live = false; };
  });
</script>

<div class="tool">
<h2>Missing dates</h2>
<p>A camera whose clock was never set stamps its pictures with a date that means nothing, such as 1 January 2000. Choose which dates Pixie treats as missing, and what to show instead.</p>
<p class="keep">This changes only how Pixie shows your photos and videos. Your files are never changed.</p>

<h3>Dates that count as missing</h3>
{#each ODD_YEARS as y (y)}
  <label class="opt"><input type="checkbox" checked={rules.odd_years.includes(y)} onchange={(e) => onrules(withOddYear(rules, y, e.currentTarget.checked))} /> 1 January {y} <span class="hint">({WHERE[y]})</span></label>
{/each}
<label class="row">Dates before this year also count as missing (leave blank for none):
  <input class="year" type="number" min="1826" step="1" value={rules.before_year ?? ''} placeholder="none"
         onchange={(e) => { const next = withBeforeYear(rules, e.currentTarget.value); onrules(next); e.currentTarget.value = next.before_year === null ? '' : String(next.before_year); }} />
</label>

<h3>When a photo has no usable date</h3>
<p>Pixie can show the date of the file itself, which is when it was last changed. This is a guess: a file copied or exported later has a later date. A file date that is itself one of the missing dates above is never used.</p>
<label class="opt"><input type="checkbox" checked={rules.file_when_none} onchange={(e) => onrules({ ...rules, file_when_none: e.currentTarget.checked })} /> Use the file's date when the photo has no date inside it</label>
<label class="opt"><input type="checkbox" checked={rules.file_when_odd} onchange={(e) => onrules({ ...rules, file_when_odd: e.currentTarget.checked })} /> Use the file's date when the date inside counts as missing</label>
<p class="hint">A change applies to your whole library at once, and to every folder you add later.</p>
<h3>With these rules</h3>
{#if error}
  <p class="err">{error}</p>
{:else if summary === null}
  <p class="hint">Counting…</p>
{:else if summary.odd + summary.none_inside === 0}
  <p>Every one of your {plural(summary.total, 'photo and video', 'photos and videos')} has a date.</p>
{:else}
  <ul class="facts">
    {#if summary.odd > 0}<li><b>{summary.odd.toLocaleString()}</b> {summary.odd === 1 ? 'has' : 'have'} a date that counts as missing.</li>{/if}
    {#if summary.none_inside > 0}<li><b>{summary.none_inside.toLocaleString()}</b> {summary.none_inside === 1 ? 'has' : 'have'} no date inside {summary.none_inside === 1 ? 'it' : 'them'}.</li>{/if}
    <li><b>{summary.from_file.toLocaleString()}</b> of those show the file's date instead.</li>
  </ul>
  <div class="leftover">
    <span><b>{summary.undated.toLocaleString()}</b> {summary.undated === 1 ? 'is' : 'are'} left with no date.</span>
    {#if summary.undated > 0}<button onclick={onshow} title="Show only the photos and videos left with no date, in Browse">Show them in Browse</button>{/if}
  </div>
{/if}
</div>

<style>
  .tool { padding: var(--page-pad); }
  h2 { margin: 0 0 8px; font-size: 18px; }
  h3 { margin: 16px 0 6px; font-size: 14px; }
  .opt { display: block; margin: 4px 0; }
  .hint { color: #666; font-size: 13px; }
  .err { color: #b00020; }
  .keep { background: #eef6ee; border-left: 3px solid #2a7a2a; padding: 6px 10px; border-radius: 0 4px 4px 0; max-width: 60ch; }
  .row { display: flex; align-items: center; gap: 8px; margin: 8px 0; flex-wrap: wrap; }
  .year { width: 6em; font: inherit; padding: 2px 4px; }
  .facts { margin: 0 0 8px; padding-left: 20px; }
  .leftover { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
</style>
