<script lang="ts">
  /** The Ask Pixie tab: a short chat. Questions go to a server (that is said before anything is sent); the answer is text, and
   *  may offer a button to a place in the app or a change to settings, which happens only when Apply is pressed. */
  import { tick } from 'svelte';
  import { askPixie } from './api';
  import { buildRequest, dayOf, errorText, parseReply, platformName, type ChatMessage, type Quota } from './ask';
  import { goToLabel } from './askGoTo';
  import { applyChanges, checkProposal, type Change, type Handlers, type SettingValues } from './askSettings';

  let { on, installId, values, handlers, from, language, ongo, onturnon, ondecline, onsettings }: {
    /** The person has agreed to questions being sent. */
    on: boolean;
    /** The random id the proxy counts the day's questions by. */
    installId: string;
    values: SettingValues;
    handlers: Handlers;
    /** The tab the person came from (it helps the model answer "here"). */
    from: string;
    language: string;
    /** A button for a place in the app was pressed. */
    ongo: (target: string) => void;
    onturnon: () => void;
    ondecline: () => void;
    /** Opens the Ask Pixie page of Settings, where it is turned off and what is sent is listed. */
    onsettings: () => void;
  } = $props();

  const EXAMPLES = ['How do I email a video?', 'Stop looking for faces', 'How do I pick several photos at once?'];

  let history = $state<ChatMessage[]>([]);
  let text = $state('');
  let busy = $state(false);
  let quota = $state<Quota | null>(null);
  let log = $state<HTMLElement | null>(null);
  let box = $state<HTMLTextAreaElement | null>(null);

  async function scrollDown() {
    await tick();
    if (log) log.scrollTop = log.scrollHeight;
  }

  async function ask(question: string) {
    const q = question.trim();
    if (q === '' || busy) return;
    history.push({ role: 'user', text: q });
    text = '';
    busy = true;
    void scrollDown();
    try {
      const body = buildRequest(history, { platform: platformName(globalThis.navigator?.userAgent ?? ''), language, tab: from, today: dayOf(new Date()) }, values);
      const answer = await askPixie(installId, body);
      let json: unknown = null;
      try { json = JSON.parse(answer.body); } catch { /* not JSON: an error below */ }
      const reply = answer.status === 200 ? parseReply(json) : null;
      if (reply === null) {
        history.push({ role: 'error', text: errorText(answer.status, json) });
      } else {
        quota = reply.quota ?? quota;
        const checked = checkProposal(reply.proposal, values);
        let said = reply.reply.trim();
        if (said === '') said = reply.proposal.length > 0 && checked.changes.length === 0 ? "I couldn't work out a safe change. Try asking another way." : reply.feedbackSent ? "I've passed that on to the developer." : 'Done.';
        history.push({ role: 'assistant', text: said, changes: checked.changes.length > 0 ? checked.changes : undefined, card: checked.changes.length > 0 ? 'open' : undefined, goTo: reply.goTo ?? undefined });
      }
    } catch (e) {
      history.push({ role: 'error', text: String(e) });
    }
    busy = false;
    void scrollDown();
    box?.focus();
  }

  /** The card as it is now: a setting may have been changed by hand since it was proposed, so it is checked again. */
  const live = (m: ChatMessage): Change[] => checkProposal((m.changes ?? []).map((c) => ({ setting: c.key, value: c.value })), values).changes;

  function apply(m: ChatMessage) {
    const changes = live(m);
    if (changes.length === 0) { m.card = 'cancelled'; return; }
    applyChanges(changes, values, handlers);
    m.card = 'applied';
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); void ask(text); }
  }

  const low = $derived(quota !== null && quota.remaining <= 3);
</script>

<div class="page">
  <div class="col">
  {#if !on}
    <div class="intro">
      <h2>Ask Pixie</h2>
      <p>Ask how to do something in Pixie, or ask Pixie to change a setting for you.</p>
      <p><b>Your pictures never leave this computer.</b> To answer, Ask Pixie sends what you type, and some information about Pixie (its settings, your language and system) to a server, where a language model writes the reply. It never sends pictures, thumbnails, faces, the names of your tags, people or trips, file names or folder names.</p>
      <p>Nothing is changed until you press Apply. You can turn Ask Pixie off at any time.</p>
      <div class="row">
        <button class="primary" onclick={onturnon}>Turn on Ask Pixie</button>
        <button onclick={ondecline}>No thanks</button>
      </div>
    </div>
  {:else}
    <div class="chat" bind:this={log} role="log" aria-live="polite" aria-label="Conversation with Ask Pixie">
      {#if history.length === 0}
        <div class="empty">
          <p>Ask a question, or say what you want changed.</p>
          <div class="chips">
            {#each EXAMPLES as ex (ex)}<button class="chip" onclick={() => void ask(ex)} disabled={busy}>{ex}</button>{/each}
          </div>
        </div>
      {/if}
      {#each history as m, i (i)}
        <div class="msg {m.role}">
          <p>{m.text}</p>
          {#if m.changes && m.card}
            {@const lines = m.card === 'open' ? live(m) : m.changes}
            <div class="card">
              {#each lines as c (c.key)}<div class="line"><b>{c.label}</b>: {c.from} → {c.to}</div>{/each}
              {#if m.card === 'open'}
                {#if lines.length > 0}
                  <div class="row"><button class="primary" onclick={() => apply(m)}>Apply</button><button onclick={() => (m.card = 'cancelled')}>Cancel</button></div>
                {:else}
                  <div class="done">Already set.</div>
                {/if}
              {:else}
                <div class="done">{m.card === 'applied' ? 'Applied.' : 'Not applied.'}</div>
              {/if}
            </div>
          {/if}
          {#if m.goTo}<div class="row"><button onclick={() => ongo(m.goTo!)}>{goToLabel(m.goTo)}</button></div>{/if}
        </div>
      {/each}
      {#if busy}<div class="msg assistant"><p class="thinking">Thinking…</p></div>{/if}
    </div>
    <div class="entry">
      <textarea bind:this={box} bind:value={text} rows="2" maxlength="600" placeholder="Ask a question…" aria-label="Your question" onkeydown={onkey} disabled={busy}></textarea>
      <button class="primary" onclick={() => void ask(text)} disabled={busy || text.trim() === ''}>Ask</button>
    </div>
    <p class="note">Your pictures stay on this computer. Your questions are sent to a server. <button class="linkbtn" onclick={onsettings}>What is sent</button>{#if low && quota} · {quota.remaining} left today{/if}</p>
  {/if}
  </div>
</div>

<style>
  /* Fills the whole area over Browse, as the other tabs do; the chat is a centred column so lines stay readable. */
  .page { position: absolute; inset: 0; background: #fff; font-size: 14px; }
  .col { height: 100%; max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; box-sizing: border-box; }
  .intro { padding: 32px 28px; max-width: 560px; line-height: 1.5; }
  .intro h2 { margin: 0 0 12px; font-size: 18px; }
  .intro p { margin: 0 0 12px; }
  .chat { flex: 1; min-height: 0; overflow-y: auto; padding: 16px 20px; display: flex; flex-direction: column; gap: 10px; }
  .empty { color: #555; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
  .chip { border: 1px solid #c9ceda; background: #fff; border-radius: 14px; padding: 4px 12px; font: inherit; cursor: pointer; }
  .chip:hover:not(:disabled) { background: #eef2fb; }
  .msg { max-width: 85%; padding: 8px 12px; border-radius: 10px; line-height: 1.4; }
  .msg p { margin: 0; white-space: pre-wrap; }
  .msg.user { align-self: flex-end; background: #e3ecff; }
  .msg.assistant { align-self: flex-start; background: #f2f3f6; }
  .msg.error { align-self: flex-start; background: #fdecea; color: #7a1c14; }
  .thinking { color: #666; }
  .card { margin-top: 8px; padding: 8px 10px; background: #fff; border: 1px solid #dde1ea; border-radius: 8px; }
  .line { margin: 2px 0; }
  .done { color: #555; font-size: 13px; margin-top: 4px; }
  .row { display: flex; gap: 8px; margin-top: 8px; }
  .entry { display: flex; gap: 8px; padding: 8px 20px 0; align-items: flex-end; }
  .entry textarea { flex: 1; font: inherit; padding: 6px 8px; resize: none; box-sizing: border-box; }
  .note { margin: 6px 20px 12px; font-size: 12px; color: #666; }
  .linkbtn { border: 0; background: none; padding: 0; font: inherit; color: #2f6fed; cursor: pointer; text-decoration: underline; }
  button.primary { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  button.primary:disabled { opacity: .5; }
</style>
