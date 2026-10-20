use image::{Rgb, RgbImage};

/// The standard 112x112 ArcFace landmark template, in YuNet's order: eye on the image's left, eye on the right,
/// nose, mouth left, mouth right.
pub const TEMPLATE: [(f32, f32); 5] = [(38.2946, 51.6963), (73.5318, 51.5014), (56.0252, 71.7366), (41.5493, 92.3655), (70.7299, 92.2041)];
pub const SIZE: u32 = 112;

/// `x' = a·x − b·y + tx`, `y' = b·x + a·y + ty`: rotation, uniform scale and shift.
#[derive(Debug, Clone, Copy)]
pub struct Similarity {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Similarity {
    pub fn apply(&self, p: (f32, f32)) -> (f32, f32) {
        (self.a * p.0 - self.b * p.1 + self.tx, self.b * p.0 + self.a * p.1 + self.ty)
    }

    pub fn inverse(&self) -> Similarity {
        let det = self.a * self.a + self.b * self.b;
        let (ia, ib) = (self.a / det, -self.b / det);
        Similarity { a: ia, b: ib, tx: -(ia * self.tx - ib * self.ty), ty: -(ib * self.tx + ia * self.ty) }
    }
}

/// The similarity that best maps `src` onto `dst` (least squares).
pub fn fit(src: &[(f32, f32); 5], dst: &[(f32, f32); 5]) -> Similarity {
    let mean = |p: &[(f32, f32); 5]| (p.iter().map(|q| q.0).sum::<f32>() / 5.0, p.iter().map(|q| q.1).sum::<f32>() / 5.0);
    let (m, n) = (mean(src), mean(dst));
    let (mut na, mut nb, mut den) = (0.0, 0.0, 0.0);
    for (s, d) in src.iter().zip(dst.iter()) {
        let (x, y, u, v) = (s.0 - m.0, s.1 - m.1, d.0 - n.0, d.1 - n.1);
        na += x * u + y * v;
        nb += x * v - y * u;
        den += x * x + y * y;
    }
    let (a, b) = if den > 0.0 { (na / den, nb / den) } else { (1.0, 0.0) };
    Similarity { a, b, tx: n.0 - (a * m.0 - b * m.1), ty: n.1 - (b * m.0 + a * m.1) }
}

fn sample(img: &RgbImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |xi: i64, yi: i64| -> [f32; 3] {
        if xi < 0 || yi < 0 || xi >= w || yi >= h {
            [0.0; 3]
        } else {
            let p = img.get_pixel(xi as u32, yi as u32).0;
            [p[0] as f32, p[1] as f32, p[2] as f32]
        }
    };
    let (xi, yi) = (x0 as i64, y0 as i64);
    let (p00, p10, p01, p11) = (at(xi, yi), at(xi + 1, yi), at(xi, yi + 1), at(xi + 1, yi + 1));
    let mut out = [0.0; 3];
    for c in 0..3 {
        out[c] = p00[c] * (1.0 - fx) * (1.0 - fy) + p10[c] * fx * (1.0 - fy) + p01[c] * (1.0 - fx) * fy + p11[c] * fx * fy;
    }
    out
}

/// The face cut out and straightened to 112x112 so that its eyes, nose and mouth sit where SFace expects them.
pub fn align(img: &RgbImage, landmarks_px: &[(f32, f32); 5]) -> RgbImage {
    let to_src = fit(&TEMPLATE, landmarks_px);
    RgbImage::from_fn(SIZE, SIZE, |u, v| {
        let (x, y) = to_src.apply((u as f32, v as f32));
        let c = sample(img, x, y);
        Rgb([c[0].round() as u8, c[1].round() as u8, c[2].round() as u8])
    })
}

/// SFace's input: planar RGB, raw 0..255.
pub fn sface_tensor(aligned: &RgbImage) -> Vec<f32> {
    let plane = (SIZE * SIZE) as usize;
    let mut t = vec![0.0f32; 3 * plane];
    for (x, y, p) in aligned.enumerate_pixels() {
        let o = (y * SIZE + x) as usize;
        for c in 0..3 {
            t[c * plane + o] = p[c] as f32;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_recovers_a_known_similarity() {
        let truth = Similarity { a: 1.7, b: 0.9, tx: 30.0, ty: -12.0 };
        let src = TEMPLATE.map(|p| truth.apply(p)); // points as seen in some photo
        let s = fit(&src, &TEMPLATE); // maps the photo's points back to the template
        for (p, q) in src.iter().zip(TEMPLATE.iter()) {
            let m = s.apply(*p);
            assert!((m.0 - q.0).abs() < 1e-2 && (m.1 - q.1).abs() < 1e-2, "{m:?} vs {q:?}");
        }
    }

    #[test]
    fn inverse_undoes_apply() {
        let s = Similarity { a: 0.4, b: -0.3, tx: 5.0, ty: 7.0 };
        let p = s.inverse().apply(s.apply((12.0, 34.0)));
        assert!((p.0 - 12.0).abs() < 1e-3 && (p.1 - 34.0).abs() < 1e-3);
    }

    #[test]
    fn align_of_a_face_at_twice_template_scale_samples_every_second_pixel() {
        // R channel = x, G channel = y over 224x224; landmarks are the template doubled and shifted by (10, 6).
        let img = RgbImage::from_fn(224, 224, |x, y| Rgb([x as u8, y as u8, 0]));
        let lm = TEMPLATE.map(|(x, y)| (2.0 * x + 10.0, 2.0 * y + 6.0));
        let out = align(&img, &lm);
        assert_eq!(out.dimensions(), (112, 112));
        let p = out.get_pixel(50, 40).0; // template (50,40) <- photo (110, 86)
        assert!((p[0] as i32 - 110).abs() <= 1 && (p[1] as i32 - 86).abs() <= 1, "{p:?}");
    }

    #[test]
    fn sface_tensor_is_planar_rgb_unscaled() {
        let img = RgbImage::from_pixel(112, 112, Rgb([1, 2, 3]));
        let t = sface_tensor(&img);
        assert_eq!((t.len(), t[0], t[112 * 112], t[2 * 112 * 112]), (3 * 112 * 112, 1.0, 2.0, 3.0));
    }
}
