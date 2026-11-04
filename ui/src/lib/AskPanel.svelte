<script lang="ts">
  /** The Ask Pixie panel, docked on the right and opened with the ? button or F1: a short chat. Questions go to a server (a line under the
   *  box says so); the answer is text, and may offer a button to a place in the app or a change to settings, which happens only when
   *  Apply is pressed. */
  import { tick } from 'svelte';
  import { askFeedback, askPixie } from './api';
  import { localHelp } from './askLocal';
  import { buildRequest, categoryLabel, dayOf, errorText, feedbackRequest, parseReply, platformName, quotaOf, type ChatMessage, type Quota } from './ask';
  import { goToLabel } from './askGoTo';
  import { applyChanges, checkProposal, type Change, type Handlers, type SettingValues } from './askSettings';

  let { on, installId, values, handlers, from, language, ongo, onsettings }: {
    /** Ask Pixie is turned on (Settings, Ask Pixie). */
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

  /** No answer from the server: answer from the help built into Pixie, so there is never a dead end. */
  function answerHere(q: string) {
    const help = localHelp(q);
    history.push(help
      ? { role: 'assistant', local: true, text: help.text, goTo: help.goTo ?? undefined }
      : { role: 'assistant', local: true, text: "I couldn't find that in Pixie's built-in help either. Try other words, or look along the tabs at the top: Browse, Send, Create, Organize, Clean up, Import/Export and Settings." });
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
        if (answer.status === 429) quota = quotaOf(json) ?? quota;
        history.push({ role: 'error', text: errorText(answer.status, json) });
        answerHere(q);
      } else {
        quota = reply.quota ?? quota;
        const checked = checkProposal(reply.proposal, values);
        let said = reply.reply.trim();
        if (said === '') said = reply.proposal.length > 0 && checked.changes.length === 0 ? "I couldn't work out a safe change. Try asking another way." : reply.feedback.length > 0 ? 'I can pass that on to the developer if you like.' : 'Done.';
        history.push({
          role: 'assistant', text: said, changes: checked.changes.length > 0 ? checked.changes : undefined, card: checked.changes.length > 0 ? 'open' : undefined, goTo: reply.goTo ?? undefined,
          notes: reply.feedback.length > 0 ? reply.feedback : undefined, noteCard: reply.feedback.length > 0 ? 'open' : undefined,
        });
      }
    } catch {
      history.push({ role: 'error', text: errorText(0, null) });
      answerHere(q);
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

  let sending = $state(false);
  let sendFailed = $state(false);

  /** The person read the notes and pressed Yes: each goes to the developer, and leaves the card once it has. */
  async function sendNotes(m: ChatMessage) {
    if (sending) return;
    sending = true;
    sendFailed = false;
    const platform = platformName(globalThis.navigator?.userAgent ?? '');
    try {
      while (m.notes && m.notes.length > 0) {
        const answer = await askFeedback(installId, feedbackRequest(m.notes[0], platform));
        if (answer.status !== 200) { sendFailed = true; break; }
        m.notes = m.notes.slice(1);
      }
      if (!sendFailed) m.noteCard = 'sent';
    } catch {
      sendFailed = true;
    }
    sending = false;
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); void ask(text); }
  }

  const low = $derived(quota !== null && quota.remaining <= 3);
</script>

<div class="page">
  {#if !on}
    <div class="intro">
      <h2>Ask Pixie</h2>
      <p>Ask Pixie is turned off, so no questions are sent.</p>
      <div class="row"><button class="primary" onclick={onsettings} title="Open the Ask Pixie page of Settings, where it is turned on and what is sent is listed">Open Settings</button></div>
    </div>
  {:else}
    <div class="chat" bind:this={log} role="log" aria-live="polite" aria-label="Conversation with Ask Pixie">
      {#if history.length === 0}
        <div class="empty">
          <p>Ask how to do something in Pixie, or say what you want changed.</p>
          <div class="chips">
            {#each EXAMPLES as ex (ex)}<button class="chip" onclick={() => void ask(ex)} disabled={busy} title={`Ask: ${ex}`}>{ex}</button>{/each}
          </div>
        </div>
      {/if}
      {#each history as m, i (i)}
        <div class="msg {m.role}">
          {#if m.local}<p class="from">From Pixie's built-in help</p>{/if}
          <p>{m.text}</p>
          {#if m.changes && m.card}
            {@const lines = m.card === 'open' ? live(m) : m.changes}
            <div class="card">
              {#each lines as c (c.key)}<div class="line"><b>{c.label}</b>: {c.from} → {c.to}</div>{/each}
              {#if m.card === 'open'}
                {#if lines.length > 0}
                  <div class="row"><button class="primary" onclick={() => apply(m)} title="Make these changes to Pixie's settings">Apply</button><button onclick={() => (m.card = 'cancelled')} title="Leave the settings as they are">Cancel</button></div>
                {:else}
                  <div class="done">Already set.</div>
                {/if}
              {:else}
                <div class="done">{m.card === 'applied' ? 'Applied.' : 'Not applied.'}</div>
              {/if}
            </div>
          {/if}
          {#if m.noteCard}
            <div class="card">
              <div class="line"><b>Pass this on to the developer?</b></div>
              {#each m.notes ?? [] as n (n.summary)}<div class="line note-text">{categoryLabel(n.category)}: {n.summary}</div>{/each}
              {#if m.noteCard === 'open'}
                <div class="done">Only this note goes, and the kind of computer you have (Windows or Mac). Nothing is sent unless you press Yes.</div>
                <div class="row">
                  <button class="primary" onclick={() => void sendNotes(m)} disabled={sending} title="Yes, send this note to the developer">Yes</button>
                  <button onclick={() => (m.noteCard = 'declined')} disabled={sending} title="Do not send anything to the developer">No</button>
                </div>
                {#if sendFailed}<div class="done">It could not be sent. Check your connection and press Yes to try again.</div>{/if}
              {:else}
                <div class="done">{m.noteCard === 'sent' ? 'Sent. Thank you.' : 'Not sent.'}</div>
              {/if}
            </div>
          {/if}
          {#if m.goTo}<div class="row"><button onclick={() => ongo(m.goTo!)} title="Go to that place in Pixie">{goToLabel(m.goTo)}</button></div>{/if}
        </div>
      {/each}
      {#if busy}<div class="msg assistant"><p class="thinking">Thinking…</p></div>{/if}
    </div>
    <div class="entry">
      <textarea bind:this={box} bind:value={text} rows="2" maxlength="600" placeholder="Ask a question…" aria-label="Your question" title="Type a question or a wish and press Enter" onkeydown={onkey} disabled={busy}></textarea>
      <button class="primary" onclick={() => void ask(text)} disabled={busy || text.trim() === ''} title="Send the question">Ask</button>
    </div>
    <p class="note">Your pictures stay on this computer. Your questions are sent to a server. <button class="linkbtn" onclick={onsettings} title="Open Settings to see what is sent, and to turn Ask Pixie off">What is sent</button>{#if low && quota} · {quota.remaining} left today{/if}</p>
  {/if}
</div>

<style>
  .page { height: 100%; display: flex; flex-direction: column; min-height: 0; background: #fff; font-size: 14px; box-sizing: border-box; }
  .intro { padding: 20px 16px; line-height: 1.5; }
  .intro h2 { margin: 0 0 12px; font-size: 16px; }
  .intro p { margin: 0 0 12px; }
  .chat { flex: 1; min-height: 0; overflow-y: auto; padding: 12px 12px; display: flex; flex-direction: column; gap: 10px; }
  .empty { color: #555; }
  .empty p { margin: 0 0 8px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { border: 1px solid #c9ceda; background: #fff; border-radius: 14px; padding: 4px 10px; font: inherit; font-size: 13px; cursor: pointer; text-align: left; }
  .chip:hover:not(:disabled) { background: #eef2fb; }
  .msg { max-width: 92%; padding: 8px 12px; border-radius: 10px; line-height: 1.4; }
  .msg p { margin: 0; white-space: pre-wrap; }
  .msg.user { align-self: flex-end; background: #e3ecff; }
  .msg.assistant { align-self: flex-start; background: #f2f3f6; }
  .msg.error { align-self: flex-start; background: #fdecea; color: #7a1c14; }
  .thinking { color: #666; }
  .from { font-size: 12px; color: #666; margin-bottom: 4px !important; }
  .card { margin-top: 8px; padding: 8px 10px; background: #fff; border: 1px solid #dde1ea; border-radius: 8px; }
  .line { margin: 2px 0; }
  .done { color: #555; font-size: 13px; margin-top: 4px; }
  .note-text { white-space: pre-wrap; }
  .row { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
  .entry { display: flex; gap: 8px; padding: 8px 12px 0; align-items: flex-end; }
  .entry textarea { flex: 1; min-width: 0; font: inherit; padding: 6px 8px; resize: none; box-sizing: border-box; }
  .note { margin: 6px 12px 10px; font-size: 12px; color: #666; }
  .linkbtn { border: 0; background: none; padding: 0; font: inherit; color: #2f6fed; cursor: pointer; text-decoration: underline; }
</style>
