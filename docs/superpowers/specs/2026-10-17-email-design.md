# Pixie: email photos to people by nickname (design)

Date: 2026-10-17. Builds on the shell (details pane with a disabled **Email...** button, index-based selection), videos and the map. GitHub issue #4.

## Context

The user wants to "easily email selected pics to people, including nicknames (dad, simon, etc)".

Decisions (user):
- **Send directly from Pixie** through the user's own email account (SMTP with an app password), not by opening a mail app.
- **App passwords first.** The user's own address is Outlook.com/Hotmail, where Microsoft's current rules for third-party SMTP are unclear (sources conflict; forum reports of "basic authentication is disabled" in 2026). The account setup has a test-email button, and the feature is built so that account setup and a real test send come first. If Microsoft refuses app-password sending, Pixie shows a clear message and "Sign in with Microsoft" (OAuth2) becomes a follow-up. Gmail, iCloud and Yahoo work with app passwords.

## Goals

- Select photos/videos, click **Email...**, pick recipients by nickname, send.
- Contacts: a small address book kept in Pixie.
- Large photos are shrunk for sending; originals are never modified.
- The password is stored in the operating system's credential store only.
- Clear plain-language errors, including provider size limits and refused logins.

## Non-goals (this version)

Sign in with Microsoft/Google (OAuth2), groups or mailing lists, importing contacts, several accounts, converting or shrinking videos, scheduled send, HTML mail, sending links instead of attachments, saving a copy to a Sent folder (providers do this for SMTP submission themselves).

## Account setup

- One account: display name (optional), email address, provider, server host, port, security (STARTTLS on 587, or SSL on 465), and the app password.
- Provider list with presets: Gmail (`smtp.gmail.com`, 587 STARTTLS, 25 MB), iCloud (`smtp.mail.me.com`, 587, 20 MB), Yahoo (`smtp.mail.yahoo.com`, 587, 25 MB), Outlook.com (`smtp-mail.outlook.com`, 587, 20 MB, with a note that Microsoft may require OAuth), Other (host, port, security, limit entered by the user, default 20 MB). Each preset shows one line on how to create an app password.
- Non-secret settings are stored in the library database (V4 migration). The password goes to the OS credential store (Windows Credential Manager; macOS Keychain) under a Pixie-specific service name, never to a file, the database or a log.
- **Send me a test email** sends a short message to the account's own address and reports the result in plain language.
- A connection is only made over an encrypted channel with certificate verification; there is no plaintext fallback.

## Contacts

- `contacts(id, nickname, name, email)`: nickname unique (case-insensitive), 1-40 characters, no `,`/`;`/`<`/`>`; email validated (one `@`, a dot in the domain, no spaces, at most 254 characters, no newlines or control characters). `name` optional.
- A **Contacts** window (from the email dialog and from a menu entry) lists contacts with add, edit and delete.
- Recipient matching: typing matches nicknames and names by prefix then substring, case-insensitive; a typed string that is a valid address is accepted as a plain recipient; anything else is an unknown name (the dialog offers to create a contact, or blocks sending).

## Compose dialog

- Opens from **Email...** when at least one item is selected (also while the map is open). Selection is index-based, so the backend resolves the selected indexes to photos (`photos_at(query, indexes)`, at most 200 indexes, using the current view's filters and order).
- Fields: **To** (chips; nickname suggestions), **Subject** (default "N photos from <date or date range>"), **Message** (optional, plain text, up to 5,000 characters), **Size** (Small 1600 px, Medium 2400 px, Original; default Small).
- Shows the attachment count and the estimated encoded size against the provider limit before sending; sending is blocked over the limit with a plain suggestion (smaller size or fewer items).
- At most 50 attachments.
- States: idle, preparing (n of N), sending, sent (names of recipients), failed (message and the dialog stays open for retry). Sending runs off the UI thread; the dialog cannot be double-sent.
- Resized copies carry no EXIF (so no location); the dialog says so. "Original" attaches the file as is (location data stays in it), and says that too.

## Preparing attachments

- Photos: decode (including HEIC), apply orientation, scale so the longer side is at most the chosen size (never enlarge), encode JPEG quality 85, no metadata. "Original" attaches the original file unchanged.
- Videos: always the original file. If a video's encoded size alone exceeds the provider limit, sending is blocked with a message naming the file.
- Attachment names: the original base name, sanitised (no path separators, control characters or leading dots), de-duplicated with a counter; resized photos use `.jpg`.
- Size estimate: sum of attachment bytes x 4/3 (base64) plus 2 KB per message; compared with the provider limit.
- Files that cannot be read or decoded are reported by name and the send is not started.

## Sending

- Backend crate additions: `lettre` (SMTP, builder, rustls) and `keyring`, both compatible with Rust 1.89.
- `Mailer` and `SecretStore` are small traits so tests use fakes; the real implementations wrap lettre and the OS store.
- Commands: `get_mail_account`, `save_mail_account(settings, password?)`, `delete_mail_account`, `send_test_email`, `list_contacts`, `save_contact`, `delete_contact`, `photos_at(query, indexes)`, `send_email(request)`. Sending runs on the blocking pool and emits `mail-progress` events.
- No automatic retry. Errors map to plain messages: login refused (with the provider's hint, and for Outlook.com a note about app passwords/OAuth), cannot reach the server, message too large for the server, recipient rejected, timed out. Server error text is logged to the console without credentials, never shown raw.
- Timeouts: connect 15 s, whole send 120 s.

## Privacy and security

- The only data that leaves the machine is the message the user sends, to their own mail server. Nothing else is sent.
- Password only in the credential store; never in logs, events or error messages.
- Recipient and subject input is validated against header injection (no CR/LF).

## Testing

- Rust: contact validation and unique case-insensitive nicknames (CRUD); recipient parsing and nickname resolution; address validation (good, bad, injection attempts); size estimation and limits; MIME building with attachments (unique sanitised names, base64 size, headers safe); resize (dimensions, orientation, no EXIF, no enlarging, JPEG output); `photos_at`; account storage with an in-memory secret store; error mapping; the send flow with a fake mailer (success, auth failure, too large, recipient rejected).
- UI: recipient chip parsing and suggestion ranking, default subject text, size label and limit state, form validation, double-send guard.
- Manual: **Send me a test email** with the user's Outlook.com account (answers the OAuth question early), then a real send of resized photos and one video to the user's own address, and with Gmail if available.

## Review Focus

- Header injection through nickname, subject, name or file names.
- The password never appears in logs, events, errors or the database; deleting the account removes it.
- Sending a very large selection: memory (attachments are prepared one at a time and streamed to the message, not all decoded at once), the 50-item and size limits.
- Double click on Send; closing the dialog or leaving the app mid-send.
- A contact deleted or edited while the dialog is open.
- Selection resolved against a view that changed (index-based selection) between opening the dialog and sending.
- HEIC and rotated photos in the resize path; videos near the limit.
- Failure messages for a refused login, a wrong host/port and no network.

## Open question carried into the build

Whether Outlook.com accepts an app password for SMTP in 2026. The test-email step answers it; the fallback is OAuth2 as a separate follow-up.

---

# Revision 1 (2026-10-17): delivery methods for many email clients

## Why

Pixie is for lots of people with different email programs, and asking for a username and password is a threatening first screen. A discussion (recorded on GitHub issue #4) compared what is possible per email client. The conclusion is a **layered design**: Pixie quietly picks the best way to deliver, and most people never see a settings screen. The direct-send code already built becomes an advanced option.

## What is possible per client (from how each is built; to be verified on real machines)

| Client | Open a draft with photos ready | Send from Pixie | Always works |
|---|---|---|---|
| Classic Outlook (desktop) | Yes (MAPI) | not needed | Yes |
| New Outlook for Windows | No (no attachments from other programs) | sign-in or app password | Yes |
| Windows Mail app | Maybe (Share), being retired | same | Yes |
| Apple Mail (Mac) | Yes (AppleScript) | not needed | Yes |
| Thunderbird | Yes (command line, MAPI) | not needed | Yes |
| Gmail / Outlook.com / Yahoo / iCloud in a browser | No (websites cannot receive files) | app password; later Microsoft sign-in; Google deferred | Yes |
| Other | depends | needs server details | Yes |

"Always works" is the fallback below. Detected on the user's own PC (read-only registry check): classic Outlook 16 is installed and is the default mail program with MAPI, so the handoff route applies there.

## Three delivery methods

1. **Handoff to a mail program** (preferred when available). Pixie opens the user's own mail program with a new message: recipients filled in, subject, text and the photos attached. The user reviews and presses Send there; Pixie never holds a password.
   - Windows: Simple MAPI (`MAPISendMailW` from `mapi32.dll`, with the dialog shown) when the default mail client advertises MAPI (classic Outlook, Thunderbird when set as default). Thunderbird is also started with its `-compose` command line (`to`, `subject`, `body`, `attachment`) when it is the default program.
   - macOS: Mail.app through `osascript` (AppleScript) creating a message with attachments and showing it. Implemented and unit-tested for script generation; not verifiable on this machine.
   - **Resizing is optional.** Without it, Pixie attaches the original files by their own paths: nothing is copied or changed. With it, the smaller copies are written to a hidden temporary folder `%TEMP%/Pixie mail/<timestamp>/` (a mail program attaches files, so the copies must exist as files for a moment). Those folders are deleted after one day, at start-up. The originals are never modified.
   - All launching uses argument arrays or the API directly, never a shell string; addresses and subjects are validated as before.
2. **Send from my own account** (advanced). The existing SMTP sender and account settings dialog, shown under "Other ways to send". Later: "Sign in with Microsoft" (OAuth2, free publisher registration, personal accounts supported). Google sign-in is deferred (restricted mail permission needs a verification review). A published "Sign in with ..." route needs a one-time publisher-side app registration; users never register anything.
3. **Prepare and paste** (fallback for everyone). Pixie puts the files on the clipboard (Windows file-drop clipboard format; macOS pasteboard file URLs) and opens a new message through `mailto:` with recipients and subject if a handler exists. Without resizing the clipboard holds the originals (no copies, no folder). With resizing it holds the smaller copies from the temporary folder. Pixie opens a folder window only if putting files on the clipboard fails. The dialog says: "Your photos are ready. Press Ctrl+V in your new email."

## Choosing

`choose_delivery(detected client, account configured) -> Delivery` (pure, tested):
- A detected mail program that supports attachments (classic Outlook, Thunderbird, Apple Mail) -> Handoff is the primary action.
- Otherwise, if an account is configured -> Send from my account is primary.
- Otherwise -> Prepare and paste is primary.
- The other available methods are listed under a small "Other ways to send" disclosure. The primary button is labelled for what it does: "Open in Outlook", "Open my email program", "Send", or "Prepare photos".
- A **Which mail program does Pixie see?** line in Email settings shows the detection result, so problems can be diagnosed.

## Compose dialog changes

The shared compose dialog (recipients by nickname, subject, message, size, estimate) changes as follows. **Size** becomes one plain checkbox, **Make photos smaller for email**: off means original files, untouched; on shows a menu (Small 1600 px, Medium 2400 px). It is on by default only when the originals add up to more than 15 MB (providers commonly refuse 20 to 25 MB), and the dialog says so. The estimate shows the total for the current choice. Videos are always sent as they are. Also: no account or password is required unless the chosen method is "Send from my account"; the primary button is labelled per delivery method; after a handoff it says "Your email program opened with 3 photos attached. Press Send there."; the size helper below applies to every delivery method (a mail program only reports a refusal after the fact, so Pixie warns first).

## Size helper: know the limit, help get under it

Email providers refuse messages over a size limit, and the limit counts the email as encoded (about one third more than the files). Pixie keeps track of the limits and helps the user get under them instead of letting a send fail.

**Known limits.** A table in code (also shown in the settings), each marked approximate because providers change them: Gmail 25 MB, Yahoo 25 MB, Outlook.com / Hotmail 20 MB, iCloud 20 MB (Apple Mail can offer Mail Drop for bigger files), a conservative default of 20 MB for anything unknown (work and ISP mail varies widely). The "Other" provider and every entry can be edited by the user.

**Which limit applies.** With "Send from my own account" the account's provider is known. With a handoff or prepare-and-paste Pixie cannot see which account the mail program will use, so the compose dialog has one small question, asked once and remembered: **Which email provider will you send from?** (the provider list, plus "Not sure", which means 20 MB). It can be changed at any time from the dialog.

**The size meter.** The compose dialog always shows a plain bar: "Email size about 22 MB of 20 MB" (green under 80%, amber near, red over), updated live as the choices change. Photos are estimated from their dimensions and file size; videos use their file size.

**Help when it is too big (or close).** In plain words, with the effect of each fix shown before it is applied:
1. **Make photos smaller** ("22 MB -> about 8 MB, fits"), preferring Medium, then Small.
2. **Send in several emails** ("3 emails of about 8 MB each"), splitting the selection into batches that each fit, keeping the order of the selection. Direct sending sends them one after another with "(1 of 3)" in the subject; handoff opens one draft at a time and asks "Ready for the next one?" between them. Limited to 5 emails; beyond that the dialog suggests choosing fewer photos instead.
3. **Choose fewer** by listing the biggest items with a check mark each, showing the new total as boxes are unticked.
4. **Leave out the videos** when videos are the main weight.
Any single item bigger than the limit on its own (typically a video) cannot be fixed by resizing or splitting, and the dialog says so plainly, naming it, and offers to leave it out. Prepare-and-paste cannot split across emails, so it offers fixes 1, 3 and 4 only.

**Pure, tested logic.** `estimate_email_bytes(items, resize_choice)`, `meter_state(bytes, limit)`, `plan_batches(item_sizes, limit) -> Vec<Vec<index>>` (every item exactly once, each batch within the limit, order preserved, items over the limit flagged), and `suggest_fixes(items, current, limit) -> Vec<Fix>` (each with its resulting size), plus a limit table lookup.

## Detection (Windows)

Read `HKCU\Software\Clients\Mail` and `HKLM\SOFTWARE\Clients\Mail` (default client name; a client's `DLLPath`/`MAPI` values), the `mailto` handler (`ProgId`), and look for `OUTLOOK.EXE` (classic Outlook), the New Outlook app package, and Thunderbird. Classification is a pure function over those facts: `ClassicOutlook`, `Thunderbird`, `OtherMapi`, `NewOutlook`, `WindowsMail`, `Unknown`. macOS: the default `mailto` handler's bundle id (`com.apple.mail`, Thunderbird, Outlook).

## Testing

- Pure and table-driven: detection (registry snapshots -> client), `choose_delivery`, mailto URL builder (percent-encoding, no header injection, length cap), Thunderbird command-line builder (quoting), AppleScript generation (escaping of quotes/backslashes in paths, subject, names), temp-folder creation and cleanup, per-delivery dialog labels.
- Behind traits with fakes: the MAPI launcher, the clipboard, "open a folder", "run a program"; the SMTP sender and the credential store already are.
- Manual: handoff to the user's classic Outlook (draft opens with recipients and attachments, original files untouched); the prepare-and-paste route into any compose window; direct send stays available.

## Review Focus (additions)

- Anything built from user input that reaches a command line, AppleScript or `mailto:` URL (quote, newline, `%`, `&`, backslash, very long strings).
- The temp folder: never writes outside it, is cleaned up, holds no originals and no metadata.
- MAPI: large attachment lists, a mail program that is closed or slow to start, a user who cancels the draft (no error shown as failure).
- A machine where detection finds nothing: the fallback must still work and say so plainly.
- Size helper: estimates that are far off (a very compressible or already tiny file), batches that still fail at the provider, a single item bigger than the limit, a wrong remembered provider, and the batch loop (cancelling halfway, closing the dialog).

## Revised build order

Already built and kept: validation types, contacts and account storage (including `photos_at`), the `pixie-mail` crate (providers, recipients, text checks, attachments, message building, SMTP sender, credential store), Tauri commands for the account and sending, the account settings dialog (moves under "Other ways to send").
Remaining: (1) delivery model: detection, `choose_delivery`, prepare-to-folder, mailto and Thunderbird builders, prepare-and-paste (folder, clipboard, open); (2) compose dialog and contacts window using the delivery model; (3) Windows MAPI handoff and Thunderbird; (4) macOS handoff (script generation tested, behaviour unverified); (5) later, separate issue: "Sign in with Microsoft".
