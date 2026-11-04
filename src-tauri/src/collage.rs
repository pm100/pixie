//! The Collage tool's commands: the templates, the collage kept between sessions, and saving the page as a picture.

use crate::state::AppState;
use pixie_core::PixieError;
use pixie_image::collage::{render, save, templates, CollageRequest, Frame, Look, Template};
use pixie_index::Db;
use serde::Deserialize;

/// The collage is kept as one piece of text in the settings, written by the window; the app only stores it.
const KEY: &str = "collage";
/// A collage is a few hundred bytes a frame; this is far more than anyone makes.
const MAX_DOC_BYTES: usize = 1_000_000;

/// A photo in a frame, and where the frame is on the page (fractions), with the part of the photo shown (the focus point, 0 to 1, and the zoom).
#[derive(Deserialize)]
pub struct FrameArg {
    photo_id: i64,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
}

#[derive(Deserialize)]
pub struct SaveArgs {
    paper_w_mm: f32,
    paper_h_mm: f32,
    dpi: u32,
    /// `#rrggbb`.
    background: String,
    corner_mm: f32,
    border_mm: f32,
    /// `#rrggbb`.
    border_colour: String,
    shadow: bool,
    /// Bottom to top.
    frames: Vec<FrameArg>,
}

fn parse_colour(text: &str) -> Result<[u8; 3], String> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    let byte = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
    match (hex.len(), byte(0), byte(2), byte(4)) {
        (6, Some(r), Some(g), Some(b)) => Ok([r, g, b]),
        _ => Err(format!("\"{text}\" is not a colour (it should look like #rrggbb).")),
    }
}

/// The text for the person: the picture errors already name the file and say what is wrong.
pub(crate) fn say(e: PixieError) -> String {
    match e {
        PixieError::Image(m) | PixieError::Invalid(m) => m,
        other => other.user_message(),
    }
}

fn store(db: &Db, doc: &str) -> Result<(), String> {
    if doc.len() > MAX_DOC_BYTES {
        return Err("The collage is too big to keep.".into());
    }
    db.set_setting(KEY, doc).map_err(|e| e.user_message())
}

fn load(db: &Db) -> Result<Option<String>, String> {
    db.get_setting(KEY).map_err(|e| e.user_message())
}

#[tauri::command]
pub fn collage_templates() -> Vec<Template> {
    templates()
}

/// The collage kept last time, as the text the window stored, or null when there is none.
#[tauri::command]
pub fn get_collage(state: tauri::State<AppState>) -> Result<Option<String>, String> {
    load(&state.db)
}

/// Keep the collage (as the text the window made of it).
#[tauri::command]
pub fn set_collage(state: tauri::State<AppState>, doc: String) -> Result<(), String> {
    store(&state.db, &doc)
}

/// Draw the page from the original files and save it as a PNG or JPEG (by the extension of `dst`).
#[tauri::command]
pub async fn save_collage(state: tauri::State<'_, AppState>, args: SaveArgs, dst: String) -> Result<(), String> {
    let background = parse_colour(&args.background)?;
    let border = parse_colour(&args.border_colour)?;
    let mut frames = Vec::new();
    for f in &args.frames {
        let p = state.db.get_photo(f.photo_id).map_err(|e| e.user_message())?.filter(|p| !p.missing).ok_or("A photo in the collage is no longer in the library.")?;
        frames.push(Frame { path: p.path.into(), x: f.x, y: f.y, w: f.w, h: f.h, focus_x: f.focus_x, focus_y: f.focus_y, zoom: f.zoom });
    }
    let dpi = args.dpi;
    let req = CollageRequest { page_w_mm: args.paper_w_mm, page_h_mm: args.paper_h_mm, dpi, background, look: Look { corner_mm: args.corner_mm, border_mm: args.border_mm, border, shadow: args.shadow }, frames };
    tauri::async_runtime::spawn_blocking(move || {
        let page = render(&req).map_err(say)?;
        save(&page, std::path::Path::new(&dst), dpi, 92).map_err(say)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_read_from_hex() {
        assert_eq!(parse_colour("#ff8000").unwrap(), [255, 128, 0]);
        assert_eq!(parse_colour("00ff00").unwrap(), [0, 255, 0]);
        assert!(parse_colour("#fff").is_err());
        assert!(parse_colour("#gggggg").is_err());
        assert!(parse_colour("").is_err());
    }

    #[test]
    fn a_stored_document_comes_back_as_it_was() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(load(&db).unwrap(), None);
        store(&db, "{\"v\":1}").unwrap();
        assert_eq!(load(&db).unwrap().as_deref(), Some("{\"v\":1}"));
        store(&db, "{\"v\":1,\"frames\":[]}").unwrap();
        assert_eq!(load(&db).unwrap().as_deref(), Some("{\"v\":1,\"frames\":[]}"), "the new text replaces the old");
    }

    #[test]
    fn a_document_over_the_limit_is_refused_and_the_old_one_stays() {
        let db = Db::open_in_memory().unwrap();
        store(&db, "small").unwrap();
        assert!(store(&db, &"x".repeat(MAX_DOC_BYTES + 1)).is_err());
        assert_eq!(load(&db).unwrap().as_deref(), Some("small"));
    }
}
