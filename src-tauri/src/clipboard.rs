//! Copying photos to the system clipboard: the files (so Explorer pastes copies and mail or chat apps attach them), the picture itself, or
//! the paths as text. The byte layouts are built by plain functions (tested everywhere); only putting them on the clipboard is Windows code.

use std::path::PathBuf;

/// What to put on the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// The files, and for one photo its picture as well: it pastes as a picture into a document and as a file into Explorer or a mail.
    Photo,
    Files,
    Image,
    Paths,
}

impl Form {
    pub fn parse(s: &str) -> Option<Form> {
        match s {
            "photo" => Some(Form::Photo),
            "files" => Some(Form::Files),
            "image" => Some(Form::Image),
            "paths" => Some(Form::Paths),
            _ => None,
        }
    }
}

/// The paths as text, one per line (the line ends the system's own programs expect).
pub fn paths_text(paths: &[PathBuf], windows_lines: bool) -> String {
    let lines: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    lines.join(if windows_lines { "\r\n" } else { "\n" })
}

#[cfg(any(windows, test))]
/// A file list as the clipboard's `CF_HDROP` holds it: a `DROPFILES` header (20 bytes, wide characters), then each path and a NUL, then a final NUL.
pub fn drop_files(paths: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&20u32.to_le_bytes()); // pFiles: where the names start
    out.extend_from_slice(&[0u8; 8]); // pt: the drop point (not used)
    out.extend_from_slice(&0u32.to_le_bytes()); // fNC
    out.extend_from_slice(&1u32.to_le_bytes()); // fWide: the names are UTF-16
    for p in paths {
        for unit in p.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(any(windows, test))]
/// A picture as the clipboard's `CF_DIB` holds it: a `BITMAPINFOHEADER` (40 bytes), then 24-bit rows, bottom row first, each padded to 4 bytes.
/// `rgb` is the picture as RGB bytes, top row first.
pub fn dib_24(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let stride = (width as usize * 3).div_ceil(4) * 4;
    let mut out = Vec::with_capacity(40 + stride * height as usize);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // positive: the rows run bottom to top
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression: none
    out.extend_from_slice(&((stride * height as usize) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // resolution and colour counts
    for y in (0..height as usize).rev() {
        let row = &rgb[y * width as usize * 3..(y + 1) * width as usize * 3];
        for px in row.chunks_exact(3) {
            out.extend_from_slice(&[px[2], px[1], px[0]]); // blue, green, red
        }
        out.resize(out.len() + stride - width as usize * 3, 0);
    }
    out
}

/// Puts these formats on the clipboard in place of what is there: (format number, bytes) for each, or a registered format by name.
#[cfg(windows)]
pub enum Format {
    Id(u32),
    Named(&'static str),
}

#[cfg(windows)]
pub fn set_clipboard(items: &[(Format, Vec<u8>)]) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    // Another program may have the clipboard open for a moment: try a few times.
    let mut opened = false;
    for _ in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    if !opened {
        return Err("The clipboard is in use by another program. Try again.".into());
    }
    let result = (|| -> Result<(), String> {
        unsafe { EmptyClipboard() }.map_err(|e| e.message())?;
        for (format, bytes) in items {
            let id = match format {
                Format::Id(id) => *id,
                Format::Named(name) => {
                    let id = unsafe { RegisterClipboardFormatW(&HSTRING::from(*name)) };
                    if id == 0 {
                        continue;
                    }
                    id
                }
            };
            let mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(|e| e.message())?;
            let at = unsafe { GlobalLock(mem) } as *mut u8;
            if at.is_null() {
                return Err("The clipboard could not be filled (out of memory).".into());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), at, bytes.len());
                let _ = GlobalUnlock(mem);
                // The clipboard owns the memory once it is accepted.
                SetClipboardData(id, Some(HANDLE(mem.0))).map_err(|e| e.message())?;
            }
        }
        Ok(())
    })();
    let _ = unsafe { CloseClipboard() };
    result
}

#[cfg(windows)]
const CF_UNICODETEXT: u32 = 13;
#[cfg(windows)]
const CF_DIB: u32 = 8;
#[cfg(windows)]
const CF_HDROP: u32 = 15;

/// Copies the files (as a file list, marked to be copied, not moved), the paths (as text) or the picture of the first file. Returns how many files it covered.
/// `Photo` is the files and, for one photo that can be read, its picture too.
#[cfg(windows)]
pub fn copy(form: Form, paths: &[PathBuf]) -> Result<usize, String> {
    match form {
        Form::Photo => {
            let list = drop_files(&paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>());
            let mut items = vec![(Format::Id(CF_HDROP), list), (Format::Named("Preferred DropEffect"), 1u32.to_le_bytes().to_vec())];
            if let [path] = paths {
                // (a picture that cannot be read is still copied as a file)
                if let Ok(img) = pixie_image::decode::decode_oriented(path) {
                    let img = img.to_rgb8();
                    items.push((Format::Id(CF_DIB), dib_24(img.width(), img.height(), img.as_raw())));
                }
            }
            set_clipboard(&items)?;
            Ok(paths.len())
        }
        Form::Files => {
            let list = drop_files(&paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>());
            // "Preferred DropEffect" 1 is Copy: Explorer then copies on Paste and does not move the originals.
            set_clipboard(&[(Format::Id(CF_HDROP), list), (Format::Named("Preferred DropEffect"), 1u32.to_le_bytes().to_vec())])?;
            Ok(paths.len())
        }
        Form::Paths => {
            let mut wide: Vec<u8> = paths_text(paths, true).encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            wide.extend_from_slice(&[0, 0]);
            set_clipboard(&[(Format::Id(CF_UNICODETEXT), wide)])?;
            Ok(paths.len())
        }
        Form::Image => {
            let [path] = paths else { return Err("Only one picture can be copied as an image at a time.".into()) };
            let img = pixie_image::decode::decode_oriented(path).map_err(|e| format!("The picture could not be read: {e}"))?.to_rgb8();
            set_clipboard(&[(Format::Id(CF_DIB), dib_24(img.width(), img.height(), img.as_raw()))])?;
            Ok(1)
        }
    }
}

/// The system pasteboard of a Mac: the files as file URLs (Finder copies them on Paste, and mail and chat programs attach them), the picture as
/// PNG, and text.
#[cfg(target_os = "macos")]
mod pasteboard {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSData, NSString, NSURL};
    use std::path::PathBuf;

    /// Puts these on the pasteboard in place of what is there: file URLs for the files, then a PNG and a text on the same item if given.
    pub fn write(files: &[PathBuf], png: Option<&[u8]>, text: Option<&str>) -> Result<(), String> {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        if !files.is_empty() {
            let urls: Vec<_> = files.iter().map(|p| ProtocolObject::<dyn NSPasteboardWriting>::from_retained(NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())))).collect();
            if !pb.writeObjects(&NSArray::from_retained_slice(&urls)) {
                return Err("The clipboard did not take the files.".into());
            }
        }
        if let Some(bytes) = png {
            if !unsafe { pb.setData_forType(Some(&NSData::with_bytes(bytes)), NSPasteboardTypePNG) } {
                return Err("The clipboard did not take the picture.".into());
            }
        }
        if let Some(text) = text {
            if !unsafe { pb.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) } {
                return Err("The clipboard did not take the text.".into());
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use objc2_app_kit::NSPasteboardTypeFileURL;

        /// Uses the real clipboard of the machine, so it only runs when asked for: `cargo test -p pixie-app --lib pasteboard -- --ignored`.
        #[test]
        #[ignore = "replaces what is on the clipboard"]
        fn the_files_and_the_picture_are_on_the_pasteboard_together() {
            write(&[PathBuf::from("/tmp/pixie-clipboard-test.jpg")], Some(&[137, 80, 78, 71]), None).unwrap();
            let types: Vec<String> = NSPasteboard::generalPasteboard().types().map(|t| t.iter().map(|s| s.to_string()).collect()).unwrap_or_default();
            assert!(types.contains(&unsafe { NSPasteboardTypeFileURL }.to_string()), "{types:?}");
            assert!(types.contains(&unsafe { NSPasteboardTypePNG }.to_string()), "{types:?}");
            write(&[], None, Some("one
two")).unwrap();
            let text = unsafe { NSPasteboard::generalPasteboard().stringForType(NSPasteboardTypeString) }.map(|s| s.to_string());
            assert_eq!(text.as_deref(), Some("one
two"));
        }
    }
}

/// The picture of a file as PNG bytes, upright.
#[cfg(target_os = "macos")]
fn png_of(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let img = pixie_image::decode::decode_oriented(path).map_err(|e| format!("The picture could not be read: {e}"))?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Copies like the Windows version: `Photo` is the files and, for one photo that can be read, its picture too.
#[cfg(target_os = "macos")]
pub fn copy(form: Form, paths: &[PathBuf]) -> Result<usize, String> {
    match form {
        Form::Photo => {
            // (a picture that cannot be read is still copied as a file)
            let png = if let [path] = paths { png_of(path).ok() } else { None };
            pasteboard::write(paths, png.as_deref(), None)?;
        }
        Form::Files => pasteboard::write(paths, None, None)?,
        Form::Paths => pasteboard::write(&[], None, Some(&paths_text(paths, false)))?,
        Form::Image => {
            let [path] = paths else { return Err("Only one picture can be copied as an image at a time.".into()) };
            pasteboard::write(&[], Some(&png_of(path)?), None)?;
        }
    }
    Ok(paths.len())
}

/// Not built for this system yet.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn copy(_form: Form, _paths: &[PathBuf]) -> Result<usize, String> {
    Err("Copying to the clipboard is not available on this system yet.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_is_read_from_its_name() {
        assert_eq!(Form::parse("photo"), Some(Form::Photo));
        assert_eq!(Form::parse("files"), Some(Form::Files));
        assert_eq!(Form::parse("image"), Some(Form::Image));
        assert_eq!(Form::parse("paths"), Some(Form::Paths));
        assert_eq!(Form::parse("zip"), None);
    }

    #[test]
    fn paths_are_one_per_line_with_the_lines_the_system_uses() {
        let paths = [PathBuf::from("C:/a/one.jpg"), PathBuf::from("C:/a/two.jpg")];
        assert_eq!(paths_text(&paths, true), "C:/a/one.jpg\r\nC:/a/two.jpg");
        assert_eq!(paths_text(&paths, false), "C:/a/one.jpg\nC:/a/two.jpg");
        assert_eq!(paths_text(&paths[..1], true), "C:/a/one.jpg", "no line end after the last");
    }

    #[test]
    fn a_file_list_has_the_header_then_each_name_and_a_final_nul() {
        let list = drop_files(&["C:\\a.jpg".to_string(), "é.png".to_string()]);
        assert_eq!(u32::from_le_bytes(list[0..4].try_into().unwrap()), 20, "the names start after the header");
        assert_eq!(u32::from_le_bytes(list[16..20].try_into().unwrap()), 1, "wide characters");
        let units: Vec<u16> = list[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let text: Vec<String> = units.split(|u| *u == 0).map(|s| String::from_utf16(s).unwrap()).collect();
        assert_eq!(text, ["C:\\a.jpg", "é.png", "", ""], "each name ends in NUL, and the list in one more");
        assert_eq!(list.len() % 2, 0);
    }

    #[test]
    fn a_bitmap_has_its_header_and_rows_bottom_first_in_blue_green_red_padded_to_four_bytes() {
        // 3 wide, 2 high: top row red, green, blue; bottom row white, black, white.
        let rgb = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255];
        let dib = dib_24(3, 2, &rgb);
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(dib[4..8].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), 2, "positive height: bottom-up");
        assert_eq!(u16::from_le_bytes(dib[14..16].try_into().unwrap()), 24);
        assert_eq!(u32::from_le_bytes(dib[20..24].try_into().unwrap()), 24, "two rows of 12 bytes (9 and 3 of padding)");
        let rows = &dib[40..];
        assert_eq!(rows.len(), 24);
        assert_eq!(&rows[0..12], &[255, 255, 255, 0, 0, 0, 255, 255, 255, 0, 0, 0], "the bottom row first");
        assert_eq!(&rows[12..24], &[0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0], "then the top row: red, green, blue as BGR");
    }
}
