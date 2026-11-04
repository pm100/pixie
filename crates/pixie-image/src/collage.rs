//! Collage: photos placed as frames on a page, saved as a PNG or JPEG (see docs/superpowers/specs/2026-11-03-collage-editor-design.md).
//!
//! The window previews the page with CSS; `render` draws the saved file from the original photos. Both follow the same rules: frames
//! bottom to top, each photo cropped to fill its frame around a focus point (and a zoom), a border band inside the frame edge, rounded
//! corners on the outline, and a soft shadow under the frame.

use crate::decode::decode_scaled;
use image::{imageops, GenericImageView, GrayImage, Luma, Rgb, RgbImage};
use pixie_core::{PixieError, Result};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The longest side of a saved page, in pixels.
pub const MAX_SIDE_PX: u32 = 16_384;

/// The shadow's offset and blur, as a share of the shorter side of the page, and how dark it is.
const SHADOW_SIZE: f32 = 0.015;
const SHADOW_OPACITY: f32 = 0.35;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A cell as fractions of the page (0 to 1). The templates are made of these; the window turns them into frames.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct CellFrac {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Serialize, Clone, Debug)]
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub cells: Vec<CellFrac>,
}

/// A rectangle in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

fn cell(x: f32, y: f32, w: f32, h: f32) -> CellFrac {
    CellFrac { x, y, w, h }
}

/// `cols` by `rows` equal cells, row by row.
fn grid(cols: u32, rows: u32) -> Vec<CellFrac> {
    let (w, h) = (1.0 / cols as f32, 1.0 / rows as f32);
    (0..rows).flat_map(|r| (0..cols).map(move |c| cell(c as f32 * w, r as f32 * h, w, h))).collect()
}

/// Every template, the plainest first.
pub fn templates() -> Vec<Template> {
    let t = |id, label, cells| Template { id, label, cells };
    let third = 1.0 / 3.0;
    vec![
        t("single", "One photo", grid(1, 1)),
        t("two-across", "Two side by side", grid(2, 1)),
        t("two-down", "Two one above the other", grid(1, 2)),
        t("three-across", "Three side by side", grid(3, 1)),
        t("three-down", "Three one above the other", grid(1, 3)),
        t("grid-2x2", "Four in a grid", grid(2, 2)),
        t("four-across", "Four side by side", grid(4, 1)),
        t("one-big-two-small", "One big, two small", vec![cell(0.0, 0.0, 0.5, 1.0), cell(0.5, 0.0, 0.5, 0.5), cell(0.5, 0.5, 0.5, 0.5)]),
        t("big-top-two-below", "One big on top, two below", vec![cell(0.0, 0.0, 1.0, 0.6), cell(0.0, 0.6, 0.5, 0.4), cell(0.5, 0.6, 0.5, 0.4)]),
        t("one-big-three-small", "One big, three small", vec![cell(0.0, 0.0, 1.0, 2.0 * third), cell(0.0, 2.0 * third, third, third), cell(third, 2.0 * third, third, third), cell(2.0 * third, 2.0 * third, third, third)]),
        t("grid-3x2", "Six in a grid", grid(3, 2)),
        t("grid-3x3", "Nine in a grid", grid(3, 3)),
    ]
}

/// The part of a `src_w` by `src_h` photo that fills a `cell_w` by `cell_h` cell: the biggest rectangle of the cell's shape that fits in
/// the photo, shrunk by `zoom` (1 to 4) around the same point, and placed by the focus (0 is the left or top, 1 the right or bottom). The
/// same as CSS `object-fit: cover` with `object-position` set to the focus, scaled up by the zoom. Returns `(x, y, w, h)`.
pub fn cover_crop(src_w: u32, src_h: u32, cell_w: u32, cell_h: u32, focus_x: f32, focus_y: f32, zoom: f32) -> (u32, u32, u32, u32) {
    let (sw, sh, cw, ch) = (src_w.max(1) as u64, src_h.max(1) as u64, cell_w.max(1) as u64, cell_h.max(1) as u64);
    // the photo is wider than the cell's shape when sw / sh > cw / ch
    let (w, h) = if sw * ch > cw * sh { ((sh * cw / ch).clamp(1, sw), sh) } else { (sw, (sw * ch / cw).clamp(1, sh)) };
    let zoom = if zoom.is_finite() { zoom.clamp(1.0, 4.0) as f64 } else { 1.0 };
    let (w, h) = (((w as f64 / zoom).round() as u64).clamp(1, w), ((h as f64 / zoom).round() as u64).clamp(1, h));
    let at = |free: u64, f: f32| (free as f64 * f.clamp(0.0, 1.0) as f64).round() as u32;
    (at(sw - w, focus_x), at(sh - h, focus_y), w as u32, h as u32)
}

/// A photo in a frame. `x`, `y`, `w` and `h` are fractions of the page.
pub struct Frame {
    pub path: PathBuf,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub focus_x: f32,
    pub focus_y: f32,
    pub zoom: f32,
}

/// How the frames look. They apply to every frame.
pub struct Look {
    pub corner_mm: f32,
    pub border_mm: f32,
    pub border: [u8; 3],
    pub shadow: bool,
}

pub struct CollageRequest {
    pub page_w_mm: f32,
    pub page_h_mm: f32,
    /// 72 to 600.
    pub dpi: u32,
    pub background: [u8; 3],
    pub look: Look,
    /// Bottom to top.
    pub frames: Vec<Frame>,
}

fn mm_to_px(mm: f32, dpi: u32) -> u32 {
    (mm.max(0.0) as f64 / 25.4 * dpi as f64).round() as u32
}

/// The size of the page in pixels, or why it cannot be made.
pub fn page_px(page_w_mm: f32, page_h_mm: f32, dpi: u32) -> Result<(u32, u32)> {
    if !(72..=600).contains(&dpi) {
        return Err(PixieError::Invalid("The resolution must be between 72 and 600 dpi.".into()));
    }
    let (w, h) = (mm_to_px(page_w_mm, dpi).max(1), mm_to_px(page_h_mm, dpi).max(1));
    check_px(w, h)?;
    Ok((w, h))
}

/// Refuses a page whose longest side is over the limit, in words for the person.
pub fn check_px(w: u32, h: u32) -> Result<()> {
    if w > MAX_SIDE_PX || h > MAX_SIDE_PX {
        return Err(PixieError::Invalid(format!("That page would be {w} by {h} pixels; the longest side can be at most {MAX_SIDE_PX}. Choose a smaller page or a lower resolution.")));
    }
    Ok(())
}

/// Is the pixel `(x, y)` of a `w` by `h` rectangle inside its rounded corners (radius `r`)?
fn inside_corners(x: u32, y: u32, w: u32, h: u32, r: u32) -> bool {
    if r == 0 {
        return true;
    }
    let near = |p: u32, len: u32| -> Option<f32> {
        if p < r {
            Some(r as f32 - p as f32 - 0.5)
        } else if p >= len - r {
            Some(p as f32 - (len - r) as f32 + 0.5)
        } else {
            None
        }
    };
    match (near(x, w), near(y, h)) {
        (Some(dx), Some(dy)) => dx * dx + dy * dy <= (r * r) as f32,
        _ => true,
    }
}

/// The frame in pixels: its fractions rounded, kept on the page, at least one pixel each way.
fn frame_rect(f: &Frame, page_w: u32, page_h: u32) -> PxRect {
    let span = |start: f32, len: f32, page: u32| -> (u32, u32) {
        let a = ((start.clamp(0.0, 1.0) as f64) * page as f64).round().min(page as f64 - 1.0) as u32;
        let b = (((start + len).clamp(0.0, 1.0) as f64) * page as f64).round().clamp(a as f64 + 1.0, page as f64) as u32;
        (a, b - a)
    };
    let ((x, w), (y, h)) = (span(f.x, f.w, page_w), span(f.y, f.h, page_h));
    PxRect { x, y, w, h }
}

/// Darken the page under and around the frame: its outline, moved down and right and blurred.
fn draw_shadow(page: &mut RgbImage, rect: PxRect, radius: u32) {
    let size = (SHADOW_SIZE * page.width().min(page.height()) as f32).max(1.0);
    let (offset, reach) = (size.round() as i64, (size * 3.0).ceil() as i64);
    let (pw, ph) = (page.width() as i64, page.height() as i64);
    // the part of the page the shadow can reach
    let (x0, y0) = ((rect.x as i64 + offset - reach).max(0), (rect.y as i64 + offset - reach).max(0));
    let (x1, y1) = ((rect.x as i64 + rect.w as i64 + offset + reach).min(pw), (rect.y as i64 + rect.h as i64 + offset + reach).min(ph));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let mut mask = GrayImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    for y in 0..rect.h {
        for x in 0..rect.w {
            let (mx, my) = (rect.x as i64 + offset + x as i64 - x0, rect.y as i64 + offset + y as i64 - y0);
            if mx >= 0 && my >= 0 && mx < mask.width() as i64 && my < mask.height() as i64 && inside_corners(x, y, rect.w, rect.h, radius) {
                mask.put_pixel(mx as u32, my as u32, Luma([255]));
            }
        }
    }
    let mask = imageops::blur(&mask, size);
    for (mx, my, m) in mask.enumerate_pixels() {
        let alpha = m.0[0] as f32 / 255.0 * SHADOW_OPACITY;
        if alpha > 0.0 {
            let px = page.get_pixel_mut((x0 + mx as i64) as u32, (y0 + my as i64) as u32);
            for c in px.0.iter_mut() {
                *c = (*c as f32 * (1.0 - alpha)).round() as u8;
            }
        }
    }
}

/// Draw the page. Photos are read from the original files one at a time, so a poster does not need them all in memory at once.
pub fn render(req: &CollageRequest) -> Result<RgbImage> {
    let (pw, ph) = page_px(req.page_w_mm, req.page_h_mm, req.dpi)?;
    let mut page = RgbImage::from_pixel(pw, ph, Rgb(req.background));
    for frame in &req.frames {
        let rect = frame_rect(frame, pw, ph);
        let radius = mm_to_px(req.look.corner_mm, req.dpi).min(rect.w.min(rect.h) / 2);
        if req.look.shadow {
            draw_shadow(&mut page, rect, radius);
        }
        // the photo sits inside the border band, which leaves at least a pixel of photo
        let band = mm_to_px(req.look.border_mm, req.dpi).min((rect.w.min(rect.h) - 1) / 2);
        let (iw, ih) = (rect.w - 2 * band, rect.h - 2 * band);
        let name = frame.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Decoded no smaller than the frame needs once cropped to fill it (and zoomed in).
        let zoom = if frame.zoom.is_finite() { frame.zoom.clamp(1.0, 4.0) as f64 } else { 1.0 };
        let edge = match image::image_dimensions(&frame.path) {
            Ok((sw, sh)) if sw > 0 && sh > 0 => {
                let scale = (iw as f64 / sw as f64).max(ih as f64 / sh as f64) * zoom;
                ((sw.max(sh) as f64 * scale).ceil() as u32).clamp(1, MAX_SIDE_PX)
            }
            _ => MAX_SIDE_PX,
        };
        let img = decode_scaled(&frame.path, edge).map_err(|e| PixieError::Image(format!("{name} could not be read: {e}")))?;
        let (sw, sh) = img.dimensions();
        let (cx, cy, cw, ch) = cover_crop(sw, sh, iw, ih, frame.focus_x, frame.focus_y, frame.zoom);
        let fitted = imageops::resize(&img.crop_imm(cx, cy, cw, ch).to_rgb8(), iw, ih, imageops::FilterType::Lanczos3);
        for y in 0..rect.h {
            for x in 0..rect.w {
                if !inside_corners(x, y, rect.w, rect.h, radius) {
                    continue;
                }
                let inner = x >= band && y >= band && x < band + iw && y < band + ih;
                let colour = if inner { *fitted.get_pixel(x - band, y - band) } else { Rgb(req.look.border) };
                page.put_pixel(rect.x + x, rect.y + y, colour);
            }
        }
    }
    Ok(page)
}

// ---- saving ----

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The PNG `pHYs` chunk: the pixels per metre, so a reader knows the printed size.
fn phys_chunk(dpi: u32) -> Vec<u8> {
    let per_metre = (dpi as f64 / 0.0254).round() as u32;
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&per_metre.to_be_bytes());
    body.extend_from_slice(&per_metre.to_be_bytes());
    body.push(1); // unit: metre
    let mut chunk = 9u32.to_be_bytes().to_vec();
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    chunk
}

/// Passes a PNG through, adding the `pHYs` chunk right after the header chunk (8 bytes of signature, then 25 of IHDR).
struct PhysWriter<W: Write> {
    inner: W,
    dpi: u32,
    head: Vec<u8>,
    done: bool,
}

impl<W: Write> Write for PhysWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.done {
            return self.inner.write(buf);
        }
        let take = (33 - self.head.len()).min(buf.len());
        self.head.extend_from_slice(&buf[..take]);
        if self.head.len() == 33 {
            self.inner.write_all(&self.head)?;
            self.inner.write_all(&phys_chunk(self.dpi))?;
            self.done = true;
        }
        Ok(take)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn encode(img: &RgbImage, file: std::fs::File, png: bool, dpi: u32, jpeg_quality: u8) -> Result<()> {
    let mut out = std::io::BufWriter::new(file);
    if png {
        let mut phys = PhysWriter { inner: &mut out, dpi, head: Vec::new(), done: false };
        let enc = image::codecs::png::PngEncoder::new(&mut phys);
        image::ImageEncoder::write_image(enc, img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8).map_err(|e| PixieError::Image(e.to_string()))?;
    } else {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, jpeg_quality.clamp(1, 100));
        enc.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(dpi.min(u16::MAX as u32) as u16));
        enc.encode_image(img).map_err(|e| PixieError::Image(e.to_string()))?;
    }
    out.flush()?;
    Ok(())
}

/// Save the page as PNG or JPEG (by the extension of `dst`), recording `dpi` in the file. Written beside `dst` under a temporary name and
/// renamed, so a file already there survives a failure.
pub fn save(img: &RgbImage, dst: &Path, dpi: u32, jpeg_quality: u8) -> Result<()> {
    let ext = dst.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let png = match ext.as_str() {
        "png" => true,
        "jpg" | "jpeg" => false,
        _ => return Err(PixieError::Invalid("Save the collage as a .png or a .jpg file.".into())),
    };
    let tmp = dst.with_extension(format!("{}.tmp", TMP_COUNTER.fetch_add(1, Ordering::Relaxed)));
    let written = std::fs::File::create(&tmp).map_err(PixieError::from).and_then(|f| encode(img, f, png, dpi, jpeg_quality));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: [u8; 3] = [10, 20, 30];
    const RED: [u8; 3] = [255, 0, 0];
    const BLUE: [u8; 3] = [0, 0, 255];

    fn solid(dir: &Path, name: &str, w: u32, h: u32, rgb: [u8; 3]) -> PathBuf {
        let p = dir.join(name);
        RgbImage::from_pixel(w, h, Rgb(rgb)).save(&p).unwrap();
        p
    }

    /// A frame filling the given fractions, the middle of its photo showing, not zoomed.
    fn frame(path: PathBuf, x: f32, y: f32, w: f32, h: f32) -> Frame {
        Frame { path, x, y, w, h, focus_x: 0.5, focus_y: 0.5, zoom: 1.0 }
    }

    fn mm(px: u32) -> f32 {
        px as f32 / 72.0 * 25.4
    }

    /// A page of `w` by `h` pixels (72 dpi), with no corners, border or shadow.
    fn request(w: u32, h: u32, frames: Vec<Frame>) -> CollageRequest {
        CollageRequest { page_w_mm: mm(w), page_h_mm: mm(h), dpi: 72, background: BG, look: Look { corner_mm: 0.0, border_mm: 0.0, border: [255, 255, 255], shadow: false }, frames }
    }

    #[test]
    fn there_are_at_least_ten_templates_with_unique_ids() {
        let all = templates();
        assert!(all.len() >= 10);
        let mut ids: Vec<_> = all.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
    }

    #[test]
    fn every_template_cell_lies_on_the_page_and_none_overlap() {
        for t in templates() {
            for (i, a) in t.cells.iter().enumerate() {
                assert!(a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= 1.0001 && a.y + a.h <= 1.0001 && a.w > 0.0 && a.h > 0.0, "{}: cell {i} is off the page", t.id);
                for (j, b) in t.cells.iter().enumerate().skip(i + 1) {
                    let apart = a.x + a.w <= b.x + 0.0001 || b.x + b.w <= a.x + 0.0001 || a.y + a.h <= b.y + 0.0001 || b.y + b.h <= a.y + 0.0001;
                    assert!(apart, "{}: cells {i} and {j} overlap", t.id);
                }
            }
        }
    }

    #[test]
    fn cover_crop_of_a_wide_photo_in_a_square_cell_keeps_full_height() {
        assert_eq!(cover_crop(400, 200, 100, 100, 0.5, 0.5, 1.0), (100, 0, 200, 200));
        assert_eq!(cover_crop(400, 200, 100, 100, 0.0, 0.5, 1.0), (0, 0, 200, 200));
        assert_eq!(cover_crop(400, 200, 100, 100, 1.0, 0.5, 1.0), (200, 0, 200, 200));
    }

    #[test]
    fn cover_crop_of_a_tall_photo_in_a_wide_cell_keeps_full_width() {
        assert_eq!(cover_crop(200, 400, 100, 50, 0.5, 1.0, 1.0), (0, 300, 200, 100));
        assert_eq!(cover_crop(200, 400, 100, 50, 0.5, 7.0, 1.0), (0, 300, 200, 100), "a focus past the edge is clamped");
    }

    #[test]
    fn cover_crop_of_a_matching_shape_is_the_whole_photo() {
        assert_eq!(cover_crop(300, 200, 150, 100, 0.3, 0.9, 1.0), (0, 0, 300, 200));
    }

    #[test]
    fn cover_crop_with_zoom_two_shows_half_the_width_and_height() {
        assert_eq!(cover_crop(400, 200, 100, 100, 0.5, 0.5, 2.0), (150, 50, 100, 100));
        assert_eq!(cover_crop(400, 200, 100, 100, 0.5, 0.5, 9.0), cover_crop(400, 200, 100, 100, 0.5, 0.5, 4.0), "more than 4 counts as 4");
        assert_eq!(cover_crop(400, 200, 100, 100, 0.5, 0.5, 0.2), cover_crop(400, 200, 100, 100, 0.5, 0.5, 1.0), "less than 1 counts as 1");
        assert_eq!(cover_crop(400, 200, 100, 100, 0.5, 0.5, f32::NAN), (100, 0, 200, 200));
    }

    #[test]
    fn a_page_larger_than_the_limit_is_an_error_that_says_so() {
        let e = page_px(1000.0, 2000.0, 300).unwrap_err().to_string();
        assert!(e.contains("16384"), "{e}");
        assert_eq!(page_px(210.0, 297.0, 300).unwrap(), (2480, 3508));
        assert!(page_px(210.0, 297.0, 10).is_err());
    }

    #[test]
    fn a_frame_is_placed_by_its_fractions_and_the_rest_is_background() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 50, 50, RED);
        let page = render(&request(100, 100, vec![frame(red, 0.25, 0.25, 0.5, 0.5)])).unwrap();
        assert_eq!(page.dimensions(), (100, 100));
        assert_eq!(*page.get_pixel(50, 50), Rgb(RED));
        assert_eq!(*page.get_pixel(26, 50), Rgb(RED));
        assert_eq!(*page.get_pixel(24, 50), Rgb(BG));
        assert_eq!(*page.get_pixel(5, 5), Rgb(BG));
        assert_eq!(*render(&request(100, 100, vec![])).unwrap().get_pixel(50, 50), Rgb(BG), "no frames is a blank page");
    }

    #[test]
    fn frames_are_drawn_bottom_to_top() {
        let dir = tempfile::tempdir().unwrap();
        let (red, blue) = (solid(dir.path(), "r.png", 50, 50, RED), solid(dir.path(), "b.png", 50, 50, BLUE));
        let up = render(&request(100, 100, vec![frame(red.clone(), 0.0, 0.0, 0.6, 1.0), frame(blue.clone(), 0.4, 0.0, 0.6, 1.0)])).unwrap();
        assert_eq!(*up.get_pixel(50, 50), Rgb(BLUE), "the later frame is on top");
        assert_eq!(*up.get_pixel(10, 50), Rgb(RED));
        let down = render(&request(100, 100, vec![frame(blue, 0.4, 0.0, 0.6, 1.0), frame(red, 0.0, 0.0, 0.6, 1.0)])).unwrap();
        assert_eq!(*down.get_pixel(50, 50), Rgb(RED));
    }

    #[test]
    fn a_border_is_a_band_inside_the_frame_edge_around_the_photo() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 60, 60, RED);
        let mut req = request(100, 100, vec![frame(red, 0.0, 0.0, 1.0, 1.0)]);
        req.look.border_mm = mm(5);
        req.look.border = BLUE;
        let page = render(&req).unwrap();
        assert_eq!(*page.get_pixel(2, 50), Rgb(BLUE));
        assert_eq!(*page.get_pixel(50, 2), Rgb(BLUE));
        assert_eq!(*page.get_pixel(97, 50), Rgb(BLUE));
        assert_eq!(*page.get_pixel(8, 50), Rgb(RED));
        assert_eq!(*page.get_pixel(50, 50), Rgb(RED));
    }

    #[test]
    fn a_border_wider_than_the_frame_still_leaves_some_photo() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 60, 60, RED);
        let mut req = request(100, 100, vec![frame(red, 0.0, 0.0, 0.2, 0.2)]);
        req.look.border_mm = mm(500);
        req.look.border = BLUE;
        let page = render(&req).unwrap();
        assert_eq!(page.dimensions(), (100, 100));
        assert_eq!(*page.get_pixel(10, 10), Rgb(RED), "one pixel of photo in the middle");
        assert_eq!(*page.get_pixel(1, 10), Rgb(BLUE));
    }

    #[test]
    fn corners_are_rounded_to_the_background_with_or_without_a_border() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 60, 60, RED);
        for border in [0u32, 3] {
            let mut req = request(100, 100, vec![frame(red.clone(), 0.0, 0.0, 1.0, 1.0)]);
            req.look.corner_mm = mm(10);
            req.look.border_mm = mm(border);
            req.look.border = BLUE;
            let page = render(&req).unwrap();
            assert_eq!(*page.get_pixel(0, 0), Rgb(BG), "the corner is cut off (border {border})");
            assert_eq!(*page.get_pixel(99, 99), Rgb(BG));
            assert_eq!(*page.get_pixel(50, 50), Rgb(RED));
            let edge = *page.get_pixel(50, 0);
            assert_eq!(edge, if border == 0 { Rgb(RED) } else { Rgb(BLUE) }, "the straight edge is kept (border {border})");
        }
    }

    #[test]
    fn a_shadow_darkens_below_and_right_of_a_frame_only_when_on() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 50, 50, RED);
        let frames = || vec![frame(red.clone(), 0.25, 0.25, 0.5, 0.5)]; // on a 200 px page: 50 to 150
        let mut req = request(200, 200, frames());
        req.background = [255, 255, 255];
        let plain = render(&req).unwrap();
        assert_eq!(*plain.get_pixel(152, 152), Rgb([255, 255, 255]));
        req.look.shadow = true;
        let shadowed = render(&req).unwrap();
        assert!(shadowed.get_pixel(152, 152).0[0] < 245, "below and right of the frame is darker: {:?}", shadowed.get_pixel(152, 152));
        assert!(shadowed.get_pixel(152, 152).0[0] < shadowed.get_pixel(47, 47).0[0], "more than above and left");
        assert_eq!(*shadowed.get_pixel(195, 195), Rgb([255, 255, 255]), "far away is untouched");
        assert_eq!(*shadowed.get_pixel(100, 100), Rgb(RED), "the frame itself is not darkened");
    }

    #[test]
    fn a_shadow_falls_under_the_frames_above_it_not_on_them() {
        let dir = tempfile::tempdir().unwrap();
        let (red, blue) = (solid(dir.path(), "r.png", 50, 50, RED), solid(dir.path(), "b.png", 50, 50, BLUE));
        let mut req = request(200, 200, vec![frame(red, 0.1, 0.1, 0.5, 0.5), frame(blue, 0.4, 0.4, 0.5, 0.5)]);
        req.look.shadow = true;
        let page = render(&req).unwrap();
        assert_eq!(*page.get_pixel(120, 120), Rgb(BLUE), "the upper frame is its own colour");
        assert_eq!(*page.get_pixel(60, 60), Rgb(RED));
    }

    #[test]
    fn zoom_and_focus_choose_the_visible_part() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("wide.png");
        RgbImage::from_fn(200, 100, |x, _| if x < 100 { Rgb(RED) } else { Rgb(BLUE) }).save(&p).unwrap();
        let mut f = frame(p.clone(), 0.0, 0.0, 1.0, 1.0);
        f.focus_x = 0.0;
        let left = render(&request(100, 100, vec![f])).unwrap();
        assert_eq!(*left.get_pixel(98, 50), Rgb(RED));
        let mut f = frame(p, 0.0, 0.0, 1.0, 1.0);
        f.focus_x = 1.0;
        f.zoom = 2.0;
        let right = render(&request(100, 100, vec![f])).unwrap();
        assert_eq!(*right.get_pixel(2, 50), Rgb(BLUE), "zoomed in on the right edge");
        assert_eq!(*right.get_pixel(50, 50), Rgb(BLUE));
    }

    #[test]
    fn a_frame_off_the_page_is_clamped_and_a_zero_size_frame_is_one_pixel() {
        let dir = tempfile::tempdir().unwrap();
        let red = solid(dir.path(), "r.png", 50, 50, RED);
        let page = render(&request(100, 100, vec![frame(red.clone(), 0.9, 0.9, 0.5, 0.5), frame(red.clone(), 0.3, 0.3, 0.0, 0.0), frame(red, 1.5, -2.0, 0.2, 0.2)])).unwrap();
        assert_eq!(page.dimensions(), (100, 100));
        assert_eq!(*page.get_pixel(95, 95), Rgb(RED));
        assert_eq!(*page.get_pixel(30, 30), Rgb(RED), "a frame with no size is a pixel, not a panic");
    }

    #[test]
    fn a_photo_that_cannot_be_read_fails_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("broken.jpg");
        std::fs::write(&bad, b"not a picture").unwrap();
        let e = render(&request(100, 100, vec![frame(bad, 0.0, 0.0, 1.0, 1.0)])).unwrap_err().to_string();
        assert!(e.contains("broken.jpg"), "{e}");
        let gone = render(&request(100, 100, vec![frame(dir.path().join("gone.jpg"), 0.0, 0.0, 1.0, 1.0)])).unwrap_err().to_string();
        assert!(gone.contains("gone.jpg"), "{gone}");
    }

    #[test]
    fn a_poster_size_page_renders_and_gives_back_its_physical_size() {
        let dir = tempfile::tempdir().unwrap();
        let cells = grid(2, 2);
        let frames = cells.iter().enumerate().map(|(i, c)| frame(solid(dir.path(), &format!("p{i}.png"), 600, 800, [200, 50 * i as u8, 0]), c.x + 0.01, c.y + 0.01, c.w - 0.02, c.h - 0.02)).collect();
        // 70 x 100 cm at 150 dpi is 4134 x 5906 pixels
        let mut req = CollageRequest { page_w_mm: 700.0, page_h_mm: 1000.0, dpi: 150, background: [255, 255, 255], look: Look { corner_mm: 5.0, border_mm: 3.0, border: [255, 255, 255], shadow: true }, frames };
        req.look.shadow = true;
        let page = render(&req).unwrap();
        assert_eq!(page.dimensions(), (4134, 5906));
        let dst = dir.path().join("poster.png");
        save(&page, &dst, 150, 90).unwrap();
        let bytes = std::fs::read(&dst).unwrap();
        let at = bytes.windows(4).position(|w| w == b"pHYs").unwrap();
        let per_metre = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap());
        let inches = 4134.0 / (per_metre as f64 * 0.0254);
        assert!((inches * 25.4 - 700.0).abs() < 1.0, "the recorded dpi gives {} mm across", inches * 25.4);
    }

    fn page() -> RgbImage {
        RgbImage::from_pixel(40, 30, Rgb([1, 2, 3]))
    }

    #[test]
    fn saving_as_png_and_jpeg_makes_a_readable_file() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["out.png", "out.jpg", "OUT.JPEG"] {
            let dst = dir.path().join(name);
            save(&page(), &dst, 300, 90).unwrap();
            assert_eq!(image::open(&dst).unwrap().dimensions(), (40, 30), "{name}");
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3, "no temporary file is left");
    }

    #[test]
    fn the_saved_file_records_its_dpi() {
        let dir = tempfile::tempdir().unwrap();
        let (png, jpg) = (dir.path().join("a.png"), dir.path().join("a.jpg"));
        save(&page(), &png, 300, 90).unwrap();
        save(&page(), &jpg, 150, 90).unwrap();
        let bytes = std::fs::read(&png).unwrap();
        let at = bytes.windows(4).position(|w| w == b"pHYs").expect("a pHYs chunk");
        let per_metre = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap());
        assert_eq!(per_metre, 11811, "300 dpi is 11811 pixels per metre");
        assert_eq!(bytes[at + 12], 1, "in metres");
        let bytes = std::fs::read(&jpg).unwrap();
        let at = bytes.windows(5).position(|w| w == b"JFIF\0").expect("a JFIF header");
        assert_eq!(bytes[at + 7], 1, "units are dots per inch");
        assert_eq!(u16::from_be_bytes([bytes[at + 8], bytes[at + 9]]), 150);
    }

    #[test]
    fn saving_with_an_unknown_extension_fails_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(save(&page(), &dir.path().join("out.bmp"), 300, 90).is_err());
        assert!(save(&page(), &dir.path().join("noext"), 300, 90).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_failed_save_leaves_an_existing_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.png");
        std::fs::write(&dst, b"keep me").unwrap();
        // the folder for the temporary file is gone: the write fails before anything is put in place
        let missing = dir.path().join("gone").join("out.png");
        assert!(save(&page(), &missing, 300, 90).is_err());
        assert_eq!(std::fs::read(&dst).unwrap(), b"keep me");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
