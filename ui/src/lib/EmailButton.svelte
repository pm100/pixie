<script lang="ts">
  /** The Email button for some photos, used by the Selected tab and the details pane. It always asks how to send first: it says
   *  what is being sent, whether it thinks that fits in an email, and offers the ways (smaller copies, several emails, as it
   *  is). Pixie cannot tell when a message was refused; the person tries again and chooses another way. */
  import { emailInfo, emailPhotos } from './api';
  import { describeItems, mbText, tooBig } from './emailSize';

  let { ids, limitMb }: {
    ids: number[];
    /** The size limit, in megabytes (Settings, Email). */
    limitMb: number;
  } = $props();

  type Info = { count: number; photos: number; videos: number; total_bytes: number; largest_bytes: number };
  let busy = $state(false);
  let error = $state('');
  let note = $state('');
  /** What is to be sent, while the choices are on screen. */
  let asking = $state<Info | null>(null);
  /** The way chosen in the dialog. */
  let choice = $state<'asis' | 'smaller' | 'split'>('asis');
  const tooLarge = $derived(asking !== null && tooBig(asking.total_bytes, limitMb));

  async function start() {
    if (busy || ids.length === 0) return;
    error = '';
    note = '';
    busy = true;
    try {
      const info = await emailInfo(ids);
      // Too big: the way that makes it fit is chosen to begin with. Otherwise: as it is.
      choice = !tooBig(info.total_bytes, limitMb) ? 'asis' : info.photos > 0 ? 'smaller' : info.count > 1 ? 'split' : 'asis';
      asking = info;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  async function send() {
    asking = null;
    busy = true;
    try {
      const r = await emailPhotos(ids, choice, limitMb);
      if (r.emails > 1) note = `Opened ${r.emails} emails.`;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
</script>

<div class="email">
  <button onclick={start} disabled={busy || ids.length === 0} title="Open a new email with the photo or photos attached">{busy ? 'Opening…' : 'Email'}</button>
  {#if note}<span class="note" role="status">{note}</span>{/if}
  {#if error}<span class="err" role="alert">{error}</span>{/if}
</div>

{#if asking}
  {@const info = asking}
  <div class="scrim" role="presentation" onclick={() => (asking = null)}></div>
  <div class="dlg" role="dialog" aria-modal="true" aria-label="Email">
    <h3>Sending {describeItems(info.photos, info.videos)}</h3>
    <p class="sum">
      {#if tooLarge}
        <b>{mbText(info.total_bytes)}</b> in all. Email services usually accept about {limitMb} MB in a message,
        and sending makes attachments about a third bigger.
      {:else}
        <b>{mbText(info.total_bytes)}</b> in all. We think they will fit in an email.
      {/if}
    </p>
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
      <button onclick={() => (asking = null)}>Cancel</button>
      <button class="primary" onclick={send}>Email</button>
    </div>
  </div>
{/if}

<style>
  .email { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .note { font-size: 12px; color: #555; }
  .err { font-size: 12px; color: #b00020; }
  .scrim { position: fixed; inset: 0; z-index: 2400; background: rgba(0, 0, 0, .35); }
  .dlg { position: fixed; z-index: 2500; left: 50%; top: 50%; transform: translate(-50%, -50%); width: min(440px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow-y: auto; box-sizing: border-box;
         padding: 16px 18px 14px; border-radius: 10px; background: #fff; color: #222; box-shadow: 0 10px 40px rgba(0, 0, 0, .3); font-size: 13.5px; }
  h3 { margin: 0 0 6px; font-size: 15px; }
  .sum { margin: 0 0 12px; color: #444; }
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
  .foot button.primary { background: #2f6fed; border-color: #2f6fed; color: #fff; }
</style>
