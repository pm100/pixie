use image::{DynamicImage, RgbImage};
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use pixie_core::{PixieError, Result};
use std::path::Path;

fn err<E: std::fmt::Display>(e: E) -> PixieError {
    PixieError::Image(e.to_string())
}

fn open(path: &Path) -> Result<HeifContext<'static>> {
    let p = path.to_str().ok_or_else(|| PixieError::Io("non-UTF-8 path".into()))?;
    HeifContext::read_from_file(p).map_err(err)
}

/// Decode the primary image. libheif applies rotation/mirroring transforms itself.
pub fn decode(path: &Path) -> Result<DynamicImage> {
    decode_context(&open(path)?)
}

/// The same, for a file whose bytes are already in memory.
pub fn decode_bytes(bytes: &[u8]) -> Result<DynamicImage> {
    decode_context(&HeifContext::read_from_bytes(bytes).map_err(err)?)
}

fn decode_context(ctx: &HeifContext) -> Result<DynamicImage> {
    let handle = ctx.primary_image_handle().map_err(err)?;
    let img = LibHeif::new()
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(err)?;
    let planes = img.planes();
    let plane = planes
        .interleaved
        .ok_or_else(|| PixieError::Image("no interleaved plane".into()))?;
    let (w, h, stride) = (plane.width as usize, plane.height as usize, plane.stride);
    let mut buf = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        buf.extend_from_slice(&plane.data[y * stride..y * stride + w * 3]);
    }
    RgbImage::from_raw(w as u32, h as u32, buf)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PixieError::Image("bad pixel buffer".into()))
}

pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let ctx = open(path).ok()?;
    let h = ctx.primary_image_handle().ok()?;
    Some((h.width(), h.height()))
}
