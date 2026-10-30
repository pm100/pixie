<script lang="ts">
  /** The email form for some photos: what is being sent and whether it thinks that fits in an email, the subject (proposed),
   *  who to send it to and some text (both optional), and the ways to send (smaller copies, several emails, as it is). Used in
   *  the Send tab and, in a dialog, by the Email button. Pixie cannot tell when a message was refused; the person tries
   *  again and chooses another way. */
  import { emailInfo, emailPhotos } from './api';
  import { badAddress } from './emailTo';
  import { describeItems, mbText, tooBig } from './emailSize';
  import { createLatest } from './latest';

  let { ids, limitMb, oncancel, onsent }: {
    ids: number[];
    /** The size limit, in megabytes (Settings, Email). */
    limitMb: number;
    /** Given: the form has a Cancel button. */
    oncancel?: () => void;
    /** A message was opened (with a note for the person, or ''). */
    onsent?: (note: string) => void;
  } = $props();

  type Info = { count: number; photos: number; videos: number; total_bytes: number; largest_bytes: number; subject: string };
  let info = $state<Info | null>(null);
  let busy = $state(false);
  let error = $state('');
  let note = $state('');
  /** The way chosen. */
  let choice = $state<'asis' | 'smaller' | 'split'>('asis');
  /** What the person typed: the subject (starts as the one proposed), who to send it to, and some text. */
  let subject = $state('');
  let to = $state('');
  let body = $state('');
  /** What is wrong with the addresses, shown under the To box (set when Email is pressed, cleared when they are edited). */
  let toError = $state('');
  const tooLarge = $derived(info !== null && tooBig(info.total_bytes, limitMb));

  // Read again when the photos change: the proposed subject and the size depend on them.
  const latest = createLatest();
  $effect(() => {
    const list = ids;
    const token = latest.next();
    info = null;
    error = '';
    note = '';
    if (list.length === 0) return;
    emailInfo(list).then((i) => {
      if (!latest.isCurrent(token)) return;
      // Too big: the way that makes it fit is chosen to begin with. Otherwise: as it is.
      choice = !tooBig(i.total_bytes, limitMb) ? 'asis' : i.photos > 0 ? 'smaller' : i.count > 1 ? 'split' : 'asis';
      subject = i.subject;
      info = i;
    }).catch((e) => { if (latest.isCurrent(token)) error = String(e); });
  });

  async function send() {
    if (busy || !info) return;
    const bad = badAddress(to);
    if (bad !== null) {
      toError = `“${bad}” is not an email address.`;
      return;
    }
    error = '';
    note = '';
    busy = true;
    try {
      // Only a subject the person changed is sent: otherwise each message of a split gets its own (with its own dates).
      const r = await emailPhotos(ids, choice, limitMb, { subject: subject.trim() !== info.subject ? subject : null, to, body });
      if (r.emails > 1) note = `Opened ${r.emails} emails.`;
      else if (r.route === 'share' && (to.trim() || body.trim())) note = 'Your email program took only the photos: add the address and text there.';
      onsent?.(note);
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
</script>

<div class="form">
  {#if info}
    <h3>Sending {describeItems(info.photos, info.videos)}</h3>
    <p class="sum">
      {#if tooLarge}
        <b>{mbText(info.total_bytes)}</b> in all. Email services usually accept about {limitMb} MB in a message,
        and sending makes attachments about a third bigger.
      {:else}
        <b>{mbText(info.total_bytes)}</b> in all. We think {info.photos + info.videos === 1 ? 'it' : 'they'} will fit in a single email.
      {/if}
    </p>
    <div class="fields">
      <label>Subject<input type="text" bind:value={subject} maxlength="200" /></label>
      <label>To (optional)<input type="text" bind:value={to} oninput={() => (toError = '')} placeholder="name@example.com, another@example.com" autocomplete="off" aria-invalid={toError ? true : undefined} aria-describedby={toError ? 'to-error' : undefined} />{#if toError}<span id="to-error" class="err" role="alert">{toError}</span>{/if}</label>
      <label>Message (optional)<textarea rows="3" bind:value={body} maxlength="5000"></textarea></label>
    </div>
    <fieldset class="options">
      <legend class="sr">How to send them</legend>
      {#if info.photos > 0}
        <label class="opt" class:on={choice === 'smaller'}>
          <input type="radio" name="how" value="smaller" bind:group={choice} />
          <span class="body"><span class="t">Make them smaller</span>
            <span class="d">Sends smaller copies of the pictures (2048 pixels on the longest side, the whole picture, nothing cropped). <b>Your originals remain untouched.</b>{#if info.videos > 0} Videos are sent as they are.{/if}</span></span>
        </label>
      {/if}
      {#if info.count > 1}
        <label class="opt" class:on={choice === 'split'}>
          <input type="radio" name="how" value="split" bind:group={choice} />
          <span class="body"><span class="t">Send in several emails</span>
            <span class="d">As many emails as it takes, each under the limit, in date order. The files are sent as they are.</span></span>
        </label>
      {/if}
      <label class="opt" class:on={choice === 'asis'}>
        <input type="radio" name="how" value="asis" bind:group={choice} />
        <span class="body"><span class="t">Send as is</span>
          <span class="d">One email with everything as it is.{#if tooLarge} It may be refused.{/if}</span></span>
      </label>
    </fieldset>
    <div class="foot">
      {#if oncancel}<button onclick={oncancel}>Cancel</button>{/if}
      <button class="primary" onclick={send} disabled={busy}>{busy ? 'Opening…' : 'Email'}</button>
    </div>
  {:else if ids.length === 0}
    <p class="sum">Nothing to send.</p>
  {:else if !error}
    <p class="sum">Reading the photos…</p>
  {/if}
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .form { font-size: 13.5px; }
  h3 { margin: 0 0 6px; font-size: 15px; }
  .sum { margin: 0 0 12px; color: #444; }
  .note { font-size: 12px; color: #555; margin: 8px 0 0; }
  .err { font-size: 12px; color: #b00020; margin: 8px 0 0; }
  .fields { display: flex; flex-direction: column; gap: 8px; margin: 0 0 12px; }
  .fields label { display: flex; flex-direction: column; gap: 2px; color: #444; }
  .fields input, .fields textarea { font: inherit; padding: 4px 6px; box-sizing: border-box; width: 100%; resize: vertical; }
  .options { border: 0; margin: 0 0 12px; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
  .opt { display: flex; align-items: flex-start; gap: 10px; padding: 8px 10px; border: 1px solid #dde1ea; border-radius: 8px; cursor: pointer; }
  .opt:hover { background: #f6f8fc; }
  .opt.on { border-color: #2f6fed; background: #eef3ff; }
  .opt input { margin: 2px 0 0; flex: none; }
  .body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .t { font-weight: 600; }
  .d { color: #555; font-size: 12.5px; line-height: 1.35; }
  .foot { display: flex; justify-content: flex-end; gap: 8px; }
</style>
