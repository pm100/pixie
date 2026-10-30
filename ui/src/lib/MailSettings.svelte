<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    deleteMailAccount, getMailAccount, listProviders, saveMailAccount, sendTestEmail,
    type MailAccount, type MailSecurity, type Provider,
  } from './api';

  let { onclose }: { onclose: () => void } = $props();

  let providers = $state<Provider[]>([]);
  let loaded = $state(false);
  let hasPassword = $state(false);
  let providerId = $state('gmail');
  let email = $state('');
  let displayName = $state('');
  let host = $state('');
  let port = $state(587);
  let security = $state<MailSecurity>('starttls');
  let limitMb = $state(20);
  let password = $state(''); // cleared after saving and when the dialog closes
  let busy = $state<'' | 'saving' | 'testing'>('');
  let result = $state<{ ok: boolean; text: string } | null>(null);
  let confirmRemove = $state(false);

  const provider = $derived(providers.find((p) => p.id === providerId));
  const isOther = $derived(providerId === 'other');

  function applyPreset(id: string) {
    providerId = id;
    const p = providers.find((x) => x.id === id);
    if (p && id !== 'other') {
      host = p.host; port = p.port; security = p.security; limitMb = p.limit_mb;
    }
  }

  onMount(async () => {
    try {
      providers = await listProviders();
      const info = await getMailAccount();
      hasPassword = info.has_password;
      if (info.account) {
        const a = info.account;
        providerId = a.provider; email = a.email; displayName = a.display_name ?? '';
        host = a.host; port = a.port; security = a.security; limitMb = a.limit_mb;
      } else {
        applyPreset('gmail');
      }
    } catch (e) {
      result = { ok: false, text: String(e) };
    }
    loaded = true;
  });

  onDestroy(() => { password = ''; });

  const current = (): MailAccount => ({
    display_name: displayName.trim() || null, email: email.trim(), provider: providerId,
    host: host.trim(), port: Number(port), security, limit_mb: Number(limitMb),
  });

  async function save(): Promise<boolean> {
    busy = 'saving';
    try {
      await saveMailAccount(current(), password || null);
      hasPassword = hasPassword || password !== '';
      password = '';
      result = { ok: true, text: 'Saved.' };
      return true;
    } catch (e) {
      result = { ok: false, text: String(e) };
      return false;
    } finally {
      busy = '';
    }
  }

  async function test() {
    if (!(await save())) return;
    busy = 'testing';
    result = { ok: true, text: 'Sending a test email to yourself…' };
    try {
      await sendTestEmail();
      result = { ok: true, text: `Sent. Check the inbox of ${email.trim()}.` };
    } catch (e) {
      result = { ok: false, text: String(e) };
    } finally {
      busy = '';
    }
  }

  async function remove() {
    try {
      await deleteMailAccount();
      hasPassword = false; email = ''; displayName = ''; password = ''; confirmRemove = false;
      applyPreset('gmail');
      result = { ok: true, text: 'The account and its saved password were removed.' };
    } catch (e) {
      result = { ok: false, text: String(e) };
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={key} />
<div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="dlg" role="dialog" aria-label="Email settings" aria-modal="true">
    <h2>Email settings</h2>
    <p class="lead">Pixie sends photos through your own email account. Your password is kept in the system password store, not in a file.</p>

    {#if !loaded}
      <p>Loading…</p>
    {:else}
      <label for="mail-provider">Mail provider</label>
      <select id="mail-provider" value={providerId} onchange={(e) => applyPreset(e.currentTarget.value)}>
        {#each providers as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
      </select>
      {#if provider}<p class="hint">{provider.hint}</p>{/if}

      <label for="mail-email">Your email address</label>
      <input id="mail-email" type="email" autocomplete="off" bind:value={email} placeholder="you@example.com" />

      <label for="mail-name">Your name (shown to recipients, optional)</label>
      <input id="mail-name" type="text" autocomplete="off" bind:value={displayName} />

      <label for="mail-password">Password</label>
      <input id="mail-password" type="password" autocomplete="off" bind:value={password}
             placeholder={hasPassword ? 'Saved. Type a new one to replace it.' : 'App password'} />

      {#if isOther}
        <div class="row">
          <div class="grow"><label for="mail-host">Mail server</label><input id="mail-host" type="text" bind:value={host} placeholder="smtp.example.com" /></div>
          <div><label for="mail-port">Port</label><input id="mail-port" type="number" min="1" max="65535" bind:value={port} /></div>
        </div>
        <div class="row">
          <div><label for="mail-sec">Security</label>
            <select id="mail-sec" bind:value={security}>
              <option value="starttls">STARTTLS (usually port 587)</option>
              <option value="ssl">SSL (usually port 465)</option>
            </select></div>
          <div><label for="mail-limit">Size limit (MB)</label><input id="mail-limit" type="number" min="1" max="100" bind:value={limitMb} /></div>
        </div>
      {:else}
        <p class="hint">Sends through {host}, port {port} ({security === 'ssl' ? 'SSL' : 'STARTTLS'}). Messages up to {limitMb} MB.</p>
      {/if}

      {#if result}<p class="result" class:bad={!result.ok} role="status">{result.text}</p>{/if}

      <div class="buttons">
        <button class="primary" onclick={save} disabled={busy !== ''}>Save</button>
        <button onclick={test} disabled={busy !== '' || email.trim() === '' || (!hasPassword && password === '')}>
          {busy === 'testing' ? 'Sending…' : 'Send me a test email'}
        </button>
        <span class="spacer"></span>
        {#if hasPassword || email}
          {#if confirmRemove}
            <button class="danger" onclick={remove}>Yes, remove</button>
            <button onclick={() => (confirmRemove = false)}>Keep</button>
          {:else}
            <button onclick={() => (confirmRemove = true)}>Remove account</button>
          {/if}
        {/if}
        <button onclick={onclose}>Close</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 3000; background: rgba(0, 0, 0, .45); display: grid; place-items: center; padding: 16px; }
  .dlg { background: #fff; color: #222; border-radius: 10px; padding: 20px 22px; width: min(520px, 100%); max-height: 92vh; overflow: auto; box-shadow: 0 10px 40px rgba(0, 0, 0, .4); }
  h2 { margin: 0 0 6px; font-size: 18px; }
  .lead { margin: 0 0 12px; color: #555; font-size: 13px; }
  label { display: block; margin: 10px 0 3px; font-size: 13px; font-weight: 600; }
  input, select { width: 100%; box-sizing: border-box; padding: 6px 8px; font: inherit; }
  .hint { margin: 4px 0 0; color: #666; font-size: 12px; }
  .row { display: flex; gap: 10px; }
  .row > div { min-width: 0; }
  .grow { flex: 1; }
  .result { margin: 12px 0 0; color: #1b6e2d; font-size: 13px; }
  .result.bad { color: #b00020; }
  .buttons { display: flex; gap: 8px; align-items: center; margin-top: 16px; flex-wrap: wrap; }
  .spacer { flex: 1; }
</style>
