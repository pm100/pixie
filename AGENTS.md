# Agent instructions

- Never commit or push unless the user says so.
- Only pluralise when a count is more than one: "1 item", "2 items" (use `plural` in `ui/src/lib/format.ts`).
- Always keep builds clean: no compiler warnings (cargo check --workspace --all-targets). Fix new ones straight away rather than leaving them; do not silence them with `#[allow]`. Delete dead code, or gate it (`#[cfg(...)]`) when only one platform or the tests use it.
- When dependencies change (Cargo.toml, ui/package.json, the vcpkg libraries or the downloaded models), run `just notices` and commit THIRD-PARTY-NOTICES.md and ui/src/generated/third-party.json; Settings, About lists them.
- Never put a control in the middle of a sentence in the UI (e.g. "Refresh every [nn] days"): word order differs between languages, so it cannot be localized. Use a label followed by the control instead: "Days between refreshes: [nn]". Existing prompts that embed controls should be fixed as they are touched.
