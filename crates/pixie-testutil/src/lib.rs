use image::{codecs::jpeg::JpegEncoder, ImageBuffer, Rgb};

/// Left half red, right half blue.
pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = ImageBuffer::from_fn(w, h, |x, _| {
        if x < w / 2 { Rgb([255u8, 0, 0]) } else { Rgb([0u8, 0, 255]) }
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .expect("encode jpeg");
    out
}

fn le16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
fn le32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

fn exif_tiff(orientation: u16, datetime: Option<&str>) -> Vec<u8> {
    let mut t = vec![b'I', b'I', 0x2A, 0, 8, 0, 0, 0]; // header, IFD0 at 8
    le16(&mut t, if datetime.is_some() { 2 } else { 1 });
    // Orientation: tag 0x0112, SHORT, count 1
    le16(&mut t, 0x0112); le16(&mut t, 3); le32(&mut t, 1); le16(&mut t, orientation); le16(&mut t, 0);
    if datetime.is_some() {
        // Exif IFD pointer: 8 header + 2 count + 2*12 entries + 4 next = 38
        le16(&mut t, 0x8769); le16(&mut t, 4); le32(&mut t, 1); le32(&mut t, 38);
    }
    le32(&mut t, 0); // next IFD
    if let Some(dt) = datetime {
        assert_eq!(dt.len(), 19, "datetime must be 'YYYY:MM:DD HH:MM:SS'");
        le16(&mut t, 1);
        let data_off = t.len() as u32 + 12 + 4;
        le16(&mut t, 0x9003); le16(&mut t, 2); le32(&mut t, 20); le32(&mut t, data_off);
        le32(&mut t, 0);
        t.extend_from_slice(dt.as_bytes());
        t.push(0);
    }
    t
}

/// A JPEG with an EXIF APP1 segment inserted right after SOI.
pub fn jpeg_with_exif(w: u32, h: u32, orientation: u16, datetime: Option<&str>) -> Vec<u8> {
    let base = jpeg(w, h);
    let tiff = exif_tiff(orientation, datetime);
    let mut seg = vec![0xFF, 0xE1];
    let len = (2 + 6 + tiff.len()) as u16;
    seg.extend_from_slice(&len.to_be_bytes());
    seg.extend_from_slice(b"Exif\0\0");
    seg.extend_from_slice(&tiff);
    let mut out = base[..2].to_vec(); // SOI
    out.extend_from_slice(&seg);
    out.extend_from_slice(&base[2..]);
    out
}
