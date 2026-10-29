<script lang="ts">
  /** The Ask Pixie tab: a short chat. Questions go to a server (that is said before anything is sent); the answer is text, and
   *  may offer a button to a place in the app or a change to settings, which happens only when Apply is pressed. */
  import { tick } from 'svelte';
  import { askPixie, findPhotos, listTagged, photosByIds } from './api';
  import { buildRequest, dayOf, errorText, parseReply, platformName, type ChatMessage, type Quota, type Reply } from './ask';
  import { answerLookup, MAX_ROUNDS, type Round } from './askLookup';
  import type { Library } from './askLibrary';
  import { CUT_FLOOR, defaultCut, describeFind, resolveFind, visibleIds, type FindSpec, type Known } from './askFind';
  import { plural } from './format';
  import { thumbUrl } from './url';
  import { goToLabel } from './askGoTo';
  import { applyChanges, checkProposal, type Change, type Handlers, type SettingValues } from './askSettings';

  let { on, installId, values, handlers, from, language, known, ongo, onturnon, ondecline, onsettings, getLibrary, onresults, onopen, onpick, ontag }: {
    /** The person has agreed to questions being sent. */
    on: boolean;
    /** The random id the proxy counts the day's questions by. */
    installId: string;
    values: SettingValues;
    handlers: Handlers;
    /** The tab the person came from (it helps the model answer "here"). */
    from: string;
    language: string;
    /** The names of the tags and people the person has, to match what the model names against (they are never sent anywhere). */
    known: Known;
    /** The names in the library, when the person has allowed Ask Pixie to see them (null: not allowed). Read at each question. */
    getLibrary: () => Library | null;
    /** Make these photos Ask Pixie's results (listed in Tools, Working on). */
    onresults: (ids: number[]) => void;
    /** Open the Tools tab on Ask Pixie's results, at this tool. */
    onopen: (tool: 'details' | 'email' | 'print') => void;
    /** Pick these photos. */
    onpick: (ids: number[]) => Promise<void>;
    /** Give these photos a tag (or, with undo, take it off them); resolves with the name it is stored under. */
    ontag: (ids: number[], tag: string, undo?: boolean) => Promise<string>;
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
  /** What is going on while a question is being answered, when it is more than waiting. */
  let activity = $state('');
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
      // The model may ask the app something before it answers (how many photos match, is there a person called Rose); the app
      // answers on this computer and asks again with the answer, a few times at most.
      const rounds: Round[] = [];
      let reply: Reply | null = null;
      let json: unknown = null;
      let status = 0;
      for (;;) {
        const library = getLibrary();
        const body = buildRequest(history, { platform: platformName(globalThis.navigator?.userAgent ?? ''), language, tab: from, today: dayOf(new Date()) }, values, library, rounds);
        const answer = await askPixie(installId, body);
        status = answer.status;
        json = null;
        try { json = JSON.parse(answer.body); } catch { /* not JSON: an error below */ }
        reply = answer.status === 200 ? parseReply(json) : null;
        if (reply === null || reply.lookup.length === 0 || rounds.length >= MAX_ROUNDS) break;
        activity = 'Looking things up…';
        const deps = { library, known, find: (texts: string[], spec: FindSpec) => findPhotos(texts, spec) };
        const asked = reply.lookup;
        rounds.push({ calls: asked.map((l) => ({ id: l.id, input: l.input })), results: await Promise.all(asked.map((l) => answerLookup(l, deps))) });
      }
      activity = '';
      if (reply === null) {
        history.push({ role: 'error', text: errorText(status, json) });
      } else {
        quota = reply.quota ?? quota;
        const checked = checkProposal(reply.proposal, values);
        let said = reply.reply.trim();
        if (said === '') said = reply.proposal.length > 0 && checked.changes.length === 0 ? "I couldn't work out a safe change. Try asking another way." : reply.feedbackSent ? "I've passed that on to the developer." : 'Done.';
        history.push({ role: 'assistant', text: said, changes: checked.changes.length > 0 ? checked.changes : undefined, card: checked.changes.length > 0 ? 'open' : undefined, goTo: reply.goTo ?? undefined });
        if (reply.find) {
          history[history.length - 1].found = { args: reply.find, status: 'running', problems: [], ids: [], allIds: [], scores: null, cut: CUT_FLOOR, preview: [], analysed: null, done: {} };
          void runFind(history[history.length - 1]);
        }
      }
    } catch (e) {
      history.push({ role: 'error', text: String(e) });
    }
    busy = false;
    activity = '';
    void scrollDown();
    box?.focus();
  }

  /** Runs the search the model asked for. It only reads, so it needs no button; what changes anything waits for one. */
  async function runFind(m: ChatMessage) {
    const f = m.found!;
    const r = resolveFind(f.args, known);
    if (!r.runnable) {
      f.problems = r.problems.length > 0 ? r.problems : ['I could not tell what to look for.'];
      f.status = 'problem';
      return;
    }
    try {
      const res = await findPhotos(r.texts, r.spec);
      f.allIds = res.ids;
      f.scores = res.scores;
      f.cut = defaultCut(res.scores ?? [], f.args.limit);
      f.analysed = res.analysed;
      f.status = 'done';
      await shown(f);
    } catch (e) {
      f.problems = [String(e)];
      f.status = 'failed';
    }
    void scrollDown();
  }

  /** Brings the photos shown and the pictures on the card in step with the cut. */
  let shownRun = 0;
  async function shown(f: NonNullable<ChatMessage['found']>) {
    const mine = ++shownRun;
    f.ids = visibleIds(f.allIds, f.scores, f.cut);
    onresults(f.ids);
    const preview = f.ids.length > 0 ? await photosByIds(f.ids.slice(0, 8)) : [];
    if (mine === shownRun) f.preview = preview;
  }

  let working = $state(false);
  async function pick(m: ChatMessage) {
    const f = m.found!;
    if (working) return;
    working = true;
    try { await onpick(f.ids); f.done.picked = f.ids.length; } catch (e) { history.push({ role: 'error', text: String(e) }); }
    working = false;
  }
  async function tag(m: ChatMessage, name: string, undo = false) {
    const f = m.found!;
    if (working) return;
    working = true;
    try {
      if (undo) {
        const t = f.done.tagged;
        if (t) { await ontag(t.ids, t.tag, true); f.done.tagged = undefined; }
      } else {
        // Only the photos that did not have the tag are tagged here, so taking it off again leaves the others as they were.
        const had = new Set((await listTagged(name).catch(() => [])).map((p) => p.id));
        const fresh = f.ids.filter((id) => !had.has(id));
        const stored = await ontag(f.ids, name);
        f.done.tagged = { tag: stored, ids: fresh };
      }
    } catch (e) { history.push({ role: 'error', text: String(e) }); }
    working = false;
  }
  function openIn(m: ChatMessage, tool: 'details' | 'email' | 'print') {
    onresults(m.found!.ids);
    m.found!.done.opened = tool;
    onopen(tool);
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
      <p><b>Your pictures never leave this computer.</b> To answer, Ask Pixie sends what you type, and some information about Pixie (its settings, your language and system) to a server, where a language model writes the reply. It never sends pictures, thumbnails, faces, file names, folder names, or the names of your tags and people.</p>
      <p>Nothing is changed until you press Apply. In Settings, Ask Pixie, you can also let it see the names of your tags, people and trips, which makes its answers better, or leave that off. You can turn Ask Pixie off at any time.</p>
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
          {#if m.found}
            {@const f = m.found}
            <div class="card">
              {#if f.status === 'running'}
                <div class="done">Looking for {describeFind(f.args)}…</div>
              {:else if f.status !== 'done'}
                {#each f.problems as p (p)}<div class="line">{p}</div>{/each}
                {#if f.status === 'problem'}<div class="done">Nothing was searched. Try again with the name as it is in Pixie.</div>{/if}
              {:else}
                <div class="line"><b>{f.ids.length === 0 ? 'Nothing found' : `Found ${plural(f.ids.length, f.args.kind === 'video' ? 'video' : 'photo')}`}</b> — {describeFind(f.args)}</div>
                {#if f.analysed !== null}<div class="done">Looking at what is in the pictures is approximate, and covers only the {plural(f.analysed, 'photo')} analysed so far (videos are not included).</div>{/if}
                {#if f.scores !== null && f.allIds.length > 1 && f.done.picked === undefined && !f.done.tagged}
                  <label class="cut">More <input type="range" min={CUT_FLOOR} max={f.scores[0]} step="0.002" bind:value={f.cut} oninput={() => void shown(f)} aria-label="How many matches to keep: more or fewer" /> Fewer</label>
                {/if}
                {#if f.args.withPeople}<div class="done">“With people” uses the faces Pixie has found so far, so photos not yet scanned for faces are left out.</div>{/if}
                {#if f.preview.length > 0}
                  <div class="thumbs">{#each f.preview as p (p.id)}<img alt="" src={thumbUrl('grid', p.id, p.mtime, p.size)} />{/each}{#if f.ids.length > f.preview.length}<span class="more">+{(f.ids.length - f.preview.length).toLocaleString()}</span>{/if}</div>
                {/if}
                {#if f.ids.length > 0}
                  <div class="row">
                    <button onclick={() => openIn(m, 'details')}>Look at them</button>
                    {#if f.args.then.includes('pick') && f.done.picked === undefined}<button class="primary" disabled={working} onclick={() => void pick(m)}>Pick {plural(f.ids.length, f.args.kind === 'video' ? 'video' : 'photo')}</button>{/if}
                    {#if f.args.then.includes('tag') && f.args.tag && !f.done.tagged}<button class="primary" disabled={working} onclick={() => void tag(m, f.args.tag!)}>Tag {plural(f.ids.length, f.args.kind === 'video' ? 'video' : 'photo')} “{f.args.tag}”</button>{/if}
                    {#if f.args.then.includes('email')}<button onclick={() => openIn(m, 'email')}>Email them</button>{/if}
                    {#if f.args.then.includes('print')}<button onclick={() => openIn(m, 'print')}>Print them</button>{/if}
                  </div>
                  {#if f.done.picked !== undefined}<div class="done">Picked {plural(f.done.picked, f.args.kind === 'video' ? 'video' : 'photo')}.</div>{/if}
                  {#if f.done.tagged}<div class="done">Tagged {plural(f.ids.length, f.args.kind === 'video' ? 'video' : 'photo')} “{f.done.tagged.tag}”. <button class="linkbtn" disabled={working} onclick={() => void tag(m, f.done.tagged!.tag, true)}>Undo</button></div>{/if}
                {/if}
              {/if}
            </div>
          {/if}
          {#if m.goTo}<div class="row"><button onclick={() => ongo(m.goTo!)}>{goToLabel(m.goTo)}</button></div>{/if}
        </div>
      {/each}
      {#if busy}<div class="msg assistant"><p class="thinking">{activity || 'Thinking…'}</p></div>{/if}
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
  .thumbs { display: flex; gap: 4px; align-items: center; margin: 6px 0; }
  .thumbs img { width: 56px; height: 56px; object-fit: cover; border-radius: 4px; background: #e8e8ec; }
  .more { font-size: 13px; color: #555; padding-left: 4px; }
  .cut { display: flex; align-items: center; gap: 8px; font-size: 13px; color: #555; margin: 4px 0; }
  .cut input { flex: 1; }
  .row { display: flex; gap: 8px; margin-top: 8px; }
  .entry { display: flex; gap: 8px; padding: 8px 20px 0; align-items: flex-end; }
  .entry textarea { flex: 1; font: inherit; padding: 6px 8px; resize: none; box-sizing: border-box; }
  .note { margin: 6px 20px 12px; font-size: 12px; color: #666; }
  .linkbtn { border: 0; background: none; padding: 0; font: inherit; color: #2f6fed; cursor: pointer; text-decoration: underline; }
  button.primary { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  button.primary:disabled { opacity: .5; }
</style>
