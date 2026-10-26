//! Email a photo from Outlook. Classic Outlook is driven through COM (a short PowerShell script: no COM bindings to
//! carry). New Outlook has no COM: it is handed an unsent `.eml` message with the photo attached (`olk.exe message.eml`),
//! and if that cannot be done (a very big file, `olk.exe` missing) the Windows Share sheet is shown instead.

use base64::Engine;

/// How the photo was handed over, for the app to say.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// A new message with the photo attached is open in classic Outlook.
    ClassicOutlook,
    /// A new message with the photo attached is open in new Outlook.
    NewOutlook,
    /// The Windows Share sheet is open.
    ShareSheet,
}

/// Biggest file attached through an `.eml` (it is held in memory and grows by a third when encoded); bigger ones go
/// through the Share sheet.
#[cfg(windows)]
const EML_LIMIT: u64 = 60 * 1024 * 1024;

fn is_plain_header(s: &str) -> bool {
    s.bytes().all(|b| (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\')
}

/// A header value that may hold any characters (an RFC 2047 encoded word when it is not plain).
fn header_text(s: &str) -> String {
    if is_plain_header(s) {
        s.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s))
    }
}

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

/// An unsent message (`X-Unsent: 1`: a mail program opens it as a draft to finish and send) with the file attached.
pub fn build_eml(name: &str, subject: &str, bytes: &[u8]) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let shown = header_text(name);
    let subject = header_text(subject);
    let star: String = name
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let mut out = String::with_capacity(encoded.len() + encoded.len() / 38 + 600);
    out.push_str("X-Unsent: 1\r\n");
    out.push_str(&format!("Subject: {subject}\r\n"));
    out.push_str("MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"pixie-boundary\"\r\n\r\n");
    out.push_str("--pixie-boundary\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\r\n");
    out.push_str(&format!(
        "--pixie-boundary\r\nContent-Type: {}; name=\"{shown}\"\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"{shown}\"; filename*=UTF-8''{star}\r\n\r\n",
        content_type(name)
    ));
    for line in encoded.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push_str("\r\n");
    }
    out.push_str("--pixie-boundary--\r\n");
    out.into_bytes()
}

/// Open the file in a new message in new Outlook: the message is written to a temporary `.eml` and handed to `olk.exe`.
/// The `.eml` stays (Outlook reads it in its own time); ones older than a day are cleared away each time.
#[cfg(windows)]
fn new_outlook(path: &std::path::Path, subject: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let dir = std::env::temp_dir().join("pixie-mail");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > std::time::Duration::from_secs(86_400));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "photo".into());
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let eml = dir.join(format!("{stamp}.eml"));
    std::fs::write(&eml, build_eml(&name, subject, &bytes)).map_err(|e| e.to_string())?;
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
$null = $m.Attachments.Add($env:PIXIE_MAIL_FILE)
$m.Display()
"#;

/// Ok(true): the message is open. Ok(false): classic Outlook is not installed (or cannot be started).
#[cfg(windows)]
fn classic(path: &std::path::Path, subject: &str) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    let mut child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", CLASSIC_SCRIPT])
        .env("PIXIE_MAIL_FILE", path)
        .env("PIXIE_MAIL_SUBJECT", subject)
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
fn share(hwnd: isize, path: &std::path::Path, subject: &str) -> Result<(), String> {
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
    let path_text = path.to_string_lossy().into_owned();
    let title = subject.to_string();
    let token = manager
        .DataRequested(&TypedEventHandler::new(move |_, args: windows::core::Ref<DataRequestedEventArgs>| {
            let request = args.ok()?.Request()?;
            let data = request.Data()?;
            data.Properties()?.SetTitle(&HSTRING::from(title.as_str()))?;
            // The file is opened off the UI thread; the Share sheet waits for it (the deferral).
            let deferral = request.GetDeferral()?;
            let path_text = path_text.clone();
            std::thread::spawn(move || {
                let item = StorageFile::GetFileFromPathAsync(&HSTRING::from(path_text.as_str())).and_then(|a| a.get());
                if let Ok(file) = item {
                    if let Ok(item) = file.cast::<IStorageItem>() {
                        let _ = data.SetStorageItems(&IIterable::<IStorageItem>::from(vec![Some(item)]), true);
                    }
                }
                let _ = deferral.Complete();
            });
            Ok(())
        }))
        .map_err(err)?;
    *LAST.lock().unwrap() = Some((hwnd, token));
    unsafe { interop.ShowShareUIForWindow(hw) }.map_err(err)
}

/// Open a new email with the file attached and `subject` as its subject: in classic Outlook if it is there, then new
/// Outlook, otherwise through the Share sheet.
#[cfg(windows)]
pub fn email_file(app: &tauri::AppHandle, path: &std::path::Path, subject: &str) -> Result<Route, String> {
    use tauri::Manager;
    if !path.is_file() {
        return Err("The file is not there any more.".into());
    }
    if classic(path, subject)? {
        return Ok(Route::ClassicOutlook);
    }
    // New Outlook is the one in use.
    if std::fs::metadata(path).map(|m| m.len() <= EML_LIMIT).unwrap_or(false) && new_outlook(path, subject).is_ok() {
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
    let (p, s) = (path.to_path_buf(), subject.to_string());
    app.run_on_main_thread(move || {
        let _ = tx.send(share(hwnd, &p, &s));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())??;
    Ok(Route::ShareSheet)
}

#[cfg(not(windows))]
pub fn email_file(_app: &tauri::AppHandle, _path: &std::path::Path, _subject: &str) -> Result<Route, String> {
    Err("Emailing a photo is only set up for Windows so far.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(b: Vec<u8>) -> String {
        String::from_utf8(b).unwrap()
    }

    #[test]
    fn the_message_is_an_unsent_draft_with_the_file_attached_in_base64() {
        let eml = text(build_eml("IMG_1.JPG", "Joan, Lisbon \u{2013} 14 July 2019", b"hello"));
        assert!(eml.starts_with("X-Unsent: 1\r\n"));
        assert!(eml.contains("Subject: =?UTF-8?B?"), "a subject with a dash is encoded: {eml}");
        assert!(eml.contains("Content-Type: image/jpeg; name=\"IMG_1.JPG\""));
        assert!(eml.contains("filename=\"IMG_1.JPG\""));
        assert!(eml.contains(&base64::engine::general_purpose::STANDARD.encode(b"hello")));
        assert!(eml.ends_with("--pixie-boundary--\r\n"));
    }

    #[test]
    fn long_files_are_wrapped_in_lines_of_76_and_decode_back() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let eml = text(build_eml("a.bin", "A file", &data));
        assert!(eml.contains("Subject: A file\r\n"));
        let part = eml.split("\r\n\r\n").nth(3).unwrap();
        let body: Vec<&str> = part.lines().take_while(|l| !l.starts_with("--")).collect();
        assert!(body.iter().all(|l| l.len() <= 76));
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(body.concat()).unwrap(), data);
        assert!(eml.contains("application/octet-stream"));
    }

    #[test]
    fn a_name_with_other_characters_is_encoded() {
        let eml = text(build_eml("Zoë \"x\".png", "Zoë", b"1"));
        assert!(eml.contains("Subject: =?UTF-8?B?"), "{eml}");
        assert!(eml.contains("filename*=UTF-8''Zo%C3%AB%20%22x%22.png"), "{eml}");
        assert!(!eml.contains("\"x\""), "no raw quote inside a header");
    }
}
