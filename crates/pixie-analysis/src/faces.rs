use crate::align::{align, sface_tensor};
use crate::vectors::normalize;
use crate::yunet::{decode, letterbox, nms, to_faces, Face, Raw, INPUT};
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::Mutex;

/// Identifies the detector + recogniser pair; stored with every scan so new models rescan the library.
pub const FACES_VERSION: &str = "yunet2023mar-sface2021dec-1";

/// Cells scoring below this are not even candidates (`yunet::MIN_SCORE` is what a stored face must reach).
const DECODE_THRESHOLD: f32 = 0.6;
const NMS_IOU: f32 = 0.3;

/// Finds faces in a photo and turns each into a vector; faces of the same person get similar (unit) vectors.
pub trait FaceEngine: Send + Sync {
    /// Faces in the upright image, best first.
    fn detect(&self, img: &DynamicImage) -> Result<Vec<Face>, String>;
    fn embed(&self, img: &DynamicImage, face: &Face) -> Result<Vec<f32>, String>;
    fn version(&self) -> &str;
}

/// A stand-in for tests: an image whose top-left pixel is red-dominant (red above 128) holds one face covering the
/// whole picture; anything else has none. The face's vector is that pixel's colour, normalised.
pub struct FakeFaceEngine;

impl FaceEngine for FakeFaceEngine {
    fn detect(&self, img: &DynamicImage) -> Result<Vec<Face>, String> {
        let p = img.to_rgb8().get_pixel(0, 0).0;
        if p[0] <= 128 {
            return Ok(Vec::new());
        }
        let lm = crate::align::TEMPLATE.map(|(x, y)| (x / 112.0, y / 112.0));
        Ok(vec![Face { x: 0.0, y: 0.0, w: 1.0, h: 1.0, score: 0.99, landmarks: lm }])
    }

    fn embed(&self, img: &DynamicImage, _face: &Face) -> Result<Vec<f32>, String> {
        let p = img.to_rgb8().get_pixel(0, 0).0;
        let mut v = vec![p[0] as f32, p[1] as f32, p[2] as f32];
        normalize(&mut v);
        Ok(v)
    }

    fn version(&self) -> &str {
        "fake-faces-1"
    }
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// YuNet (detector) and SFace (recogniser) on the CPU through ONNX Runtime.
pub struct FaceModels {
    yunet: Mutex<Session>,
    sface: Mutex<Session>,
}

impl FaceModels {
    /// `dir` holds `yunet.onnx` and `sface.onnx` (see `models::FACE_FILES`).
    pub fn load(dir: &Path) -> Result<FaceModels, String> {
        let session = |name: &str| -> Result<Session, String> {
            Session::builder()
                .and_then(|b| b.with_intra_threads(1))
                .and_then(|b| b.commit_from_file(dir.join(name)))
                .map_err(|e| format!("cannot load {name}: {e}"))
        };
        Ok(FaceModels { yunet: Mutex::new(session("yunet.onnx")?), sface: Mutex::new(session("sface.onnx")?) })
    }
}

impl FaceEngine for FaceModels {
    fn detect(&self, img: &DynamicImage) -> Result<Vec<Face>, String> {
        let (tensor, rw, rh) = letterbox(img);
        let input = Tensor::from_array(([1usize, 3, INPUT, INPUT], tensor)).map_err(err)?;
        let mut session = self.yunet.lock().unwrap_or_else(|p| p.into_inner());
        let out = session.run(ort::inputs!["input" => input]).map_err(err)?;
        let grab = |name: &str| -> Result<Vec<f32>, String> {
            Ok(out[name].try_extract_tensor::<f32>().map_err(err)?.1.to_vec())
        };
        let (cls, obj, bbox, kps) = (
            [grab("cls_8")?, grab("cls_16")?, grab("cls_32")?],
            [grab("obj_8")?, grab("obj_16")?, grab("obj_32")?],
            [grab("bbox_8")?, grab("bbox_16")?, grab("bbox_32")?],
            [grab("kps_8")?, grab("kps_16")?, grab("kps_32")?],
        );
        let raw = Raw {
            cls: [&cls[0], &cls[1], &cls[2]],
            obj: [&obj[0], &obj[1], &obj[2]],
            bbox: [&bbox[0], &bbox[1], &bbox[2]],
            kps: [&kps[0], &kps[1], &kps[2]],
        };
        Ok(to_faces(nms(decode(&raw, DECODE_THRESHOLD), NMS_IOU), rw, rh))
    }

    fn embed(&self, img: &DynamicImage, face: &Face) -> Result<Vec<f32>, String> {
        let rgb = img.to_rgb8();
        let (w, h) = (rgb.width() as f32, rgb.height() as f32);
        let lm = face.landmarks.map(|(x, y)| (x * w, y * h));
        let input = Tensor::from_array(([1usize, 3, 112, 112], sface_tensor(&align(&rgb, &lm)))).map_err(err)?;
        let mut session = self.sface.lock().unwrap_or_else(|p| p.into_inner());
        let out = session.run(ort::inputs!["data" => input]).map_err(err)?;
        let mut v: Vec<f32> = out["fc1"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec();
        normalize(&mut v);
        Ok(v)
    }

    fn version(&self) -> &str {
        FACES_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn px(r: u8, g: u8, b: u8) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(16, 16, Rgb([r, g, b])))
    }

    #[test]
    fn the_fake_engine_finds_a_face_in_a_red_image_and_none_in_a_blue_one() {
        let e = FakeFaceEngine;
        assert_eq!(e.detect(&px(250, 0, 0)).unwrap().len(), 1);
        assert!(e.detect(&px(0, 0, 250)).unwrap().is_empty());
    }

    #[test]
    fn the_fake_engine_embeds_to_a_unit_vector() {
        let e = FakeFaceEngine;
        let img = px(250, 40, 10);
        let f = e.detect(&img).unwrap().remove(0);
        let v = e.embed(&img, &f).unwrap();
        assert!((v.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-5);
    }
}
