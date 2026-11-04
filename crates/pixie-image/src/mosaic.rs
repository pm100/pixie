//! Photomosaic: rebuild a picture as a grid of small square photos.

use crate::collage::{check_px, cover_crop, mm_to_px, MAX_SIDE_PX};
use crate::decode::decode_scaled;
use image::{imageops, DynamicImage, GenericImageView, RgbImage};
use pixie_core::{PixieError, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The side of the miniature kept for every photo, in pixels.
const SIG_EDGE: u32 = 12;

/// What is kept of a photo to use it as a tile: its average colour, and a small picture of its centre square (`SIG_EDGE` square, RGB).
#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub avg: [u8; 3],
    pub tiny: Vec<u8>,
}

/// The signature of a picture that is already upright.
pub fn signature(img: &DynamicImage) -> Signature {
    let (w, h) = img.dimensions();
    let side = w.min(h).max(1);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side).to_rgb8();
    let tiny = imageops::resize(&square, SIG_EDGE, SIG_EDGE, imageops::FilterType::Triangle).into_raw();
    let n = (tiny.len() / 3) as u64;
    let mut sum = [0u64; 3];
    for px in tiny.chunks_exact(3) {
        for (s, c) in sum.iter_mut().zip(px) {
            *s += *c as u64;
        }
    }
    Signature { avg: sum.map(|s| ((s + n / 2) / n) as u8), tiny }
}

/// The side of a tile in pixels.
pub fn tile_px(tile_mm: f32, dpi: u32) -> u32 {
    mm_to_px(tile_mm, dpi).max(1)
}

/// How many whole tiles fit across and down (at least one each).
pub fn grid_size(page_w_px: u32, page_h_px: u32, tile_px: u32) -> (u32, u32) {
    let t = tile_px.max(1);
    ((page_w_px / t).max(1), (page_h_px / t).max(1))
}

/// Where cell `(col, row)` lies on a `w` by `h` page cut into `cols` by `rows` cells: the cells share the page out exactly, so the
/// last tile in a row may be a pixel or two wider than the first. Returns `(x, y, w, h)`, at least one pixel each way.
pub fn cell_rect(col: u32, row: u32, cols: u32, rows: u32, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let edge = |i: u32, n: u32, len: u32| (i as u64 * len as u64 / n as u64) as u32;
    let (x0, x1) = (edge(col, cols, w), edge(col + 1, cols, w));
    let (y0, y1) = (edge(row, rows, h), edge(row + 1, rows, h));
    (x0.min(w.saturating_sub(1)), y0.min(h.saturating_sub(1)), (x1 - x0).max(1), (y1 - y0).max(1))
}

/// The mean colour of every cell of the target, row by row.
pub fn cell_colours(target: &RgbImage, cols: u32, rows: u32) -> Vec<[u8; 3]> {
    let (w, h) = target.dimensions();
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let (x, y, cw, ch) = cell_rect(c, r, cols, rows, w, h);
            let mut sum = [0u64; 3];
            for py in y..(y + ch).min(h) {
                for px in x..(x + cw).min(w) {
                    for (s, v) in sum.iter_mut().zip(target.get_pixel(px, py).0) {
                        *s += v as u64;
                    }
                }
            }
            let n = (cw as u64 * ch as u64).max(1);
            out.push(sum.map(|s| ((s + n / 2) / n) as u8));
        }
    }
    out
}

// ---- choosing the photo for every cell ----

/// A colour in CIELAB (D65), where distance is close to how different two colours look.
fn lab(rgb: [u8; 3]) -> [f32; 3] {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// A k-d tree over colours: finds the nearest one that a caller-supplied test allows.
struct KdTree {
    points: Vec<[f32; 3]>,
    /// Candidate numbers, arranged so the middle of each range is its node.
    order: Vec<u32>,
}

impl KdTree {
    fn new(points: Vec<[f32; 3]>) -> KdTree {
        let mut order: Vec<u32> = (0..points.len() as u32).collect();
        fn build(points: &[[f32; 3]], order: &mut [u32], axis: usize) {
            if order.len() <= 1 {
                return;
            }
            let mid = order.len() / 2;
            order.select_nth_unstable_by(mid, |a, b| points[*a as usize][axis].total_cmp(&points[*b as usize][axis]).then(a.cmp(b)));
            let (lo, rest) = order.split_at_mut(mid);
            build(points, lo, (axis + 1) % 3);
            build(points, &mut rest[1..], (axis + 1) % 3);
        }
        build(&points, &mut order, 0);
        KdTree { points, order }
    }

    /// The allowed candidate nearest to `q` (the lowest number among equals), or None when none is allowed.
    fn nearest(&self, q: [f32; 3], allowed: &dyn Fn(usize) -> bool) -> Option<usize> {
        let mut best: Option<(f32, u32)> = None;
        self.search(&self.order, 0, q, allowed, &mut best);
        best.map(|(_, i)| i as usize)
    }

    fn search(&self, order: &[u32], axis: usize, q: [f32; 3], allowed: &dyn Fn(usize) -> bool, best: &mut Option<(f32, u32)>) {
        if order.is_empty() {
            return;
        }
        let mid = order.len() / 2;
        let here = order[mid];
        let p = self.points[here as usize];
        if allowed(here as usize) {
            let d = (0..3).map(|k| (p[k] - q[k]) * (p[k] - q[k])).sum::<f32>();
            if best.is_none_or(|(bd, bi)| d < bd || (d == bd && here < bi)) {
                *best = Some((d, here));
            }
        }
        let diff = q[axis] - p[axis];
        let (near, far) = if diff < 0.0 { (&order[..mid], &order[mid + 1..]) } else { (&order[mid + 1..], &order[..mid]) };
        self.search(near, (axis + 1) % 3, q, allowed, best);
        if best.is_none_or(|(bd, _)| diff * diff <= bd) {
            self.search(far, (axis + 1) % 3, q, allowed, best);
        }
    }
}

/// Which candidate fills each cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// For each cell (row by row), the number of its candidate.
    pub picks: Vec<usize>,
    /// The distance in tiles within which no candidate appears twice, after shrinking it when there were too few photos for the wish.
    pub gap_used: u32,
}

/// One attempt with a given gap: None if some cell had no candidate left that was far enough from its other uses.
fn try_assign(cells: &[[f32; 3]], cols: u32, tree: &KdTree, gap: u32) -> Option<Vec<usize>> {
    let mut uses: Vec<Vec<(u32, u32)>> = if gap > 0 { vec![Vec::new(); tree.points.len()] } else { Vec::new() };
    let mut picks = Vec::with_capacity(cells.len());
    for (k, lab) in cells.iter().enumerate() {
        let (c, r) = (k as u32 % cols, k as u32 / cols);
        let free = |i: usize| gap == 0 || uses[i].iter().all(|&(uc, ur)| uc.abs_diff(c).max(ur.abs_diff(r)) > gap);
        let i = tree.nearest(*lab, &free)?;
        if gap > 0 {
            uses[i].retain(|&(_, ur)| r - ur <= gap);
            uses[i].push((c, r));
        }
        picks.push(i);
    }
    Some(picks)
}

/// Choose a candidate photo for every cell: the one whose colour is nearest, never one that already appears within `gap` tiles (sideways,
/// down or across; 1 means never touching). When there are too few candidates for that, the gap is made smaller, as little as needed.
/// The same input always gives the same answer.
pub fn assign(cells: &[[u8; 3]], cols: u32, candidates: &[[u8; 3]], gap: u32) -> Assignment {
    if candidates.is_empty() || cells.is_empty() {
        return Assignment { picks: Vec::new(), gap_used: 0 };
    }
    let tree = KdTree::new(candidates.iter().map(|c| lab(*c)).collect());
    let cells: Vec<[f32; 3]> = cells.iter().map(|c| lab(*c)).collect();
    let cols = cols.max(1);
    // A gap of g needs about (g + 1)² different photos; do not try wider than that.
    let roomy = (candidates.len() as f64).sqrt() as u32;
    let mut g = gap.min(roomy.saturating_sub(1).max(1));
    loop {
        if let Some(picks) = try_assign(&cells, cols, &tree, g) {
            return Assignment { picks, gap_used: g };
        }
        g -= 1; // a gap of 0 never fails, so this stops
    }
}

// ---- drawing ----

/// Move the tile's colours towards `towards`: by `blend` (0 to 1) of the way from the tile's mean colour to it, keeping the detail.
/// At 0 the tile is left exactly as it is.
pub fn tint(tile: &mut RgbImage, towards: [u8; 3], blend: f32) {
    let blend = if blend.is_finite() { blend.clamp(0.0, 1.0) } else { 0.0 };
    if blend == 0.0 {
        return;
    }
    let n = (tile.width() as u64 * tile.height() as u64).max(1);
    let mut sum = [0u64; 3];
    for px in tile.pixels() {
        for (s, v) in sum.iter_mut().zip(px.0) {
            *s += v as u64;
        }
    }
    let shift: [f32; 3] = std::array::from_fn(|k| (towards[k] as f32 - sum[k] as f32 / n as f32) * blend);
    for px in tile.pixels_mut() {
        for k in 0..3 {
            px.0[k] = (px.0[k] as f32 + shift[k]).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The grid and what the target looks like in each cell (row by row).
pub struct Layout {
    pub cols: u32,
    pub rows: u32,
    pub cell_colours: Vec<[u8; 3]>,
}

/// A quick picture of the mosaic: every cell drawn from the miniature (`tinies[candidate]`) of its photo, `draw_tile_px` pixels a side.
pub fn draft(layout: &Layout, picks: &[usize], tinies: &[&[u8]], blend: f32, draw_tile_px: u32) -> RgbImage {
    let d = draw_tile_px.max(1);
    let mut page = RgbImage::new(layout.cols * d, layout.rows * d);
    let mut drawn: HashMap<usize, RgbImage> = HashMap::new();
    for (k, pick) in picks.iter().enumerate().take((layout.cols * layout.rows) as usize) {
        let tile = drawn.entry(*pick).or_insert_with(|| {
            let tiny = RgbImage::from_raw(SIG_EDGE, SIG_EDGE, tinies[*pick].to_vec()).unwrap_or_else(|| RgbImage::new(SIG_EDGE, SIG_EDGE));
            imageops::resize(&tiny, d, d, imageops::FilterType::Triangle)
        });
        let mut tile = tile.clone();
        tint(&mut tile, layout.cell_colours[k], blend);
        imageops::replace(&mut page, &tile, ((k as u32 % layout.cols) * d) as i64, ((k as u32 / layout.cols) * d) as i64);
    }
    page
}

/// The side of a tile in the small pictures shown while a mosaic is made (the draft and the picture as it is built).
pub fn preview_tile_px(cols: u32, rows: u32) -> u32 {
    (1200 / cols.max(rows).max(1)).clamp(2, 24)
}

/// What a build asks of the one who calls it, and tells them as it goes.
pub struct Hooks<'a> {
    /// `progress(done, total)` is called as photos are finished and returns false to stop (the build is then `Err("Cancelled.")`).
    pub progress: &'a mut dyn FnMut(u32, u32) -> bool,
    /// When a file cannot be read, another candidate to put in its cells (None: the build fails, naming the file).
    pub spare: &'a mut dyn FnMut(usize) -> Option<usize>,
    /// A small picture of the mosaic so far, grey where no photo has been placed: at the start, every so often, and at the end.
    pub preview: &'a mut dyn FnMut(&RgbImage),
}

/// The finished picture, drawn from the original files. Each photo that is used is read once, however many cells it fills, so only the page
/// and one photo are in memory at a time. See `Hooks` for what it reports and asks.
pub fn build(
    layout: &Layout,
    picks: &[usize],
    paths: &[PathBuf],
    page_w_px: u32,
    page_h_px: u32,
    blend: f32,
    hooks: &mut Hooks,
) -> Result<RgbImage> {
    check_px(page_w_px, page_h_px)?;
    let mut page = RgbImage::new(page_w_px.max(1), page_h_px.max(1));
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (cell, pick) in picks.iter().enumerate() {
        groups.entry(*pick).or_default().push(cell);
    }
    let total = groups.len() as u32;
    let cancelled = || PixieError::Invalid("Cancelled.".into());
    let d = preview_tile_px(layout.cols, layout.rows);
    let mut preview = RgbImage::from_pixel(layout.cols * d, layout.rows * d, image::Rgb([222, 222, 226]));
    (hooks.preview)(&preview);
    let mut last_preview = Instant::now();
    if !(hooks.progress)(0, total) {
        return Err(cancelled());
    }
    let mut work: Vec<(usize, Vec<usize>)> = groups.into_iter().rev().collect();
    let mut done = 0;
    while let Some((pick, cells)) = work.pop() {
        let rects: Vec<_> = cells.iter().map(|k| cell_rect(*k as u32 % layout.cols, *k as u32 / layout.cols, layout.cols, layout.rows, page.width(), page.height())).collect();
        let want = rects.iter().map(|r| r.2.max(r.3)).max().unwrap_or(1);
        let path = &paths[pick];
        // Decoded no smaller than the biggest cell needs from the photo's shorter side.
        let edge = match image::image_dimensions(path) {
            Ok((sw, sh)) if sw > 0 && sh > 0 => ((sw.max(sh) as f64 * want as f64 / sw.min(sh) as f64).ceil() as u32).clamp(1, MAX_SIDE_PX),
            _ => MAX_SIDE_PX,
        };
        let img = match decode_scaled(path, edge) {
            Ok(img) => img,
            Err(e) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match (hooks.spare)(pick) {
                    Some(other) if other != pick => {
                        work.push((other, cells));
                        continue;
                    }
                    _ => return Err(PixieError::Image(format!("{name} could not be read: {e}"))),
                }
            }
        };
        let (w, h) = img.dimensions();
        let side = w.min(h).max(1);
        let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side).to_rgb8();
        let mut sized: HashMap<(u32, u32), RgbImage> = HashMap::new();
        for (k, (x, y, cw, ch)) in cells.iter().zip(rects) {
            let mut tile = sized.entry((cw, ch)).or_insert_with(|| imageops::resize(&square, cw, ch, imageops::FilterType::Lanczos3)).clone();
            tint(&mut tile, layout.cell_colours[*k], blend);
            imageops::replace(&mut page, &tile, x as i64, y as i64);
        }
        let small = imageops::resize(&square, d, d, imageops::FilterType::Triangle);
        for k in &cells {
            let mut t = small.clone();
            tint(&mut t, layout.cell_colours[*k], blend);
            imageops::replace(&mut preview, &t, ((*k as u32 % layout.cols) * d) as i64, ((*k as u32 / layout.cols) * d) as i64);
        }
        if last_preview.elapsed() >= Duration::from_millis(300) {
            (hooks.preview)(&preview);
            last_preview = Instant::now();
        }
        done += 1;
        if !(hooks.progress)(done, total) {
            return Err(cancelled());
        }
    }
    (hooks.preview)(&preview);
    Ok(page)
}

// ---- the built picture ----

/// The side of a tile of the viewing pyramid.
const VIEW_TILE: u32 = 256;

/// The colour behind the picture in the viewer (the window's `#2b2b2f`): what the padding of an edge tile is painted, so it does not show.
const VIEW_BACKGROUND: image::Rgb<u8> = image::Rgb([43, 43, 47]);

/// What was built: its size, the resolution recorded in it, and the number of zoom levels of the viewing pyramid (level 0 is the whole
/// picture in one tile, the last level is full size).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Built {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub levels: u32,
}

fn level_count(w: u32, h: u32) -> u32 {
    let mut levels = 1;
    let mut longest = w.max(h);
    while longest > VIEW_TILE {
        longest = longest.div_ceil(2);
        levels += 1;
    }
    levels
}

/// Keep the built picture in `dir`: `full.png`, and the viewing pyramid as `z/{level}/{x}/{y}.jpg` (edge tiles padded to a whole tile).
/// Written beside `dir` and swapped in at the end, so a failure leaves the picture that was there.
pub fn write_built(img: &RgbImage, dir: &std::path::Path, dpi: u32) -> Result<Built> {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.with_file_name(format!("{name}.new"));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    let made = write_parts(img, &tmp, dpi);
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::rename(&tmp, dir)?;
    made
}

fn write_parts(img: &RgbImage, dir: &std::path::Path, dpi: u32) -> Result<Built> {
    crate::collage::save(img, &dir.join("full.png"), dpi, 92)?;
    let (w, h) = img.dimensions();
    let levels = level_count(w, h);
    let mut smaller: Option<RgbImage> = None;
    for z in (0..levels).rev() {
        let level = smaller.as_ref().unwrap_or(img);
        let (lw, lh) = level.dimensions();
        for ty in 0..lh.div_ceil(VIEW_TILE) {
            for tx in 0..lw.div_ceil(VIEW_TILE) {
                let (x, y) = (tx * VIEW_TILE, ty * VIEW_TILE);
                let mut tile = RgbImage::from_pixel(VIEW_TILE, VIEW_TILE, VIEW_BACKGROUND);
                let part = imageops::crop_imm(level, x, y, VIEW_TILE.min(lw - x), VIEW_TILE.min(lh - y)).to_image();
                imageops::replace(&mut tile, &part, 0, 0);
                let folder = dir.join("z").join(z.to_string()).join(tx.to_string());
                std::fs::create_dir_all(&folder)?;
                std::fs::write(folder.join(format!("{ty}.jpg")), jpeg_bytes(&tile, 85)?)?;
            }
        }
        if z > 0 {
            smaller = Some(imageops::resize(level, lw.div_ceil(2), lh.div_ceil(2), imageops::FilterType::Triangle));
        }
    }
    Ok(Built { width: w, height: h, dpi, levels })
}

/// One tile of the viewing pyramid (a JPEG), if there is one.
pub fn view_tile(dir: &std::path::Path, z: u32, x: u32, y: u32) -> Option<Vec<u8>> {
    std::fs::read(dir.join("z").join(z.to_string()).join(x.to_string()).join(format!("{y}.jpg"))).ok()
}

/// Write the built picture to `dst` as a PNG (copied as it is) or a JPEG (by the extension of `dst`). A file already at `dst` survives a failure.
pub fn save_built(dir: &std::path::Path, dst: &std::path::Path, dpi: u32) -> Result<()> {
    let ext = dst.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let full = dir.join("full.png");
    match ext.as_str() {
        "png" => {
            let tmp = dst.with_extension("png.tmp");
            if let Err(e) = std::fs::copy(&full, &tmp).and_then(|_| std::fs::rename(&tmp, dst)) {
                let _ = std::fs::remove_file(&tmp);
                return Err(e.into());
            }
            Ok(())
        }
        "jpg" | "jpeg" => {
            let img = image::open(&full).map_err(|e| PixieError::Image(e.to_string()))?.to_rgb8();
            crate::collage::save(&img, dst, dpi, 92)
        }
        _ => Err(PixieError::Invalid("Save the mosaic as a .png or a .jpg file.".into())),
    }
}

/// The signature of a grid thumbnail (a small upright JPEG made by `thumbs`).
pub fn signature_of_thumbnail(thumb: &std::path::Path) -> Result<Signature> {
    Ok(signature(&decode_scaled(thumb, crate::thumbs::GRID_EDGE)?))
}

/// A picture as JPEG bytes.
pub fn jpeg_bytes(img: &RgbImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100)).encode_image(img).map_err(|e| PixieError::Image(e.to_string()))?;
    Ok(out)
}

/// The part of the target photo to rebuild, as a picture of `out_w` by `out_h` pixels (the shape of the page): the same crop and zoom as a
/// Collage frame.
pub fn crop_target(img: &DynamicImage, out_w: u32, out_h: u32, focus_x: f32, focus_y: f32, zoom: f32) -> RgbImage {
    let (sw, sh) = img.dimensions();
    let (x, y, w, h) = cover_crop(sw, sh, out_w, out_h, focus_x, focus_y, zoom);
    imageops::resize(&img.crop_imm(x, y, w, h).to_rgb8(), out_w.max(1), out_h.max(1), imageops::FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn grid_size_fits_whole_tiles() {
        assert_eq!(grid_size(1000, 500, 100), (10, 5));
        assert_eq!(grid_size(1050, 520, 100), (10, 5));
        assert_eq!(grid_size(50, 50, 100), (1, 1));
    }

    #[test]
    fn tile_px_follows_mm_and_dpi() {
        assert_eq!(tile_px(25.4, 300), 300);
        assert_eq!(tile_px(0.0, 300), 1);
    }

    #[test]
    fn cell_rects_cover_the_page_exactly() {
        let (w, h, cols, rows) = (1050u32, 520u32, 10u32, 5u32);
        let mut area = 0u64;
        for r in 0..rows {
            for c in 0..cols {
                let (x, y, cw, ch) = cell_rect(c, r, cols, rows, w, h);
                assert!(cw >= 1 && ch >= 1 && x + cw <= w && y + ch <= h);
                area += cw as u64 * ch as u64;
            }
        }
        assert_eq!(area, w as u64 * h as u64);
    }

    #[test]
    fn signature_of_a_flat_picture_is_that_colour() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([255, 0, 0])));
        let s = signature(&img);
        assert_eq!(s.avg, [255, 0, 0]);
        assert_eq!(s.tiny.len(), 432);
    }

    #[test]
    fn signature_takes_the_centre_square() {
        let mut img = RgbImage::new(30, 10);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = Rgb(if x < 10 { [0, 0, 255] } else if x < 20 { [0, 255, 0] } else { [255, 0, 0] });
        }
        assert_eq!(signature(&DynamicImage::ImageRgb8(img)).avg, [0, 255, 0]);
    }

    #[test]
    fn cell_colours_average_each_cell() {
        let mut img = RgbImage::new(4, 2);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = Rgb(if x < 2 { [0, 0, 0] } else { [255, 255, 255] });
        }
        assert_eq!(cell_colours(&img, 2, 1), vec![[0, 0, 0], [255, 255, 255]]);
    }

    /// No two cells showing the same candidate lie within `gap` tiles of each other (sideways, down or across).
    fn spread(a: &Assignment, cols: u32, gap: u32) -> bool {
        let at = |k: usize| ((k as u32 % cols) as i64, (k as u32 / cols) as i64);
        for i in 0..a.picks.len() {
            for j in i + 1..a.picks.len() {
                let ((x1, y1), (x2, y2)) = (at(i), at(j));
                if a.picks[i] == a.picks[j] && (x1 - x2).abs().max((y1 - y2).abs()) <= gap as i64 {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn each_cell_gets_the_nearest_colour() {
        let a = assign(&[[250, 0, 0], [0, 250, 0]], 2, &[[255, 0, 0], [0, 255, 0], [0, 0, 255]], 1);
        assert_eq!(a.picks, vec![0, 1]);
    }

    #[test]
    fn the_same_photo_never_touches_itself() {
        let cands: Vec<[u8; 3]> = (0..6).map(|i| [100 + i, 100, 100]).collect();
        let a = assign(&vec![[100, 100, 100]; 36], 6, &cands, 1);
        assert_eq!(a.gap_used, 1);
        assert!(spread(&a, 6, 1));
    }

    #[test]
    fn a_wider_gap_is_kept_when_there_are_enough_photos() {
        let cands: Vec<[u8; 3]> = (0..100).map(|i| [100 + (i % 5) as u8, 100 + (i / 5 % 5) as u8, 100]).collect();
        let a = assign(&vec![[100, 100, 100]; 100], 10, &cands, 3);
        assert_eq!(a.gap_used, 3);
        assert!(spread(&a, 10, 3));
    }

    #[test]
    fn the_gap_shrinks_when_photos_run_out() {
        let cands: Vec<[u8; 3]> = (0..6).map(|i| [100 + i, 100, 100]).collect();
        let a = assign(&vec![[100, 100, 100]; 100], 10, &cands, 5);
        assert!(a.gap_used >= 1 && a.gap_used < 5, "gap used {}", a.gap_used);
        assert!(spread(&a, 10, a.gap_used));
    }

    #[test]
    fn one_candidate_fills_everything() {
        let a = assign(&vec![[10, 20, 30]; 9], 3, &[[0, 0, 0]], 4);
        assert_eq!(a.gap_used, 0);
        assert_eq!(a.picks, vec![0; 9]);
    }

    #[test]
    fn no_candidates_gives_no_picks() {
        assert!(assign(&[[1, 2, 3]], 1, &[], 3).picks.is_empty());
    }

    #[test]
    fn same_input_same_answer() {
        let cands: Vec<[u8; 3]> = (0..30).map(|i| [(i * 8) as u8, (i * 5) as u8, (i * 3) as u8]).collect();
        let cells: Vec<[u8; 3]> = (0..40).map(|i| [(i * 6) as u8, (i * 4) as u8, 90]).collect();
        assert_eq!(assign(&cells, 8, &cands, 2), assign(&cells, 8, &cands, 2));
    }

    #[test]
    fn many_candidates_are_quick() {
        let mut seed = 12345u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as u8
        };
        let cands: Vec<[u8; 3]> = (0..50_000).map(|_| [next(), next(), next()]).collect();
        let cells: Vec<[u8; 3]> = (0..2_500).map(|_| [next(), next(), next()]).collect();
        let a = assign(&cells, 50, &cands, 3);
        assert_eq!(a.picks.len(), 2_500);
        assert_eq!(a.gap_used, 3);
    }

    fn mean(img: &RgbImage) -> f32 {
        img.as_raw().iter().map(|v| *v as f32).sum::<f32>() / img.as_raw().len() as f32
    }

    #[test]
    fn tint_zero_changes_nothing() {
        let mut tile = RgbImage::from_fn(4, 4, |x, y| Rgb([(x * 40) as u8, (y * 40) as u8, 7]));
        let before = tile.clone();
        tint(&mut tile, [200, 10, 10], 0.0);
        assert_eq!(tile, before);
    }

    #[test]
    fn tint_full_moves_the_mean_to_the_target_and_keeps_detail() {
        let mut tile = RgbImage::from_fn(4, 2, |x, _| Rgb(if x < 2 { [100; 3] } else { [140; 3] }));
        tint(&mut tile, [60, 60, 60], 1.0);
        assert!((mean(&tile) - 60.0).abs() < 2.0);
        assert_ne!(tile.get_pixel(0, 0), tile.get_pixel(3, 0));
    }

    fn layout(cols: u32, rows: u32, colours: Vec<[u8; 3]>) -> Layout {
        Layout { cols, rows, cell_colours: colours }
    }

    #[test]
    fn draft_has_the_size_of_the_grid() {
        let l = layout(5, 4, vec![[0; 3]; 20]);
        let tiny = vec![0u8; 432];
        assert_eq!(draft(&l, &[0; 20], &[&tiny], 0.0, 8).dimensions(), (40, 32));
    }

    #[test]
    fn draft_cell_comes_from_its_pick() {
        let l = layout(2, 1, vec![[255, 0, 0], [0, 0, 255]]);
        let (red, blue) = ([255u8, 0, 0].repeat(144), [0u8, 0, 255].repeat(144));
        let d = draft(&l, &[0, 1], &[&red, &blue], 0.0, 6);
        assert_eq!(d.get_pixel(2, 3).0, [255, 0, 0]);
        assert_eq!(d.get_pixel(9, 3).0, [0, 0, 255]);
    }

    fn solid(dir: &std::path::Path, name: &str, rgb: [u8; 3]) -> std::path::PathBuf {
        let p = dir.join(name);
        RgbImage::from_pixel(40, 40, Rgb(rgb)).save(&p).unwrap();
        p
    }

    fn go(l: &Layout, picks: &[usize], paths: &[std::path::PathBuf], w: u32, h: u32) -> pixie_core::Result<RgbImage> {
        build(l, picks, paths, w, h, 0.0, &mut Hooks { progress: &mut |_, _| true, spare: &mut |_| None, preview: &mut |_| {} })
    }

    #[test]
    fn build_places_each_photo_in_its_cells() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![solid(dir.path(), "r.png", [255, 0, 0]), solid(dir.path(), "b.png", [0, 0, 255])];
        let l = layout(2, 1, vec![[255, 0, 0], [0, 0, 255]]);
        let page = go(&l, &[0, 1], &paths, 200, 100).unwrap();
        assert_eq!(page.dimensions(), (200, 100));
        assert_eq!(page.get_pixel(10, 50).0, [255, 0, 0]);
        assert_eq!(page.get_pixel(190, 50).0, [0, 0, 255]);
    }

    #[test]
    fn build_decodes_a_repeated_photo_once() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![solid(dir.path(), "r.png", [255, 0, 0])];
        let l = layout(2, 2, vec![[255, 0, 0]; 4]);
        let mut totals = Vec::new();
        let page = build(&l, &[0; 4], &paths, 100, 100, 0.0, &mut Hooks { progress: &mut |_, total| { totals.push(total); true }, spare: &mut |_| None, preview: &mut |_| {} }).unwrap();
        assert!(totals.iter().all(|t| *t == 1), "one photo to read: {totals:?}");
        assert_eq!(page.get_pixel(5, 5), page.get_pixel(95, 95));
    }

    #[test]
    fn build_shows_the_photos_as_they_are_placed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![solid(dir.path(), "r.png", [255, 0, 0]), solid(dir.path(), "b.png", [0, 0, 255])];
        let l = layout(2, 1, vec![[255, 0, 0], [0, 0, 255]]);
        let mut shots: Vec<RgbImage> = Vec::new();
        build(&l, &[0, 1], &paths, 200, 100, 0.0, &mut Hooks { progress: &mut |_, _| true, spare: &mut |_| None, preview: &mut |p| shots.push(p.clone()) }).unwrap();
        let d = preview_tile_px(2, 1);
        assert!(shots.len() >= 2, "once at the start and once at the end at least");
        assert_eq!(shots[0].dimensions(), (2 * d, d));
        assert_eq!(shots[0].get_pixel(1, 1).0, [222, 222, 226], "nothing placed yet");
        let last = shots.last().unwrap();
        assert_eq!(last.get_pixel(1, 1).0, [255, 0, 0]);
        assert_eq!(last.get_pixel(d + 1, 1).0, [0, 0, 255]);
    }

    #[test]
    fn build_can_be_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![solid(dir.path(), "r.png", [255, 0, 0])];
        let l = layout(1, 1, vec![[255, 0, 0]]);
        let e = build(&l, &[0], &paths, 50, 50, 0.0, &mut Hooks { progress: &mut |_, _| false, spare: &mut |_| None, preview: &mut |_| {} }).unwrap_err();
        assert!(e.to_string().contains("Cancelled."), "{e}");
    }

    #[test]
    fn build_uses_a_spare_for_an_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![dir.path().join("gone.png"), solid(dir.path(), "b.png", [0, 0, 255])];
        let l = layout(1, 1, vec![[0, 0, 255]]);
        let page = build(&l, &[0], &paths, 50, 50, 0.0, &mut Hooks { progress: &mut |_, _| true, spare: &mut |bad| (bad == 0).then_some(1), preview: &mut |_| {} }).unwrap();
        assert_eq!(page.get_pixel(25, 25).0, [0, 0, 255]);
    }

    #[test]
    fn build_fails_with_words_when_there_is_no_spare() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![dir.path().join("gone.png")];
        let l = layout(1, 1, vec![[0; 3]]);
        let e = go(&l, &[0], &paths, 50, 50).unwrap_err();
        assert!(e.to_string().contains("gone.png"), "{e}");
    }

    #[test]
    fn a_page_at_the_limit_is_refused_before_any_work() {
        let l = layout(1, 1, vec![[0; 3]]);
        let e = go(&l, &[0], &[], 16_385, 100).unwrap_err();
        assert!(e.to_string().contains("longest side can be at most 16384"), "{e}");
    }

    // ---- the built picture ----

    fn gradient(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| Rgb([(x % 251) as u8, (y % 251) as u8, 90]))
    }

    #[test]
    fn write_built_makes_the_pyramid() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("built");
        let b = write_built(&gradient(600, 300), &dir, 150).unwrap();
        assert_eq!((b.width, b.height, b.dpi, b.levels), (600, 300, 150, 3));
        assert_eq!(image::open(dir.join("full.png")).unwrap().dimensions(), (600, 300));
        assert!(view_tile(&dir, 2, 2, 1).is_some());
        assert!(view_tile(&dir, 2, 3, 0).is_none());
        assert!(view_tile(&dir, 0, 0, 0).is_some());
        assert!(view_tile(&dir, 0, 1, 0).is_none());
        let tile = image::load_from_memory(&view_tile(&dir, 0, 0, 0).unwrap()).unwrap();
        assert_eq!(tile.dimensions(), (VIEW_TILE, VIEW_TILE), "edge tiles are padded to a whole tile");
        let corner = tile.to_rgb8().get_pixel(VIEW_TILE - 2, VIEW_TILE - 2).0;
        assert!(corner.iter().all(|c| (*c as i32 - 45).abs() < 8), "the padding is the viewer's background, not a grey that shows: {corner:?}");
    }

    #[test]
    fn an_older_built_picture_survives_a_failed_write() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("built");
        write_built(&gradient(300, 200), &dir, 150).unwrap();
        let before = std::fs::read(dir.join("full.png")).unwrap();
        std::fs::write(root.path().join("built.new"), b"in the way").unwrap();
        assert!(write_built(&gradient(400, 100), &dir, 150).is_err());
        assert_eq!(std::fs::read(dir.join("full.png")).unwrap(), before);
    }

    #[test]
    fn save_built_copies_a_png() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("built");
        write_built(&gradient(300, 200), &dir, 150).unwrap();
        let dst = root.path().join("out.png");
        save_built(&dir, &dst, 150).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), std::fs::read(dir.join("full.png")).unwrap());
    }

    #[test]
    fn save_built_makes_a_jpeg() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("built");
        write_built(&gradient(300, 200), &dir, 150).unwrap();
        let dst = root.path().join("out.jpg");
        save_built(&dir, &dst, 150).unwrap();
        assert_eq!(image::open(&dst).unwrap().dimensions(), (300, 200));
    }

    #[test]
    fn save_built_refuses_other_extensions_with_words() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("built");
        write_built(&gradient(100, 100), &dir, 150).unwrap();
        let e = save_built(&dir, &root.path().join("out.bmp"), 150).unwrap_err();
        assert!(e.to_string().contains("Save the mosaic as a .png or a .jpg file."), "{e}");
    }

    #[test]
    fn a_thumbnail_file_gives_its_signature() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.jpg");
        RgbImage::from_pixel(64, 48, Rgb([0, 200, 0])).save(&p).unwrap();
        let s = signature_of_thumbnail(&p).unwrap();
        assert!(s.avg[1] > 180 && s.avg[0] < 30, "{:?}", s.avg);
    }

    #[test]
    fn jpeg_bytes_decode_back_to_the_same_size() {
        let bytes = jpeg_bytes(&RgbImage::from_pixel(30, 20, Rgb([9, 9, 9])), 85).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().dimensions(), (30, 20));
    }

    #[test]
    fn crop_target_has_the_size_asked_and_zoom_picks_the_part() {
        let mut img = RgbImage::new(100, 100);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = Rgb([(x * 2) as u8, 0, 0]);
        }
        let img = DynamicImage::ImageRgb8(img);
        let whole = crop_target(&img, 50, 50, 0.0, 0.0, 1.0);
        let zoomed = crop_target(&img, 50, 50, 0.0, 0.0, 4.0);
        assert_eq!(zoomed.dimensions(), (50, 50));
        assert!(whole.get_pixel(49, 0).0[0] > 150);
        assert!(zoomed.get_pixel(49, 0).0[0] < 60);
    }
}
