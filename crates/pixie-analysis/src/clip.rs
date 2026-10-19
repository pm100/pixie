use crate::clip_pre::to_tensor;
use crate::embedder::Embedder;
use crate::vectors::normalize;
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::Tokenizer;

pub const MODEL_VERSION: &str = "clip-b32-q8-1";
const TEXT_LEN: usize = 77;

/// OpenAI CLIP ViT-B/32 (int8) running on the CPU through ONNX Runtime.
pub struct ClipEmbedder {
    vision: Mutex<Session>,
    text: Mutex<Session>,
    tokenizer: Tokenizer,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl ClipEmbedder {
    /// `dir` holds `vision.onnx`, `text.onnx` and `tokenizer.json` (see `models::CLIP_FILES`).
    pub fn load(dir: &Path) -> Result<ClipEmbedder, String> {
        let session = |name: &str| -> Result<Session, String> {
            Session::builder()
                .and_then(|b| b.with_intra_threads(1))
                .and_then(|b| b.commit_from_file(dir.join(name)))
                .map_err(|e| format!("cannot load {name}: {e}"))
        };
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| format!("cannot load tokenizer: {e}"))?;
        // We pad ourselves (with 0, as CLIP does) so the sequence is always exactly TEXT_LEN.
        tokenizer.with_padding(None);
        Ok(ClipEmbedder { vision: Mutex::new(session("vision.onnx")?), text: Mutex::new(session("text.onnx")?), tokenizer })
    }

    fn token_ids(&self, text: &str) -> Result<Vec<i64>, String> {
        let enc = self.tokenizer.encode(text, true).map_err(err)?;
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        if ids.len() > TEXT_LEN {
            let eot = *ids.last().unwrap_or(&0);
            ids.truncate(TEXT_LEN);
            ids[TEXT_LEN - 1] = eot; // the model reads the end-of-text token, so keep it
        }
        ids.resize(TEXT_LEN, 0);
        Ok(ids)
    }
}

impl Embedder for ClipEmbedder {
    fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
        let input = Tensor::from_array(([1usize, 3, 224, 224], to_tensor(img))).map_err(err)?;
        let mut session = self.vision.lock().unwrap_or_else(|p| p.into_inner());
        let out = session.run(ort::inputs!["pixel_values" => input]).map_err(err)?;
        let mut v: Vec<f32> = out["image_embeds"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec();
        normalize(&mut v);
        Ok(v)
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let input = Tensor::from_array(([1usize, TEXT_LEN], self.token_ids(text)?)).map_err(err)?;
        let mut session = self.text.lock().unwrap_or_else(|p| p.into_inner());
        let out = session.run(ort::inputs!["input_ids" => input]).map_err(err)?;
        let mut v: Vec<f32> = out["text_embeds"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec();
        normalize(&mut v);
        Ok(v)
    }

    fn version(&self) -> &str {
        MODEL_VERSION
    }
}
