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
    let ids = rank(embedder, text, items, k, floor)?;
    db.set_search_hits(&ids).map_err(|e| e.to_string())?;
    Ok(ids.len())
}

/// The photos that best match `text`, best first, without changing the current search result. Blank text matches nothing.
pub fn rank(embedder: &dyn Embedder, text: &str, items: &[(i64, Vec<f32>)], k: usize, floor: f32) -> Result<Vec<i64>, String> {
    Ok(rank_scored(embedder, text, items, k, floor)?.into_iter().map(|(id, _)| id).collect())
}

/// What a content search found for one thing, and how its scores spread over every analysed photo (so a score can be read as "how
/// many standard deviations above the usual photo").
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    /// The photos at or above the floor, best first, with their scores.
    pub hits: Vec<(i64, f32)>,
    pub mean: f32,
    pub sd: f32,
}

/// `rank_scored` with the mean and standard deviation of the score over all the photos, not only those above the floor. Blank text
/// matches nothing.
pub fn rank_with_stats(embedder: &dyn Embedder, text: &str, items: &[(i64, Vec<f32>)], k: usize, floor: f32) -> Result<(Vec<(i64, f32)>, f32, f32), String> {
    if text.trim().is_empty() || items.is_empty() {
        return Ok((Vec::new(), 0.0, 1.0));
    }
    let query = embedder.embed_text(text.trim())?;
    let all = top_k(&query, items, items.len(), f32::MIN);
    let n = all.len() as f32;
    let mean = all.iter().map(|h| h.score).sum::<f32>() / n;
    let sd = (all.iter().map(|h| (h.score - mean) * (h.score - mean)).sum::<f32>() / n).sqrt().max(1e-6);
    let hits = all.into_iter().take_while(|h| h.score >= floor).take(k).map(|h| (h.id, h.score)).collect();
    Ok((hits, mean, sd))
}

/// `rank` with each photo's score (cosine similarity), best first.
pub fn rank_scored(embedder: &dyn Embedder, text: &str, items: &[(i64, Vec<f32>)], k: usize, floor: f32) -> Result<Vec<(i64, f32)>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let query = embedder.embed_text(text.trim())?;
    Ok(top_k(&query, items, k, floor).into_iter().map(|h| (h.id, h.score)).collect())
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
    fn rank_gives_the_matching_ids_and_leaves_the_current_search_alone() {
        let (db, cat, other) = library();
        run(&db, &FakeEmbedder, "other", SEARCH_LIMIT, SEARCH_FLOOR).unwrap();
        let items = db.load_embeddings("fake-1").unwrap();
        assert_eq!(rank(&FakeEmbedder, "cat", &items, SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), vec![cat]);
        assert_eq!(hits(&db), vec![other]);
        assert!(rank(&FakeEmbedder, "  ", &items, SEARCH_LIMIT, SEARCH_FLOOR).unwrap().is_empty());
    }

    #[test]
    fn rank_scored_gives_the_scores_best_first() {
        let (db, cat, _) = library();
        let items = db.load_embeddings("fake-1").unwrap();
        let scored = rank_scored(&FakeEmbedder, "cat", &items, SEARCH_LIMIT, SEARCH_FLOOR).unwrap();
        assert_eq!(scored.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![cat]);
        assert!(scored[0].1 > 0.9);
    }

    #[test]
    fn rank_with_stats_also_gives_how_the_scores_spread_over_all_the_photos() {
        let (db, cat, _) = library();
        let items = db.load_embeddings("fake-1").unwrap();
        let (hits, mean, sd) = rank_with_stats(&FakeEmbedder, "cat", &items, SEARCH_LIMIT, SEARCH_FLOOR).unwrap();
        assert_eq!(hits.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![cat]);
        // the cat photo scores 1, the other 0: mean 0.5, standard deviation 0.5, counting the photo that is below the floor too
        assert!((mean - 0.5).abs() < 1e-5 && (sd - 0.5).abs() < 1e-5, "{mean} {sd}");
        assert_eq!(rank_with_stats(&FakeEmbedder, " ", &items, SEARCH_LIMIT, SEARCH_FLOOR).unwrap(), (vec![], 0.0, 1.0));
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
