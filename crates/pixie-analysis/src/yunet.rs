use image::DynamicImage;

#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    pub landmarks: [(f32, f32); 5],
}

pub struct Raw<'a> {
    pub cls: [&'a [f32]; 3],
    pub obj: [&'a [f32]; 3],
    pub bbox: [&'a [f32]; 3],
    pub kps: [&'a [f32]; 3],
}

pub struct Face {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    pub landmarks: [(f32, f32); 5],
}

pub const INPUT: usize = 640;
const STRIDES: [usize; 3] = [8, 16, 32];
/// Faces smaller than this (longest side, in pixels of the 640 input) are too small to recognise.
pub const MIN_FACE_PX: f32 = 26.0;
/// Detections scoring below this are not stored.
pub const MIN_SCORE: f32 = 0.85;

/// Turn the 12 raw network outputs into boxes (pixels of the 640 input), one per cell scoring at least `threshold`.
pub fn decode(raw: &Raw, threshold: f32) -> Vec<Detection> {
    let mut out = Vec::new();
    for (k, &stride) in STRIDES.iter().enumerate() {
        let cols = INPUT / stride;
        let s = stride as f32;
        for i in 0..cols * cols {
            let score = (raw.cls[k][i].clamp(0.0, 1.0) * raw.obj[k][i].clamp(0.0, 1.0)).sqrt();
            if score < threshold {
                continue;
            }
            let (r, c) = ((i / cols) as f32, (i % cols) as f32);
            let b = &raw.bbox[k][i * 4..i * 4 + 4];
            let (cx, cy) = ((c + b[0]) * s, (r + b[1]) * s);
            let (w, h) = (b[2].exp() * s, b[3].exp() * s);
            let kp = &raw.kps[k][i * 10..i * 10 + 10];
            let mut landmarks = [(0.0, 0.0); 5];
            for (n, l) in landmarks.iter_mut().enumerate() {
                *l = ((kp[2 * n] + c) * s, (kp[2 * n + 1] + r) * s);
            }
            out.push(Detection { x: cx - w / 2.0, y: cy - h / 2.0, w, h, score, landmarks });
        }
    }
    out
}

fn iou(a: &Detection, b: &Detection) -> f32 {
    let (x1, y1) = (a.x.max(b.x), a.y.max(b.y));
    let (x2, y2) = ((a.x + a.w).min(b.x + b.w), (a.y + a.h).min(b.y + b.h));
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a.w * a.h + b.w * b.h - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Non-maximum suppression: best first, dropping boxes that overlap a better one by `iou_max` or more.
pub fn nms(mut d: Vec<Detection>, iou_max: f32) -> Vec<Detection> {
    d.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Detection> = Vec::new();
    for c in d {
        if kept.iter().all(|k| iou(k, &c) < iou_max) {
            kept.push(c);
        }
    }
    kept
}

/// The image scaled so its long edge is 640, at the top-left of a zero-padded 640x640 BGR planar tensor (0..255).
/// Also returns the scaled width and height in pixels.
pub fn letterbox(img: &DynamicImage) -> (Vec<f32>, f32, f32) {
    let rgb = img.to_rgb8();
    let scale = INPUT as f32 / rgb.width().max(rgb.height()) as f32;
    let rw = ((rgb.width() as f32 * scale).round() as u32).clamp(1, INPUT as u32);
    let rh = ((rgb.height() as f32 * scale).round() as u32).clamp(1, INPUT as u32);
    let small = image::imageops::resize(&rgb, rw, rh, image::imageops::FilterType::Triangle);
    let plane = INPUT * INPUT;
    let mut t = vec![0.0f32; 3 * plane];
    for (x, y, p) in small.enumerate_pixels() {
        let o = y as usize * INPUT + x as usize;
        t[o] = p[2] as f32;
        t[plane + o] = p[1] as f32;
        t[2 * plane + o] = p[0] as f32;
    }
    (t, rw as f32, rh as f32)
}

/// Detections as fractions of the upright image (`rw`, `rh` = the scaled size), best first; small and weak ones dropped.
pub fn to_faces(d: Vec<Detection>, rw: f32, rh: f32) -> Vec<Face> {
    let f = |v: f32, d: f32| (v / d).clamp(0.0, 1.0);
    let mut faces: Vec<Face> = d
        .into_iter()
        .filter(|d| d.w.max(d.h) >= MIN_FACE_PX && d.score >= MIN_SCORE)
        .map(|d| {
            let mut landmarks = d.landmarks;
            for l in landmarks.iter_mut() {
                *l = (f(l.0, rw), f(l.1, rh));
            }
            let (x, y) = (f(d.x, rw), f(d.y, rh));
            Face { x, y, w: f(d.x + d.w, rw) - x, h: f(d.y + d.h, rh) - y, score: d.score, landmarks }
        })
        .collect();
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    faces
}

#[cfg(test)]
mod tests {
    use super::*;

    type Planes = (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>);
    fn blank(n: usize) -> Planes {
        (vec![0.0; n], vec![0.0; n], vec![0.0; n * 4], vec![0.0; n * 10])
    }

    fn run(p8: &Planes, threshold: f32) -> Vec<Detection> {
        let (c16, o16, b16, k16) = blank(1600);
        let (c32, o32, b32, k32) = blank(400);
        let raw = Raw { cls: [&p8.0, &c16, &c32], obj: [&p8.1, &o16, &o32], bbox: [&p8.2, &b16, &b32], kps: [&p8.3, &k16, &k32] };
        decode(&raw, threshold)
    }

    #[test]
    fn decode_reads_a_stride_8_cell() {
        let mut p = blank(6400);
        let i = 10 * 80 + 20; // row 10, column 20 at stride 8 (80 columns)
        p.0[i] = 1.0;
        p.1[i] = 1.0;
        p.2[i * 4..i * 4 + 4].copy_from_slice(&[0.5, 0.5, 4f32.ln(), 4f32.ln()]);
        let d = run(&p, 0.5);
        assert_eq!(d.len(), 1);
        assert!((d[0].x - 148.0).abs() < 1e-3 && (d[0].y - 68.0).abs() < 1e-3); // centre (164,84), 32x32
        assert!((d[0].w - 32.0).abs() < 1e-3 && (d[0].h - 32.0).abs() < 1e-3);
        assert!((d[0].landmarks[0].0 - 160.0).abs() < 1e-3 && (d[0].landmarks[0].1 - 80.0).abs() < 1e-3);
        assert!((d[0].score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn decode_ignores_weak_cells() {
        let mut p = blank(6400);
        p.0.iter_mut().for_each(|v| *v = 0.3);
        p.1.iter_mut().for_each(|v| *v = 0.3);
        assert!(run(&p, 0.5).is_empty());
    }

    #[test]
    fn nms_keeps_the_best_of_overlapping_boxes_and_both_of_separate_ones() {
        let d = |x: f32, s: f32| Detection { x, y: 0.0, w: 40.0, h: 40.0, score: s, landmarks: [(0.0, 0.0); 5] };
        let kept = nms(vec![d(0.0, 0.9), d(4.0, 0.95), d(200.0, 0.8)], 0.3);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].score, 0.95);
    }

    #[test]
    fn letterbox_scales_the_long_edge_to_640_top_left_and_pads_with_zero() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1280, 640, image::Rgb([10, 20, 30])));
        let (t, rw, rh) = letterbox(&img);
        assert_eq!((rw, rh), (640.0, 320.0));
        assert_eq!(t.len(), 3 * 640 * 640);
        assert_eq!(t[0], 30.0); // BGR: plane 0 is blue
        assert_eq!(t[640 * 640], 20.0); // plane 1 green
        assert_eq!(t[2 * 640 * 640], 10.0); // plane 2 red
        assert_eq!(t[500 * 640], 0.0); // below the image: padding
    }

    #[test]
    fn to_faces_gives_fractions_and_drops_small_or_weak_detections() {
        let big = Detection { x: 64.0, y: 32.0, w: 128.0, h: 96.0, score: 0.95, landmarks: [(128.0, 64.0); 5] };
        let tiny = Detection { w: 10.0, h: 10.0, ..big.clone() };
        let weak = Detection { score: 0.5, ..big.clone() };
        let f = to_faces(vec![big, tiny, weak], 640.0, 320.0);
        assert_eq!(f.len(), 1);
        assert!((f[0].x - 0.1).abs() < 1e-6 && (f[0].y - 0.1).abs() < 1e-6);
        assert!((f[0].w - 0.2).abs() < 1e-6 && (f[0].h - 0.3).abs() < 1e-6);
        assert!((f[0].landmarks[0].0 - 0.2).abs() < 1e-6);
    }
}
