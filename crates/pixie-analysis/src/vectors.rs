pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Scale to unit length; the zero vector stays as it is.
pub fn normalize(v: &mut [f32]) {
    let n = dot(v, v).sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub id: i64,
    pub score: f32,
}

/// The `k` best matches scoring at least `floor`, best first. Vectors are expected to be unit length.
pub fn top_k(query: &[f32], items: &[(i64, Vec<f32>)], k: usize, floor: f32) -> Vec<Hit> {
    let mut hits: Vec<Hit> =
        items.iter().map(|(id, v)| Hit { id: *id, score: dot(query, v) }).filter(|h| h.score >= floor).collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(k);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_makes_unit_length() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((dot(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_zero_vector_is_left_alone() {
        let mut v = vec![0.0, 0.0];
        normalize(&mut v);
        assert_eq!(v, vec![0.0, 0.0]);
    }

    #[test]
    fn top_k_orders_limits_and_applies_floor() {
        let items = vec![(1, vec![1.0, 0.0]), (2, vec![0.8, 0.6]), (3, vec![0.0, 1.0])];
        let hits = top_k(&[1.0, 0.0], &items, 2, 0.1);
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![1, 2]);
        let strict = top_k(&[1.0, 0.0], &items, 5, 0.9);
        assert_eq!(strict.len(), 1);
        assert!(strict.iter().all(|h| h.score >= 0.9));
    }
}
