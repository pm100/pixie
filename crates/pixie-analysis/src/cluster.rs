use crate::vectors::{dot, normalize};

/// A face joins a person when its cosine similarity to that person's centroid is at least this. SFace's own
/// same-person threshold is 0.363; this is stricter on purpose: a wrongly merged person is worse than a split one,
/// which Merge repairs.
pub const JOIN_THRESHOLD: f32 = 0.42;

/// A person as the grouping sees them: the (unit) mean of their faces' embeddings and how many faces that is.
#[derive(Debug, Clone, PartialEq)]
pub struct Centroid {
    pub id: i64,
    pub vector: Vec<f32>,
    pub n: i64,
}

/// Index of the most similar person and the similarity.
pub fn nearest(face: &[f32], people: &[Centroid]) -> Option<(usize, f32)> {
    people.iter().enumerate().map(|(i, c)| (i, dot(face, &c.vector))).max_by(|a, b| a.1.total_cmp(&b.1))
}

/// The person this face belongs to, if any is similar enough.
pub fn assign(face: &[f32], people: &[Centroid], threshold: f32) -> Option<usize> {
    assign_excluding(face, people, threshold, &[])
}

/// `assign`, never choosing a person in `excluded` (ids): the people this face was rejected from.
pub fn assign_excluding(face: &[f32], people: &[Centroid], threshold: f32, excluded: &[i64]) -> Option<usize> {
    people
        .iter()
        .enumerate()
        .filter(|(_, c)| !excluded.contains(&c.id))
        .map(|(i, c)| (i, dot(face, &c.vector)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, s)| *s >= threshold)
        .map(|(i, _)| i)
}

/// Fold one more face into the running mean.
pub fn absorb(c: &mut Centroid, face: &[f32]) {
    let n = c.n as f32;
    for (v, f) in c.vector.iter_mut().zip(face) {
        *v = (*v * n + f) / (n + 1.0);
    }
    normalize(&mut c.vector);
    c.n += 1;
}

/// The centroid of two people merged, weighted by how many faces each has.
pub fn combine(a: &Centroid, b: &Centroid) -> Vec<f32> {
    let (wa, wb) = (a.n.max(1) as f32, b.n.max(1) as f32);
    let mut v: Vec<f32> = a.vector.iter().zip(&b.vector).map(|(x, y)| x * wa + y * wb).collect();
    normalize(&mut v);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: i64, v: [f32; 2], n: i64) -> Centroid {
        Centroid { id, vector: v.to_vec(), n }
    }

    #[test]
    fn a_face_joins_the_closest_person_above_the_threshold() {
        let people = vec![c(1, [1.0, 0.0], 3), c(2, [0.0, 1.0], 3)];
        assert_eq!(assign(&[0.6, 0.8], &people, 0.42), Some(1));
        assert_eq!(assign(&[0.9, 0.1], &people, 0.42), Some(0));
    }

    #[test]
    fn a_face_like_nobody_joins_nobody() {
        let people = vec![c(1, [1.0, 0.0], 3)];
        assert_eq!(assign(&[0.0, 1.0], &people, 0.42), None);
        assert_eq!(assign(&[1.0, 0.0], &[], 0.42), None);
    }

    #[test]
    fn a_face_exactly_at_the_threshold_joins() {
        assert_eq!(assign(&[0.5, 0.0], &[c(1, [1.0, 0.0], 1)], 0.5), Some(0));
    }

    #[test]
    fn absorb_moves_the_centroid_toward_the_face_and_counts_it() {
        let mut p = c(1, [1.0, 0.0], 1);
        absorb(&mut p, &[0.0, 1.0]);
        assert_eq!(p.n, 2);
        assert!((p.vector[0] - p.vector[1]).abs() < 1e-6);
        assert!((dot(&p.vector, &p.vector) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn combine_weights_by_face_count() {
        let mid = combine(&c(1, [1.0, 0.0], 1), &c(2, [0.0, 1.0], 1));
        assert!((mid[0] - mid[1]).abs() < 1e-6);
        let heavy = combine(&c(1, [1.0, 0.0], 9), &c(2, [0.0, 1.0], 1));
        assert!(heavy[0] > heavy[1] * 5.0);
    }

    #[test]
    fn a_face_never_joins_a_person_it_was_rejected_from() {
        let people = vec![c(1, [1.0, 0.0], 3), c(2, [0.8, 0.6], 3)];
        assert_eq!(assign(&[1.0, 0.0], &people, 0.42), Some(0)); // person 1 is the best match...
        assert_eq!(assign_excluding(&[1.0, 0.0], &people, 0.42, &[1]), Some(1)); // ...so skip them: the next best
        assert_eq!(assign_excluding(&[1.0, 0.0], &people, 0.42, &[1, 2]), None); // nobody left
        assert_eq!(assign_excluding(&[0.0, 1.0], &people, 0.9, &[]), None); // the threshold still applies
    }
}
