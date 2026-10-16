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

/// What to put in a synthetic MP4. Boxes are valid ISO base media, not playable video.
#[derive(Clone, Default)]
pub struct Mp4Spec {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    /// Seconds since 1904-01-01 (MP4 epoch); 0 = unset.
    pub created_1904: u64,
    /// 0, 90, 180 or 270 degrees clockwise.
    pub rotation: u16,
    /// ISO 6709 string such as "+20.6858-156.4440/".
    pub iso6709: Option<String>,
    /// Put `moov` after a dummy `mdat` (a non-"faststart" file).
    pub moov_last: bool,
    /// Write a version 1 `mvhd` (64-bit times), as some cameras do.
    pub mvhd_v1: bool,
    /// Write a timescale of 0 (broken files).
    pub zero_timescale: bool,
}

fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(payload);
    v
}

fn fixed(v: i32) -> [u8; 4] {
    v.to_be_bytes()
}

pub fn mp4(s: &Mp4Spec) -> Vec<u8> {
    let ftyp = bx(b"ftyp", &[b"isom".as_slice(), &[0, 0, 2, 0], b"isomiso2mp41"].concat());
    // mvhd v0, or v1 with 64-bit times
    let timescale: u32 = if s.zero_timescale { 0 } else { 1000 };
    let mut mvhd = if s.mvhd_v1 { vec![1u8, 0, 0, 0] } else { vec![0u8; 4] };
    if s.mvhd_v1 {
        mvhd.extend_from_slice(&s.created_1904.to_be_bytes());
        mvhd.extend_from_slice(&s.created_1904.to_be_bytes());
        mvhd.extend_from_slice(&timescale.to_be_bytes());
        mvhd.extend_from_slice(&(s.duration_ms as u64).to_be_bytes());
    } else {
        mvhd.extend_from_slice(&(s.created_1904 as u32).to_be_bytes());
        mvhd.extend_from_slice(&(s.created_1904 as u32).to_be_bytes());
        mvhd.extend_from_slice(&timescale.to_be_bytes());
        mvhd.extend_from_slice(&s.duration_ms.to_be_bytes());
    }
    mvhd.extend_from_slice(&[0, 1, 0, 0, 1, 0, 0, 0]); // rate 1.0, volume 1.0, reserved
    mvhd.extend_from_slice(&[0u8; 8]);
    for m in [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000] {
        mvhd.extend_from_slice(&fixed(m));
    }
    mvhd.extend_from_slice(&[0u8; 24]); // pre_defined
    mvhd.extend_from_slice(&2u32.to_be_bytes()); // next track id
    let mvhd = bx(b"mvhd", &mvhd);
    // tkhd v0 with a rotation matrix
    let (a, b, c, d) = match s.rotation % 360 {
        90 => (0, 0x10000, -0x10000, 0),
        180 => (-0x10000, 0, 0, -0x10000),
        270 => (0, -0x10000, 0x10000, 0),
        _ => (0x10000, 0, 0, 0x10000),
    };
    let mut tkhd = vec![0, 0, 0, 3];
    tkhd.extend_from_slice(&(s.created_1904 as u32).to_be_bytes());
    tkhd.extend_from_slice(&(s.created_1904 as u32).to_be_bytes());
    tkhd.extend_from_slice(&1u32.to_be_bytes()); // track id
    tkhd.extend_from_slice(&[0u8; 4]);
    tkhd.extend_from_slice(&s.duration_ms.to_be_bytes());
    tkhd.extend_from_slice(&[0u8; 8]);
    tkhd.extend_from_slice(&[0u8; 8]); // layer, alt group, volume, reserved
    for m in [a, b, 0, c, d, 0, 0, 0, 0x4000_0000] {
        tkhd.extend_from_slice(&fixed(m));
    }
    tkhd.extend_from_slice(&((s.width) << 16).to_be_bytes());
    tkhd.extend_from_slice(&((s.height) << 16).to_be_bytes());
    let trak = bx(b"trak", &bx(b"tkhd", &tkhd));
    let mut moov_payload = [mvhd, trak].concat();
    if let Some(loc) = &s.iso6709 {
        let mut xyz = (loc.len() as u16).to_be_bytes().to_vec();
        xyz.extend_from_slice(&[0x15, 0xc7]); // language
        xyz.extend_from_slice(loc.as_bytes());
        moov_payload.extend(bx(b"udta", &bx(&[0xa9, b'x', b'y', b'z'], &xyz)));
    }
    let moov = bx(b"moov", &moov_payload);
    let mdat = bx(b"mdat", &[0u8; 64]);
    if s.moov_last {
        [ftyp, mdat, moov].concat()
    } else {
        [ftyp, moov, mdat].concat()
    }
}

/// Seconds between 1904-01-01 and 1970-01-01.
pub const MP4_EPOCH_OFFSET: u64 = 2_082_844_800;
