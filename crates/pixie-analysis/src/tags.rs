use crate::embedder::Embedder;
use crate::vectors::dot;

/// Bump when `VOCAB` or the selection rules change: every analysed photo is re-tagged from its stored vector.
pub const TAGS_VERSION: &str = "tags-1";

/// CLIP's own logit scale, so the softmax below means what the model was trained to mean.
const TEMPERATURE: f32 = 100.0;
/// A tag needs at least this cosine similarity to the photo...
const ABS_FLOOR: f32 = 0.2;
/// ...and at least this share of the probability among all labels, so a photo is not tagged with every near neighbour.
const MIN_PROB: f32 = 0.12;
pub const MAX_TAGS: usize = 5;

/// Everyday things people look for in their photos. Lowercase, shown as written.
pub const VOCAB: &[&str] = &[
    // places and scenes
    "beach", "sea", "lake", "river", "waterfall", "mountain", "forest", "countryside", "field", "desert", "snow",
    "garden", "park", "city street", "skyline", "bridge", "building", "church", "castle", "museum", "market",
    "restaurant", "kitchen", "living room", "bedroom", "bathroom", "office", "school", "airport", "train station",
    "road", "harbour", "farm", "campsite", "playground", "swimming pool", "stadium", "concert",
    // light and weather
    "sunset", "sunrise", "night", "fireworks", "rain", "fog", "clouds", "rainbow",
    // nature
    "flowers", "trees", "autumn leaves", "grass", "rocks", "sky", "moon", "stars",
    // animals
    "dog", "cat", "horse", "cow", "sheep", "bird", "fish", "butterfly", "insect", "duck", "deer", "monkey",
    "elephant", "lion", "rabbit", "chicken", "zoo animals",
    // people and occasions
    "people", "crowd", "child", "baby", "portrait", "group photo", "selfie", "couple", "family", "wedding",
    "bride", "birthday party", "graduation", "dancing", "sport", "football", "running", "cycling", "swimming",
    "skiing", "hiking", "playing music",
    // what people wear
    "hat", "sunglasses", "glasses", "dress", "uniform", "jacket", "helmet",
    // vehicles
    "car", "truck", "bus", "bicycle", "motorbike", "boat", "ship", "plane", "train", "tractor",
    // food and drink
    "food", "cake", "pizza", "coffee", "drinks", "fruit", "vegetables", "barbecue", "ice cream", "bread",
    "breakfast", "wine", "candles",
    // things
    "book", "computer", "phone", "television", "toys", "balloons", "christmas tree", "presents", "tent", "flag",
    "statue", "painting", "sign", "map",
    // paperwork and screens
    "document", "screenshot", "receipt", "whiteboard", "text",
    // how it was shot
    "close-up", "aerial view", "panorama", "black and white photo", "blurry photo",
];

fn softmax(scores: &[f32]) -> Vec<f32> {
    let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = scores.iter().map(|s| ((s - max) * TEMPERATURE).exp()).collect();
    let sum: f32 = exp.iter().sum();
    exp.iter().map(|e| e / sum).collect()
}

/// Indices of the labels worth tagging, best first: similar enough in absolute terms, a real share of the
/// probability among all labels, and at most `max` of them.
pub fn select(scores: &[f32], max: usize, floor: f32, min_prob: f32) -> Vec<usize> {
    if scores.is_empty() {
        return Vec::new();
    }
    let probs = softmax(scores);
    let mut idx: Vec<usize> = (0..scores.len()).filter(|&i| scores[i] >= floor && probs[i] >= min_prob).collect();
    idx.sort_by(|&a, &b| probs[b].total_cmp(&probs[a]));
    idx.truncate(max);
    idx
}

/// The labels embedded once as text, ready to be compared with any number of photo vectors.
pub struct TagIndex {
    labels: Vec<String>,
    vectors: Vec<Vec<f32>>,
}

impl TagIndex {
    pub fn build(embedder: &dyn Embedder, labels: &[&str]) -> Result<TagIndex, String> {
        let mut vectors = Vec::with_capacity(labels.len());
        for l in labels {
            vectors.push(embedder.embed_text(&format!("a photo of {l}")).map_err(|e| format!("cannot embed \"{l}\": {e}"))?);
        }
        Ok(TagIndex { labels: labels.iter().map(|l| l.to_string()).collect(), vectors })
    }

    /// The tags for one photo vector (unit length), best first, each with its probability among the labels.
    pub fn tags_for(&self, image: &[f32]) -> Vec<(String, f32)> {
        let scores: Vec<f32> = self.vectors.iter().map(|v| dot(image, v)).collect();
        let probs = softmax(&scores);
        select(&scores, MAX_TAGS, ABS_FLOOR, MIN_PROB).into_iter().map(|i| (self.labels[i].clone(), probs[i])).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::Embedder;

    /// Each label's prompt maps to its own axis; the image side is never used here.
    struct Axes(Vec<&'static str>);

    impl Embedder for Axes {
        fn embed_image(&self, _: &image::DynamicImage) -> Result<Vec<f32>, String> {
            Err("unused".into())
        }
        fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
            let i = self.0.iter().position(|l| text.contains(l)).ok_or("unknown label")?;
            let mut v = vec![0.0; self.0.len()];
            v[i] = 1.0;
            Ok(v)
        }
        fn version(&self) -> &str {
            "axes"
        }
    }

    #[test]
    fn a_clear_winner_is_the_only_tag() {
        assert_eq!(select(&[0.30, 0.25, 0.10], 5, 0.2, 0.12), vec![0]);
    }

    #[test]
    fn close_runners_up_are_kept_best_first() {
        assert_eq!(select(&[0.29, 0.30, 0.10], 5, 0.2, 0.12), vec![1, 0]);
    }

    #[test]
    fn nothing_above_the_similarity_floor_means_no_tags() {
        assert!(select(&[0.15, 0.14, 0.10], 5, 0.2, 0.12).is_empty());
    }

    #[test]
    fn a_label_below_the_floor_is_dropped_even_if_it_wins_the_softmax() {
        assert_eq!(select(&[0.30, 0.1999], 5, 0.2, 0.12), vec![0]);
    }

    #[test]
    fn the_number_of_tags_is_capped() {
        assert_eq!(select(&[0.3; 8], 5, 0.2, 0.0).len(), 5);
    }

    #[test]
    fn index_tags_a_vector_with_the_nearest_labels_and_a_probability_score() {
        let labels = vec!["beach", "forest", "hat"];
        let idx = TagIndex::build(&Axes(labels.clone()), &labels).unwrap();
        let tags = idx.tags_for(&[0.0, 0.9, 0.0]); // pure "forest" direction, similarity 0.9
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "forest");
        assert!(tags[0].1 > 0.9 && tags[0].1 <= 1.0);
    }

    #[test]
    fn index_with_no_confident_match_gives_no_tags() {
        let labels = vec!["beach", "forest"];
        let idx = TagIndex::build(&Axes(labels.clone()), &labels).unwrap();
        assert!(idx.tags_for(&[0.05, 0.05]).is_empty());
    }

    #[test]
    fn a_label_the_embedder_cannot_embed_is_an_error() {
        assert!(TagIndex::build(&Axes(vec!["beach"]), &["beach", "castle"]).is_err());
    }

    #[test]
    fn the_built_in_vocabulary_is_sane() {
        assert!(VOCAB.len() >= 120);
        let mut sorted = VOCAB.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), VOCAB.len(), "duplicate labels");
        for l in ["beach", "forest", "hat", "people"] {
            assert!(VOCAB.contains(&l), "{l}");
        }
        assert!(VOCAB.iter().all(|l| l == &l.trim() && !l.is_empty() && *l == l.to_lowercase()));
    }
}
