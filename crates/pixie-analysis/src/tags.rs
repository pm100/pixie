use crate::embedder::Embedder;
use crate::vectors::dot;

/// Bump when `VOCAB` or the selection rules change: every analysed photo is re-tagged from its stored vector.
pub const TAGS_VERSION: &str = "tags-2";

/// CLIP's own logit scale, so the softmax below means what the model was trained to mean.
const TEMPERATURE: f32 = 100.0;
/// A tag needs at least this cosine similarity to the photo...
const ABS_FLOOR: f32 = 0.22;
/// ...and at least this share of the probability among all labels, so a photo is not tagged with every near neighbour.
const MIN_PROB: f32 = 0.15;
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

/// Things photos often show that are not in `VOCAB`. They compete in the softmax but are never shown as tags:
/// a photo of a ring then goes to "ring", not to the nearest real label ("sunglasses").
pub const DECOYS: &[&str] = &[
    "ring", "jewellery", "watch", "necklace", "hands", "feet", "face", "hair", "tattoo", "shoes", "bag", "clothes",
    "furniture", "plant", "tools", "machine", "toy animal", "insect close-up", "reptile", "spider", "pet",
    "wild animal", "sculpture", "pattern", "texture", "logo", "poster", "box", "bottle", "plate", "cup", "window",
    "door", "wall", "floor", "ceiling", "bed", "chair", "table", "lamp", "mirror", "shelf", "cables", "pavement",
    "an object on a table", "an abstract image", "a dark photo", "an empty room", "a person in a costume",
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
    /// The shown labels first, then the decoys.
    labels: Vec<String>,
    vectors: Vec<Vec<f32>>,
    shown: usize,
}

impl TagIndex {
    pub fn build(embedder: &dyn Embedder, labels: &[&str]) -> Result<TagIndex, String> {
        Self::build_with(embedder, labels, DECOYS)
    }

    pub fn build_with(embedder: &dyn Embedder, labels: &[&str], decoys: &[&str]) -> Result<TagIndex, String> {
        let all: Vec<&str> = labels.iter().chain(decoys.iter()).copied().collect();
        let mut vectors = Vec::with_capacity(all.len());
        for l in &all {
            vectors.push(embedder.embed_text(&format!("a photo of {l}")).map_err(|e| format!("cannot embed \"{l}\": {e}"))?);
        }
        Ok(TagIndex { labels: all.iter().map(|l| l.to_string()).collect(), vectors, shown: labels.len() })
    }

    /// The tags for one photo vector (unit length), best first, each with its probability among all labels
    /// (decoys included, so the probability is a share against "something else", not just against other tags).
    pub fn tags_for(&self, image: &[f32]) -> Vec<(String, f32)> {
        let scores: Vec<f32> = self.vectors.iter().map(|v| dot(image, v)).collect();
        let probs = softmax(&scores);
        select(&scores, usize::MAX, ABS_FLOOR, MIN_PROB)
            .into_iter()
            .filter(|&i| i < self.shown)
            .take(MAX_TAGS)
            .map(|i| (self.labels[i].clone(), probs[i]))
            .collect()
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
        let idx = TagIndex::build_with(&Axes(labels.clone()), &labels, &[]).unwrap();
        let tags = idx.tags_for(&[0.0, 0.9, 0.0]); // pure "forest" direction, similarity 0.9
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "forest");
        assert!(tags[0].1 > 0.9 && tags[0].1 <= 1.0);
    }

    #[test]
    fn index_with_no_confident_match_gives_no_tags() {
        let labels = vec!["beach", "forest"];
        let idx = TagIndex::build_with(&Axes(labels.clone()), &labels, &[]).unwrap();
        assert!(idx.tags_for(&[0.05, 0.05]).is_empty());
    }

    #[test]
    fn a_decoy_that_fits_better_takes_the_probability_and_is_never_shown() {
        // axes: sunglasses, ring (decoy). A photo between them, nearer the ring.
        let ax = Axes(vec!["sunglasses", "ring"]);
        let idx = TagIndex::build_with(&ax, &["sunglasses"], &["ring"]).unwrap();
        assert!(idx.tags_for(&[0.18, 0.30]).is_empty(), "ring photo must not be tagged sunglasses");
        let tags = idx.tags_for(&[0.30, 0.05]);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "sunglasses");
    }

    #[test]
    fn decoys_do_not_use_up_the_tag_cap() {
        let names: Vec<&'static str> = vec!["a", "b", "c", "d", "e", "f", "x", "y"];
        let ax = Axes(names.clone());
        let idx = TagIndex::build_with(&ax, &names[..6], &names[6..]).unwrap();
        let tags = idx.tags_for(&[0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.31, 0.31]);
        assert!(tags.len() <= MAX_TAGS);
        assert!(tags.iter().all(|t| !["x", "y"].contains(&t.0.as_str())));
    }

    #[test]
    fn decoys_are_distinct_from_the_vocabulary() {
        for d in DECOYS {
            assert!(!VOCAB.contains(d), "{d}");
        }
        let mut s = DECOYS.to_vec();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), DECOYS.len());
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
