# Ask Pixie proxy

A Cloudflare Worker between Pixie and a language model. Forked from Ear Ring's Sage proxy (its `assistant-proxy/`); the design is
`docs/superpowers/specs/2026-10-28-ask-pixie-design.md`.

- `POST /v1/ask`: answers one question. The body is the last turns plus a context block (platform, language, tab, the settings with
  their current values, and the guide and the places a button can lead to, which describe the version of the app that is asking). The reply is text, optionally a settings proposal, a `goTo` place in the app, and a note that feedback was sent.
- `POST /v1/feedback`: stores one note for the developer. The model only drafts notes (`feedback` in the `/v1/ask` reply); the app shows the note and
  calls this only if the person answers Yes. At most 5 a day per install.
- `GET /health`: liveness.
- Cron (08:00 UTC): emails unsent feedback as one digest.

What the model is told is split in two. The worker holds the rules (`src/prompt.ts`; the five fixed rules come first, and nothing the app
sends can change them). The app holds what is true of its own version: the guide (`ui/src/lib/askGuide.ts`), the places a button can lead
to (`ui/src/lib/askGoTo.ts`) and the settings, and sends them with every question. So the worker knows nothing about tabs or tools: old
and new versions of Pixie can be in use at once, each answered from its own description, and a change to the guide is tested with the app
and shipped with it, with no redeploy. The worker checks the shapes and sizes (`src/validate.ts`), offers `go_to` only the places the app
listed, and nothing in the request can carry a picture.

## Choosing the model

`wrangler.toml` `[vars]`: `PROVIDER` is `anthropic` (secret `ANTHROPIC_API_KEY`; `MODEL` optional) or `openai` (any OpenAI-compatible API;
secret `OPENAI_API_KEY`, vars `OPENAI_BASE_URL`, `MODEL`). Change the vars and `npx wrangler deploy`; no app release needed.

## Develop

From the repository root, with `just`:

    just ask-test         # install, type check, unit tests with stubbed providers (no network)
    just ask-dev          # local server on http://localhost:8787; put secrets in ask-proxy/.dev.vars (untracked)
    just ask-deploy       # tests, then `wrangler deploy`: live for every copy of Pixie at once

Or in this folder: `npm install`, `npm test`, `npm run typecheck`, `npx wrangler dev`.

Redeploy only when something in this folder changed (the rules, the tool shapes, the checks, the model in `wrangler.toml`). A change to the
app's guide or places is not one.

## First deploy (once, by hand)

    npx wrangler login
    npx wrangler kv namespace create QUOTA        # paste the id into wrangler.toml
    npx wrangler kv namespace create FEEDBACK     # likewise
    npx wrangler secret put ANTHROPIC_API_KEY     # or OPENAI_API_KEY
    npx wrangler secret put RESEND_API_KEY
    npx wrangler secret put DIGEST_TO
    just ask-deploy

Pixie has the deployed address built in (`DEFAULT_ENDPOINT` in `src-tauri/src/ask.rs`). To try another one, for example `npx wrangler dev`
(http://localhost:8787/v1/ask), run Pixie with `PIXIE_ASK_URL` set to it. Change the built-in address if the worker is ever moved.

## Dev builds are not limited

Debug builds of Pixie send a secret in `X-Dev-Key`; the worker skips the daily limit (and does not count the question) only when it
equals its `DEV_KEY` secret. Release builds never send it. The server decides, so a request that merely claims to be a dev build
gets nothing.

    npx wrangler secret put DEV_KEY               # 32 hex digits, e.g. python -c "import secrets;print(secrets.token_hex(16))"

Put the same 32 digits in `ask-dev-key.txt` in Pixie's data folder (on Windows `%LOCALAPPDATA%pp.pixie.desktop\`), or in the
`PIXIE_ASK_DEV_KEY` environment variable. Without `DEV_KEY` set on the worker, nobody is exempt.

## After changing the settings the app exposes

Regenerate the contract fixture (from `ui/`; it also carries the app's guide and places) and re-run the proxy tests:

    WRITE_ASK_FIXTURE=1 npx vitest run src/lib/askSettings.test.ts
    cd ../ask-proxy && npm test

## Trying it on the real model

    node scripts/examples.mjs https://<your worker>.workers.dev
