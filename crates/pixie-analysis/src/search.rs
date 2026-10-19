use crate::embedder::Embedder;
use crate::vectors::top_k;
use pixie_index::Db;

/// Cosine similarity below which a photo is not a match. CLIP scores a true text-to-image match around 0.25-0.35
/// and unrelated pairs near 0.1, so this keeps real matches and drops noise.
pub const SEARCH_FLOOR: f32 = 0.2;
pub const SEARCH_LIMIT: usize = 500;

/// Rank every analysed photo against `text` and make the best ones the current search result (see
/// `PhotoQuery::search`). Blank text clears the result. Returns how many photos matched.
pub fn run(db: &Db, embedder: &dyn Embedder, text: &str, k: usize, floor: f32) -> Result<usize, String> {
    let items = if text.trim().is_empty() { Vec::new() } else { db.load_embeddings(embedder.version()).map_err(|e| e.to_string())? };
    run_with(db, embedder, text, k, floor, &items)
}

/// `run` with the vectors already loaded (the service keeps them between searches).
pub fn run_with(db: &Db, embedder: &dyn Embedder, text: &str, k: usize, floor: f32, items: &[(i64, Vec<f32>)]) -> Result<usize, String> {
    if text.trim().is_empty() {
        db.clear_search_hits().map_err(|e| e.to_string())?;
        return Ok(0);
    }
    let query = embedder.embed_text(text.trim())?;
    let ids: Vec<i64> = top_k(&query, items, k, floor).into_iter().map(|h| h.id).collect();
    db.set_search_hits(&ids).map_err(|e| e.to_string())?;
    Ok(ids.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::FakeEmbedder;
    use pixie_core::{PhotoMeta, PhotoQuery};

    fn library() -> (Db, i64, i64) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let cat = db.upsert_photo(f, "/p/cat.jpg", 1, 1, &PhotoMeta::default()).unwrap();
        let other = db.upsert_photo(f, "/p/other.jpg", 1, 1, &PhotoMeta::default()).unwrap();
        db.put_embedding(cat, "fake-1", &[1.0, 0.0]).unwrap();
        db.put_embedding(other, "fake-1", &[0.0, 1.0]).unwrap();
        (db, cat, other)
    }

    fn hits(db: &Db) -> Vec<i64> {
        let q = PhotoQuery { search: true, ..Default::default() };
        db.list_photos_in(&q, 0, 100).unwrap().iter().map(|p| p.id).collect()
    }

    #[test]
    fn search_sets_hits_to_matching_photos_only() {
        let (db, cat, _) = library();
        assert_eq!(run(&db, &FakeEmbedder, "cat", SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), 1);
        assert_eq!(hits(&db), vec![cat]);
    }

    #[test]
    fn blank_query_clears_the_hits() {
        let (db, _, _) = library();
        run(&db, &FakeEmbedder, "cat", SEARCH_LIMIT, SEARCH_FLOOR).unwrap();
        assert_eq!(run(&db, &FakeEmbedder, "   ", SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), 0);
        assert!(hits(&db).is_empty());
    }

    #[test]
    fn nothing_above_the_floor_is_zero_hits_not_an_error() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &PhotoMeta::default()).unwrap();
        db.put_embedding(a, "fake-1", &[0.0, 1.0]).unwrap();
        assert_eq!(run(&db, &FakeEmbedder, "cat", SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), 0);
        assert!(hits(&db).is_empty());
    }

    #[test]
    fn limit_caps_the_hits() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        for i in 0..5 {
            let id = db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &PhotoMeta::default()).unwrap();
            db.put_embedding(id, "fake-1", &[1.0, 0.0]).unwrap();
        }
        assert_eq!(run(&db, &FakeEmbedder, "cat", 3, SEARCH_FLOOR).unwrap(), 3);
    }

    #[test]
    fn photos_with_embeddings_from_another_model_are_not_searched() {
        let (db, cat, _) = library();
        db.put_embedding(cat, "old-model", &[1.0, 0.0]).unwrap();
        let other_model = db.load_embeddings("old-model").unwrap();
        assert_eq!(other_model.len(), 1);
        // The fake embedder is "fake-1": only its own vectors count, so exactly the one cat photo matches.
        assert_eq!(run(&db, &FakeEmbedder, "cat", SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), 1);
    }
}
