//! Grouping faces by what is known about all of them at once.
//!
//! The first pass over a library places each face as it arrives, against the groups as they are at that moment: a
//! greedy, order-dependent guess. These functions look at every face together, so what is learned later (a face between
//! two groups, a group that is really two people, the faces the user has confirmed) can put early guesses right.
//!
//! Faces are unit vectors (SFace's 128 numbers), so the dot product of two is their cosine similarity.

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// Two faces look like different people when their similarity is below this (SFace's own same-person value).
pub const DIFFERENT: f32 = 0.363;

/// A group is "mixed" (looks like more than one person) when more than this share of its pairs look like different
/// people. Measured on real libraries, groups of one man or one woman sit at 3 to 8 percent, and groups that are clearly
/// a blend of several people at 20 to 30 percent.
pub const MIXED_ABOVE: f32 = 0.06;

/// How many faces are compared pair by pair when judging a group; more would only be slower.
const MIXED_SAMPLE: usize = 300;

/// What is clustered: a set of faces that are very alike (or one face) stands in for all of them.
struct Unit {
    members: Vec<usize>,
    /// The sum of the members' vectors. The mean pairwise similarity of two units is `sum_a . sum_b / (n_a * n_b)`.
    sum: Vec<f32>,
}

/// Pre-group very alike faces so the pairwise table stays small for a big library. With `max_units` or fewer faces every
/// face is its own unit and the clustering below is exact.
fn units_of(vectors: &[&[f32]], max_units: usize) -> Vec<Unit> {
    let single = |vectors: &[&[f32]]| vectors.iter().enumerate().map(|(i, v)| Unit { members: vec![i], sum: v.to_vec() }).collect::<Vec<_>>();
    if vectors.len() <= max_units {
        return single(vectors);
    }
    let mut tight = 0.90f32;
    loop {
        let mut units: Vec<Unit> = Vec::new();
        let mut dirs: Vec<Vec<f32>> = Vec::new();
        for (i, v) in vectors.iter().enumerate() {
            let best = dirs.iter().enumerate().map(|(u, d)| (u, dot(v, d))).max_by(|a, b| a.1.total_cmp(&b.1));
            match best {
                Some((u, s)) if s >= tight => {
                    for (a, b) in units[u].sum.iter_mut().zip(v.iter()) {
                        *a += b;
                    }
                    units[u].members.push(i);
                    dirs[u] = units[u].sum.clone();
                    normalize(&mut dirs[u]);
                }
                _ => {
                    units.push(Unit { members: vec![i], sum: v.to_vec() });
                    dirs.push(v.to_vec());
                }
            }
        }
        if units.len() <= max_units || tight <= 0.45 {
            return units;
        }
        tight -= 0.05;
    }
}

/// Average-linkage clustering (UPGMA, by the nearest-neighbour chain): repeatedly join the two groups with the highest
/// mean pairwise similarity, and stop when that falls below `cut`. Returns the groups as lists of indexes into `vectors`.
///
/// Unlike placing faces one by one against a running average, this judges a group by *all* its members, so the average
/// of a few similar-looking people cannot become a blurry face that attracts the rest.
pub fn upgma(vectors: &[&[f32]], cut: f32, max_units: usize) -> Vec<Vec<usize>> {
    if vectors.is_empty() {
        return Vec::new();
    }
    let units = units_of(vectors, max_units.max(2));
    let m = units.len();
    let mut s = vec![f32::NEG_INFINITY; m * m];
    for i in 0..m {
        for j in (i + 1)..m {
            let v = dot(&units[i].sum, &units[j].sum) / (units[i].members.len() * units[j].members.len()) as f32;
            s[i * m + j] = v;
            s[j * m + i] = v;
        }
    }
    let mut size: Vec<f32> = units.iter().map(|u| u.members.len() as f32).collect();
    let mut active = vec![true; m];
    let mut merges: Vec<(f32, usize, usize)> = Vec::with_capacity(m);
    let mut chain: Vec<usize> = Vec::new();
    let mut alive = m;
    let mut next_start = 0usize;
    while alive > 1 {
        if chain.is_empty() {
            while !active[next_start] {
                next_start += 1;
            }
            chain.push(next_start);
        }
        loop {
            let a = *chain.last().expect("the chain is not empty");
            let row = &s[a * m..(a + 1) * m];
            let b = row.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|(j, _)| j).expect("a row has entries");
            if chain.len() > 1 && b == chain[chain.len() - 2] {
                break;
            }
            chain.push(b);
        }
        let b = chain.pop().expect("two in the chain");
        let a = chain.pop().expect("two in the chain");
        merges.push((s[a * m + b], a, b));
        let (na, nb) = (size[a], size[b]);
        for j in 0..m {
            if active[j] && j != a && j != b {
                let v = (na * s[a * m + j] + nb * s[b * m + j]) / (na + nb);
                s[a * m + j] = v;
                s[j * m + a] = v;
            }
        }
        for j in 0..m {
            s[b * m + j] = f32::NEG_INFINITY;
            s[j * m + b] = f32::NEG_INFINITY;
        }
        size[a] += nb;
        active[b] = false;
        alive -= 1;
    }
    // Cut the tree: average linkage never joins at a higher similarity than before, so taking the joins in descending order
    // until one falls below the cut gives the groups at that level.
    let mut parent: Vec<usize> = (0..m).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    merges.sort_by(|x, y| y.0.total_cmp(&x.0));
    for (sim, a, b) in merges {
        if sim < cut {
            break;
        }
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[rb] = ra;
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for (u, unit) in units.iter().enumerate() {
        let root = find(&mut parent, u);
        groups.entry(root).or_default().extend(unit.members.iter().copied());
    }
    groups.into_values().map(|mut g| {
        g.sort_unstable();
        g
    })
    .collect()
}

/// The share of pairs in this group that look like different people (a sample of up to 300 faces, evenly spaced, so the
/// answer is the same every time). 0 for a group of one.
pub fn mixed_fraction(vectors: &[&[f32]]) -> f32 {
    let n = vectors.len();
    if n < 2 {
        return 0.0;
    }
    let take = MIXED_SAMPLE.min(n);
    let idx: Vec<usize> = (0..take).map(|k| k * n / take).collect();
    let (mut pairs, mut different) = (0u64, 0u64);
    for (i, &a) in idx.iter().enumerate() {
        for &b in &idx[i + 1..] {
            pairs += 1;
            if dot(vectors[a], vectors[b]) < DIFFERENT {
                different += 1;
            }
        }
    }
    different as f32 / pairs as f32
}

/// The group's mean face, as a unit vector.
pub fn centroid(vectors: &[&[f32]]) -> Vec<f32> {
    let mut sum = vec![0.0f32; vectors.first().map_or(0, |v| v.len())];
    for v in vectors {
        for (a, b) in sum.iter_mut().zip(v.iter()) {
            *a += b;
        }
    }
    normalize(&mut sum);
    sum
}

/// `k` faces that show the spread of a group, not just its best faces: the most typical one first, then each next one the
/// face least like those already chosen. Only the `pool` clearest pictures (by `quality`, higher is better) are
/// candidates, so what is shown is something a person can recognise.
pub fn spread(vectors: &[&[f32]], quality: &[f32], k: usize, pool: usize) -> Vec<usize> {
    if vectors.is_empty() || k == 0 {
        return Vec::new();
    }
    let mut candidates: Vec<usize> = (0..vectors.len()).collect();
    candidates.sort_by(|&a, &b| quality[b].total_cmp(&quality[a]));
    candidates.truncate(pool.max(k));
    let mean = centroid(vectors);
    let first = *candidates.iter().max_by(|&&a, &&b| dot(vectors[a], &mean).total_cmp(&dot(vectors[b], &mean))).expect("some candidate");
    let mut chosen = vec![first];
    while chosen.len() < k.min(candidates.len()) {
        let next = candidates
            .iter()
            .filter(|c| !chosen.contains(c))
            .map(|&c| (c, chosen.iter().map(|&p| dot(vectors[c], vectors[p])).fold(f32::NEG_INFINITY, f32::max)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(c, _)| c);
        match next {
            Some(c) => chosen.push(c),
            None => break,
        }
    }
    chosen
}

/// Faces the user has confirmed as one person.
pub struct Anchor<'a> {
    pub person: i64,
    pub faces: Vec<&'a [f32]>,
}

/// The person this face looks most like among the confirmed ones, never one in `excluded`: `(person, score, margin)`.
/// The score is the mean of the face's `top` highest similarities to that person's confirmed faces, so one lucky match
/// is not enough and a person with many confirmed faces is not favoured over one with a few. The margin is how far
/// ahead of the runner-up it is.
pub fn best_person(face: &[f32], anchors: &[Anchor], excluded: &[i64], top: usize) -> Option<(i64, f32, f32)> {
    let mut scored: Vec<(i64, f32)> = anchors
        .iter()
        .filter(|a| !excluded.contains(&a.person) && !a.faces.is_empty())
        .map(|a| {
            let mut sims: Vec<f32> = a.faces.iter().map(|f| dot(face, f)).collect();
            sims.sort_by(|x, y| y.total_cmp(x));
            let k = top.min(sims.len()).max(1);
            (a.person, sims[..k].iter().sum::<f32>() / k as f32)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (person, score) = *scored.first()?;
    let margin = scored.get(1).map_or(score, |second| score - second.1);
    Some((person, score, margin))
}

/// Try to take a mixed group apart: regroup its faces at stricter and stricter levels and use the first that gives at least
/// two pieces of `min_piece` faces or more. Smaller leftovers come back too (as pieces under `min_piece`); the caller decides
/// what to do with them. `None` when no level splits it.
pub fn split(vectors: &[&[f32]], levels: &[f32], min_piece: usize, max_units: usize) -> Option<Vec<Vec<usize>>> {
    for &level in levels {
        let pieces = upgma(vectors, level, max_units);
        if pieces.iter().filter(|p| p.len() >= min_piece).count() >= 2 {
            return Some(pieces);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit vector at `deg` degrees in the plane (with the third and fourth axes zero).
    fn at(deg: f32) -> Vec<f32> {
        let r = deg.to_radians();
        vec![r.cos(), r.sin(), 0.0, 0.0]
    }
    fn refs(v: &[Vec<f32>]) -> Vec<&[f32]> {
        v.iter().map(|x| x.as_slice()).collect()
    }
    fn sorted(mut g: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
        g.sort();
        g
    }

    #[test]
    fn faces_that_look_alike_end_up_together_and_different_ones_apart() {
        // three people 100 degrees apart (cosine -0.17), each seen a few times with small variations
        let v: Vec<Vec<f32>> = [0.0, 5.0, -4.0, 100.0, 104.0, 97.0, 200.0, 206.0].iter().map(|d| at(*d)).collect();
        let groups = sorted(upgma(&refs(&v), 0.5, 100));
        assert_eq!(groups, vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7]]);
    }

    #[test]
    fn a_stricter_cut_gives_more_and_smaller_groups() {
        let v: Vec<Vec<f32>> = [0.0, 20.0, 40.0, 60.0].iter().map(|d| at(*d)).collect();
        assert_eq!(upgma(&refs(&v), 0.5, 100).len(), 1); // cos 20 = 0.94 ... cos 60 = 0.5: the average stays above 0.5
        assert_eq!(upgma(&refs(&v), 0.95, 100).len(), 4);
        assert_eq!(upgma(&refs(&v), 0.9, 100).len(), 2);
    }

    #[test]
    fn a_single_face_and_no_faces_are_handled() {
        assert!(upgma(&[], 0.5, 10).is_empty());
        let one = vec![at(0.0)];
        assert_eq!(upgma(&refs(&one), 0.5, 10), vec![vec![0]]);
    }

    #[test]
    fn a_big_library_is_pre_grouped_and_still_comes_out_the_same() {
        // 300 faces: 3 people, 100 faces each, tiny variations; with only 20 units allowed they are pre-grouped
        let mut v: Vec<Vec<f32>> = Vec::new();
        for p in 0..3 {
            for i in 0..100 {
                v.push(at(p as f32 * 120.0 + (i % 7) as f32 * 0.5));
            }
        }
        let exact = sorted(upgma(&refs(&v), 0.5, 1000));
        let pre = sorted(upgma(&refs(&v), 0.5, 20));
        assert_eq!(exact.len(), 3);
        assert_eq!(pre, exact);
    }

    #[test]
    fn a_group_is_mixed_when_many_pairs_look_like_different_people() {
        let same: Vec<Vec<f32>> = (0..20).map(|i| at(i as f32)).collect();
        assert_eq!(mixed_fraction(&refs(&same)), 0.0);
        // half the faces are one person, half another 90 degrees away (cosine ~0): half of all pairs are across
        let blend: Vec<Vec<f32>> = (0..20).map(|i| at(if i % 2 == 0 { 0.0 } else { 90.0 })).collect();
        let m = mixed_fraction(&refs(&blend));
        assert!(m > 0.4 && m < 0.6, "{m}");
        assert!(m > MIXED_ABOVE);
        assert_eq!(mixed_fraction(&refs(&[at(0.0)])), 0.0);
    }

    #[test]
    fn the_spread_shows_different_looking_faces_not_the_best_three() {
        // faces 0 to 4 are one look (and the clearest), 5 and 6 another, 7 a third
        let v: Vec<Vec<f32>> = [0.0, 1.0, 2.0, 3.0, 4.0, 90.0, 92.0, 180.0].iter().map(|d| at(*d)).collect();
        let quality = [0.9, 0.9, 0.9, 0.9, 0.9, 0.5, 0.5, 0.4];
        let picked = spread(&refs(&v), &quality, 3, 100);
        assert_eq!(picked.len(), 3);
        let looks: std::collections::BTreeSet<usize> = picked.iter().map(|&i| if i < 5 { 0 } else if i < 7 { 1 } else { 2 }).collect();
        assert_eq!(looks.len(), 3, "one of each look: {picked:?}");
        // only the clearest pictures are candidates: with a pool of 5 the odd ones out are not shown
        let tight = spread(&refs(&v), &quality, 3, 5);
        assert!(tight.iter().all(|&i| i < 5));
    }

    #[test]
    fn the_nearest_confirmed_person_wins_only_by_a_margin_and_never_a_rejected_one() {
        let joe: Vec<Vec<f32>> = [0.0, 6.0, -5.0].iter().map(|d| at(*d)).collect();
        let mary: Vec<Vec<f32>> = [100.0, 104.0].iter().map(|d| at(*d)).collect();
        let anchors = vec![Anchor { person: 1, faces: refs(&joe) }, Anchor { person: 2, faces: refs(&mary) }];
        let (p, score, margin) = best_person(&at(3.0), &anchors, &[], 3).unwrap();
        assert_eq!(p, 1);
        assert!(score > 0.9 && margin > 0.9);
        assert_eq!(best_person(&at(3.0), &anchors, &[1], 3).unwrap().0, 2); // not the person it was rejected from
        assert!(best_person(&at(3.0), &anchors, &[1, 2], 3).is_none());
        // a face halfway between two people has a small margin
        let (_, _, between) = best_person(&at(52.0), &anchors, &[], 3).unwrap();
        assert!(between < 0.25, "{between}");
        assert!(best_person(&at(3.0), &[], &[], 3).is_none());
    }

    #[test]
    fn a_mixed_group_splits_into_its_people_at_the_first_level_that_works() {
        let mut v: Vec<Vec<f32>> = Vec::new();
        for d in [0.0, 3.0, 6.0, 9.0, 12.0] {
            v.push(at(d)); // one person
        }
        for d in [60.0, 63.0, 66.0, 69.0] {
            v.push(at(d)); // another (cosine to the first ~0.5)
        }
        // at a loose level they are one group; at 0.7 they come apart
        assert_eq!(upgma(&refs(&v), 0.45, 100).len(), 1);
        let pieces = split(&refs(&v), &[0.5, 0.7, 0.9], 3, 100).expect("splits");
        assert_eq!(sorted(pieces), vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8]]);
        assert!(split(&refs(&v[..5]), &[0.5, 0.7, 0.9], 3, 100).is_none()); // one person does not split
    }
}
