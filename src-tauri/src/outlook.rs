//! Email a photo from Outlook. Classic Outlook is driven through COM (a short PowerShell script: no COM bindings to
//! carry). New Outlook has no COM: it is handed an unsent `.eml` message with the photo attached (`olk.exe message.eml`),
//! and if that cannot be done (a very big file, `olk.exe` missing) the Windows Share sheet is shown instead.

use base64::Engine;

/// How the photo was handed over, for the app to say.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)] // (each system uses its own)
pub enum Route {
    /// A new message with the photo attached is open in classic Outlook.
    ClassicOutlook,
    /// A new message with the photo attached is open in new Outlook.
    NewOutlook,
    /// (Mac) A new message with the photo attached is open in Outlook.
    MacOutlook,
    /// (Mac) A new message with the photo attached is open in Apple Mail.
    AppleMail,
    /// The Windows Share sheet is open.
    ShareSheet,
}

/// What goes in the new message besides the files.
#[derive(Clone, Default, Debug)]
pub struct Draft {
    pub subject: String,
    /// Addresses to put in To (may be none: the person addresses it).
    pub to: Vec<String>,
    /// Text for the body (may be empty).
    pub body: String,
}

/// The addresses typed in the Email box, separated by commas, semicolons or spaces. Each must look like `name@host`: the
/// text goes into a mail header and into scripts, so nothing else (quotes, brackets, line breaks) gets through.
pub fn clean_to(text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for a in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|a| !a.is_empty()) {
        let ok = a.len() <= 254
            && a.bytes().all(|b| b.is_ascii_graphic() && !b"<>()[]\\\",:;".contains(&b))
            && a.split_once('@').is_some_and(|(n, h)| !n.is_empty() && h.contains('.') && !h.starts_with('.') && !h.ends_with('.') && !h.contains('@'));
        if !ok {
            return Err(format!("\u{201c}{a}\u{201d} is not an email address."));
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(a)) {
            out.push(a.to_string());
        }
    }
    Ok(out)
}

/// Biggest file attached through an `.eml` (it is held in memory and grows by a third when encoded); bigger ones go
/// through the Share sheet.
#[cfg(windows)]
const EML_LIMIT: u64 = 60 * 1024 * 1024;

#[cfg_attr(not(windows), allow(dead_code))]
fn is_plain_header(s: &str) -> bool {
    s.bytes().all(|b| (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\')
}

/// A header value that may hold any characters (an RFC 2047 encoded word when it is not plain).
#[cfg_attr(not(windows), allow(dead_code))]
fn header_text(s: &str) -> String {
    if is_plain_header(s) {
        s.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s))
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg" | "jpe") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        Some("tif" | "tiff") => "image/tiff",
        Some("bmp") => "image/bmp",
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",
        Some("mkv") => "video/x-matroska",
        Some("wmv") => "video/x-ms-wmv",
        _ => "application/octet-stream",
    }
}

/// An unsent message (`X-Unsent: 1`: a mail program opens it as a draft to finish and send) with the files attached:
/// each is its name and its bytes.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn build_eml(draft: &Draft, files: &[(&str, &[u8])]) -> Vec<u8> {
    let subject = header_text(&draft.subject);
    let mut out = String::new();
    out.push_str("X-Unsent: 1\r\n");
    if !draft.to.is_empty() {
        out.push_str(&format!("To: {}\r\n", draft.to.join(", ")));
    }
    out.push_str(&format!("Subject: {subject}\r\n"));
    out.push_str("MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"pixie-boundary\"\r\n\r\n");
    out.push_str("--pixie-boundary\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n");
    for line in base64::engine::general_purpose::STANDARD.encode(draft.body.as_bytes()).as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    for (name, bytes) in files {
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let shown = header_text(name);
        let star: String = name
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect();
        out.reserve(encoded.len() + encoded.len() / 38 + 400);
        out.push_str(&format!(
            "--pixie-boundary\r\nContent-Type: {}; name=\"{shown}\"\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"{shown}\"; filename*=UTF-8''{star}\r\n\r\n",
            content_type(name)
        ));
        for line in encoded.as_bytes().chunks(76) {
            out.push_str(std::str::from_utf8(line).unwrap_or_default());
            out.push_str("\r\n");
        }
    }
    out.push_str("--pixie-boundary--\r\n");
    out.into_bytes()
}

/// The folder for temporary messages and smaller copies of pictures. Things in it older than a day are cleared away
/// each time it is asked for (a mail program reads what it was given in its own time, so nothing is removed at once).
pub fn mail_temp_dir() -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("pixie-mail");
    std::fs::create_dir_all(&dir)?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > std::time::Duration::from_secs(86_400));
            if old {
                let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
            }
        }
    }
    Ok(dir)
}

/// A new folder inside it, for one set of smaller copies (so two copies with the same name never meet).
pub fn new_copy_dir() -> std::io::Result<std::path::PathBuf> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dir = mail_temp_dir()?.join(format!("copies-{stamp}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Open the file in a new message in new Outlook: the message is written to a temporary `.eml` and handed to `olk.exe`.
/// The `.eml` stays (Outlook reads it in its own time); ones older than a day are cleared away each time.
#[cfg(windows)]
fn new_outlook(paths: &[std::path::PathBuf], draft: &Draft) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let dir = mail_temp_dir().map_err(|e| e.to_string())?;
    let mut loaded: Vec<(String, Vec<u8>)> = Vec::new();
    for path in paths {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "photo".into());
        loaded.push((name, std::fs::read(path).map_err(|e| e.to_string())?));
    }
    let files: Vec<(&str, &[u8])> = loaded.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let eml = dir.join(format!("{stamp}.eml"));
    std::fs::write(&eml, build_eml(draft, &files)).map_err(|e| e.to_string())?;
    std::process::Command::new("olk.exe").arg(&eml).creation_flags(0x0800_0000).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// The script that makes the message. The path comes in the environment, never spliced into the script. Exit code 3:
/// classic Outlook is not there to drive.
#[cfg(windows)]
const CLASSIC_SCRIPT: &str = r#"
# New Outlook (olk.exe) running and classic not: the person uses new Outlook. Classic's COM would start a second,
# hidden-away classic Outlook to hold the message, so leave it to the Share sheet.
if ((Get-Process olk -ErrorAction SilentlyContinue) -and -not (Get-Process OUTLOOK -ErrorAction SilentlyContinue)) { exit 3 }
try { $o = New-Object -ComObject Outlook.Application } catch { exit 3 }
$m = $o.CreateItem(0)
$m.Subject = $env:PIXIE_MAIL_SUBJECT
if ($env:PIXIE_MAIL_TO) { $m.To = $env:PIXIE_MAIL_TO }
if ($env:PIXIE_MAIL_BODY) { $m.Body = $env:PIXIE_MAIL_BODY }
foreach ($f in $env:PIXIE_MAIL_FILE.Split('|')) { $null = $m.Attachments.Add($f) }
$m.Display()
"#;

/// Ok(true): the message is open. Ok(false): classic Outlook is not installed (or cannot be started).
#[cfg(windows)]
fn classic(paths: &[std::path::PathBuf], draft: &Draft) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    // (a `|` cannot be in a file name on Windows, so it can separate the paths)
    let joined = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("|");
    let mut child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", CLASSIC_SCRIPT])
        .env("PIXIE_MAIL_FILE", joined)
        .env("PIXIE_MAIL_SUBJECT", &draft.subject)
        .env("PIXIE_MAIL_TO", draft.to.join("; "))
        .env("PIXIE_MAIL_BODY", &draft.body)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Outlook starting up can take a while; a hung one must not hang the button for ever.
    let began = std::time::Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if began.elapsed() > std::time::Duration::from_secs(45) {
            let _ = child.kill();
            return Err("Outlook did not answer.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(3) => Ok(false),
        _ => Err(format!("Outlook could not make the message: {}", String::from_utf8_lossy(&out.stderr).trim())),
    }
}

/// Show the Windows Share sheet for the file. Must run on the main (UI) thread.
#[cfg(windows)]
fn share(hwnd: isize, paths: &[std::path::PathBuf], subject: &str) -> Result<(), String> {
    use std::sync::Mutex;
    use windows::{
        core::{factory, Interface, HSTRING},
        ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager},
        Foundation::TypedEventHandler,
        Storage::{IStorageItem, StorageFile},
        Win32::{Foundation::HWND, UI::Shell::IDataTransferManagerInterop},
    };
    use windows_collections::IIterable;

    // The handler of the last share, taken off before the next is put on.
    static LAST: Mutex<Option<(isize, i64)>> = Mutex::new(None);
    let err = |e: windows::core::Error| e.message();

    let interop = factory::<DataTransferManager, IDataTransferManagerInterop>().map_err(err)?;
    let hw = HWND(hwnd as *mut _);
    let manager: DataTransferManager = unsafe { interop.GetForWindow(hw) }.map_err(err)?;
    if let Some((_, token)) = LAST.lock().unwrap().take() {
        let _ = manager.RemoveDataRequested(token);
    }
    let path_texts: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let title = subject.to_string();
    let token = manager
        .DataRequested(&TypedEventHandler::new(move |_, args: windows::core::Ref<DataRequestedEventArgs>| {
            let request = args.ok()?.Request()?;
            let data = request.Data()?;
            data.Properties()?.SetTitle(&HSTRING::from(title.as_str()))?;
            // The file is opened off the UI thread; the Share sheet waits for it (the deferral).
            let deferral = request.GetDeferral()?;
            let path_texts = path_texts.clone();
            std::thread::spawn(move || {
                let mut items: Vec<Option<IStorageItem>> = Vec::new();
                for path_text in &path_texts {
                    let item = StorageFile::GetFileFromPathAsync(&HSTRING::from(path_text.as_str())).and_then(|a| a.get());
                    if let Ok(file) = item {
                        if let Ok(item) = file.cast::<IStorageItem>() {
                            items.push(Some(item));
                        }
                    }
                }
                if !items.is_empty() {
                    let _ = data.SetStorageItems(&IIterable::<IStorageItem>::from(items), true);
                }
                let _ = deferral.Complete();
            });
            Ok(())
        }))
        .map_err(err)?;
    *LAST.lock().unwrap() = Some((hwnd, token));
    unsafe { interop.ShowShareUIForWindow(hw) }.map_err(err)
}

/// Open a new email with the files attached, `draft.subject` as its subject, and the address and text if there are any: in
/// classic Outlook if it is there, then new Outlook, otherwise through the Share sheet (which takes only the files and a title).
#[cfg(windows)]
pub fn email_files(app: &tauri::AppHandle, paths: &[std::path::PathBuf], draft: &Draft) -> Result<Route, String> {
    use tauri::Manager;
    if paths.is_empty() {
        return Err("There is nothing to send.".into());
    }
    if let Some(gone) = paths.iter().find(|p| !p.is_file()) {
        return Err(format!("{} is not there any more.", gone.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    }
    if classic(paths, draft)? {
        return Ok(Route::ClassicOutlook);
    }
    // New Outlook is the one in use.
    let total: u64 = paths.iter().map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX / 4)).sum();
    if total <= EML_LIMIT && new_outlook(paths, draft).is_ok() {
        return Ok(Route::NewOutlook);
    }
    let window = app
        .webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false))
        .or_else(|| app.get_webview_window("main"))
        .ok_or("no window to share from")?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let (tx, rx) = std::sync::mpsc::channel();
    let (p, s) = (paths.to_vec(), draft.subject.clone());
    app.run_on_main_thread(move || {
        let _ = tx.send(share(hwnd, &p, &s));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())??;
    Ok(Route::ShareSheet)
}

/// The mail app registered for `mailto:` links, as a lower-case bundle id, from the macOS Launch Services settings (as
/// JSON). None: none was ever chosen (Apple Mail then).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mailto_handler(settings: &serde_json::Value) -> Option<String> {
    settings
        .get("LSHandlers")?
        .as_array()?
        .iter()
        .find(|h| h.get("LSHandlerURLScheme").and_then(|s| s.as_str()) == Some("mailto"))
        .and_then(|h| h.get("LSHandlerRoleAll").and_then(|s| s.as_str()))
        .map(str::to_lowercase)
}

/// Make a message in Apple Mail with the file attached and open its window.
#[cfg(target_os = "macos")]
const MAIL_SCRIPT: &str = r#"
on run argv
  set theSubject to item 1 of argv
  set theTo to item 2 of argv
  set theBody to item 3 of argv
  set AppleScript's text item delimiters to ","
  set addresses to {}
  if theTo is not "" then set addresses to text items of theTo
  -- (the paths are turned into files out here: inside the tell, "POSIX file" is the mail app's to interpret)
  set theFiles to {}
  repeat with i from 4 to count of argv
    set end of theFiles to POSIX file (item i of argv)
  end repeat
  tell application "Mail"
    set m to make new outgoing message with properties {subject:theSubject, content:theBody, visible:true}
    repeat with a in addresses
      tell m to make new to recipient at end of to recipients with properties {address:(contents of a)}
    end repeat
    repeat with f in theFiles
      tell content of m to make new attachment with properties {file name:(contents of f)} at after the last paragraph
    end repeat
    activate
  end tell
end run
"#;

/// The same in Microsoft Outlook.
#[cfg(target_os = "macos")]
const OUTLOOK_SCRIPT: &str = r#"
on run argv
  set theSubject to item 1 of argv
  set theTo to item 2 of argv
  set theBody to item 3 of argv
  set AppleScript's text item delimiters to ","
  set addresses to {}
  if theTo is not "" then set addresses to text items of theTo
  set theFiles to {}
  repeat with i from 4 to count of argv
    set end of theFiles to POSIX file (item i of argv)
  end repeat
  tell application "Microsoft Outlook"
    set m to make new outgoing message with properties {subject:theSubject, content:theBody}
    repeat with a in addresses
      make new recipient at m with properties {email address:{address:(contents of a)}}
    end repeat
    repeat with f in theFiles
      make new attachment at m with properties {file:(contents of f)}
    end repeat
    open m
    activate
  end tell
end run
"#;

/// Run an AppleScript with the subject, the addresses (joined with commas), the text and then the files as its arguments
/// (never spliced into the script).
#[cfg(target_os = "macos")]
fn run_script(script: &str, app: &str, paths: &[std::path::PathBuf], draft: &Draft) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("osascript")
        .arg("-")
        .arg(&draft.subject)
        .arg(draft.to.join(","))
        .arg(&draft.body)
        .args(paths)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("no input for osascript")?.write_all(script.as_bytes()).map_err(|e| e.to_string())?;
    // The mail app starting up can take a while; a hung one must not hang the button for ever.
    let began = std::time::Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if began.elapsed() > std::time::Duration::from_secs(60) {
            let _ = child.kill();
            return Err(format!("{app} did not answer."));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let why = String::from_utf8_lossy(&out.stderr);
    if why.contains("-1743") {
        return Err(format!("Pixie is not allowed to control {app}. Allow it in System Settings \u{2192} Privacy & Security \u{2192} Automation."));
    }
    Err(format!("{app} could not make the message: {}", why.trim()))
}

/// Open a new email with the files attached, the subject, and the address and text if there are any, in the Mac's mail app
/// (Outlook if that is what `mailto:` links open, otherwise Apple Mail).
#[cfg(target_os = "macos")]
pub fn email_files(_app: &tauri::AppHandle, paths: &[std::path::PathBuf], draft: &Draft) -> Result<Route, String> {
    if paths.is_empty() {
        return Err("There is nothing to send.".into());
    }
    if let Some(gone) = paths.iter().find(|p| !p.is_file()) {
        return Err(format!("{} is not there any more.", gone.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    }
    let settings = std::env::var_os("HOME")
        .map(|h| std::path::Path::new(&h).join("Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist"))
        .and_then(|p| std::process::Command::new("plutil").args(["-convert", "json", "-o", "-"]).arg(p).output().ok())
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok());
    if settings.as_ref().and_then(mailto_handler).is_some_and(|b| b.contains("outlook")) {
        run_script(OUTLOOK_SCRIPT, "Outlook", paths, draft)?;
        return Ok(Route::MacOutlook);
    }
    run_script(MAIL_SCRIPT, "Mail", paths, draft)?;
    Ok(Route::AppleMail)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn email_files(_app: &tauri::AppHandle, _paths: &[std::path::PathBuf], _draft: &Draft) -> Result<Route, String> {
    Err("Emailing a photo is only set up for Windows and Mac so far.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(b: Vec<u8>) -> String {
        String::from_utf8(b).unwrap()
    }

    fn draft(subject: &str) -> Draft {
        Draft { subject: subject.into(), ..Draft::default() }
    }

    #[test]
    fn the_address_and_text_go_in_the_message() {
        let d = Draft { subject: "Hi".into(), to: vec!["a@b.com".into(), "c@d.org".into()], body: "Here you are \u{2013} enjoy".into() };
        let eml = text(build_eml(&d, &[("a.jpg", b"x")]));
        assert!(eml.contains("To: a@b.com, c@d.org\r\n"), "{eml}");
        assert!(eml.contains(&base64::engine::general_purpose::STANDARD.encode("Here you are \u{2013} enjoy")), "{eml}");
        assert!(!text(build_eml(&draft("Hi"), &[])).contains("To:"), "no To header without an address");
    }

    #[test]
    fn typed_addresses_are_split_checked_and_made_unique() {
        assert_eq!(clean_to(" a@b.com; c@d.org,A@B.com  e@f.net ").unwrap(), vec!["a@b.com", "c@d.org", "e@f.net"]);
        assert!(clean_to("  ").unwrap().is_empty());
        for bad in ["nobody", "a@b", "@b.com", "a@@b.com", "a\"b@c.com", "<a@b.com>", "zo\u{eb}@b.com", "a@b.com\u{0}"] {
            assert!(clean_to(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_message_is_an_unsent_draft_with_the_file_attached_in_base64() {
        let eml = text(build_eml(&draft("Joan, Lisbon \u{2013} 14 July 2019"), &[("IMG_1.JPG", b"hello")]));
        assert!(eml.starts_with("X-Unsent: 1\r\n"));
        assert!(eml.contains("Subject: =?UTF-8?B?"), "a subject with a dash is encoded: {eml}");
        assert!(eml.contains("Content-Type: image/jpeg; name=\"IMG_1.JPG\""));
        assert!(eml.contains("filename=\"IMG_1.JPG\""));
        assert!(eml.contains(&base64::engine::general_purpose::STANDARD.encode(b"hello")));
        assert!(eml.ends_with("--pixie-boundary--\r\n"));
    }

    #[test]
    fn several_files_are_each_attached_under_their_own_name() {
        let eml = text(build_eml(&draft("Photos"), &[("a.jpg", b"one"), ("b.png", b"two")]));
        assert_eq!(eml.matches("Content-Disposition: attachment;").count(), 2);
        assert!(eml.contains("filename=\"a.jpg\"") && eml.contains("filename=\"b.png\""));
        assert!(eml.contains("Content-Type: image/png; name=\"b.png\""));
        assert!(eml.contains(&base64::engine::general_purpose::STANDARD.encode(b"one")));
        assert!(eml.contains(&base64::engine::general_purpose::STANDARD.encode(b"two")));
        assert!(eml.ends_with("--pixie-boundary--\r\n"));
        assert_eq!(eml.matches("--pixie-boundary--").count(), 1, "one closing line");
    }

    #[test]
    fn long_files_are_wrapped_in_lines_of_76_and_decode_back() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let eml = text(build_eml(&draft("A file"), &[("a.bin", &data)]));
        assert!(eml.contains("Subject: A file\r\n"));
        let part = eml.split("\r\n\r\n").nth(3).unwrap();
        let body: Vec<&str> = part.lines().take_while(|l| !l.starts_with("--")).collect();
        assert!(body.iter().all(|l| l.len() <= 76));
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(body.concat()).unwrap(), data);
        assert!(eml.contains("application/octet-stream"));
    }

    #[test]
    fn the_mailto_handler_is_found_in_the_launch_services_settings() {
        let s: serde_json::Value = serde_json::from_str(
            r#"{"LSHandlers":[{"LSHandlerURLScheme":"http","LSHandlerRoleAll":"com.google.chrome"},{"LSHandlerURLScheme":"mailto","LSHandlerRoleAll":"com.microsoft.Outlook"}]}"#,
        )
        .unwrap();
        assert_eq!(mailto_handler(&s).as_deref(), Some("com.microsoft.outlook"));
        assert_eq!(mailto_handler(&serde_json::json!({"LSHandlers": []})), None);
        assert_eq!(mailto_handler(&serde_json::json!({})), None);
    }

    #[test]
    fn a_name_with_other_characters_is_encoded() {
        let eml = text(build_eml(&draft("Zoë"), &[("Zoë \"x\".png", b"1")]));
        assert!(eml.contains("Subject: =?UTF-8?B?"), "{eml}");
        assert!(eml.contains("filename*=UTF-8''Zo%C3%AB%20%22x%22.png"), "{eml}");
        assert!(!eml.contains("\"x\""), "no raw quote inside a header");
    }
}
