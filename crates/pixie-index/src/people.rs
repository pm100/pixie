//! Who is who. Faces are found once; who they belong to is worked out again as the library grows.
//!
//! A person is either **confirmed** (the user named, accepted, split off or hid them) or a **group** that only the
//! computer suggested. A confirmed person's confirmed faces are fixed points: new faces are compared with them and join
//! only when clearly theirs (and wait to be checked). Everything else, the groups and the faces in no group, is
//! regrouped from scratch by `tidy`, so what is learned from later photos can fix what an earlier guess got wrong.
use crate::db::{bump_rev, db_err, prune_people, recompute_person, row_to_tile, vector_of, Db};
use crate::regroup::{self, Anchor, MIXED_ABOVE};
use pixie_core::{FaceOrder, FaceOverview, FacePerson, FaceTile, FaceView, PixieError, Result, Suggestion, TidyReport, UnsortedFace};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// Groups are cut where the average similarity between two clusters falls below this.
pub const GROUP_JOIN: f32 = 0.45;
/// A group needs this many faces; fewer stay in no group (a face seen once or twice is mostly noise).
pub const MIN_GROUP: usize = 3;
/// More faces than this are pre-grouped into units before the exact grouping (see `regroup::upgma`).
pub const MAX_UNITS: usize = 6000;
/// A new face joins a confirmed person on its own only when this alike (mean of its closest faces) ...
/// "Try to group": the lower bar for the faces left over: a lower cut, and two faces from two photos are enough.
pub const LOOSE_JOIN: f32 = 0.40;
const LOOSE_MIN_FACES: usize = 2;
const LOOSE_MIN_PHOTOS: usize = 2;
/// Faces in no group are listed with look-alikes together: clusters this loose, each walked nearest to nearest.
const ORDER_JOIN: f32 = 0.33;
pub const ATTACH_SCORE: f32 = 0.50;
/// ... and at least this much more alike than the next person.
pub const ATTACH_MARGIN: f32 = 0.05;
/// "Looks like Joe" is offered from this score.
pub const SUGGEST_SCORE: f32 = 0.40;
/// How many of a person's closest faces a face is compared with.
const ANCHOR_TOP: usize = 3;
/// At most this many of a person's confirmed faces are compared with (spread evenly).
const ANCHOR_CAP: usize = 200;
/// The share of a person's confirmed faces, the least typical, left out of the comparison: naming a big group vouches
/// for it as a whole, and the odd wrong face in it must not draw other people in.
const ANCHOR_TRIM: f32 = 0.15;
/// A person needs this many faces before any are left out.
const TRIM_FROM: usize = 10;
static TRIM: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(ANCHOR_TRIM.to_bits());

fn trim_share() -> f32 {
    f32::from_bits(TRIM.load(std::sync::atomic::Ordering::Relaxed))
}
/// Tried in turn when sorting out a group that looks like several people.
const SPLIT_LEVELS: [f32; 6] = [0.50, 0.55, 0.60, 0.65, 0.70, 0.75];
/// Faces shown on a card, chosen to show how the person looks (see `regroup::spread`).
const SAMPLES: usize = 6;
const SAMPLE_POOL: usize = 150;
/// How many times a cluster that looks like several people is cut again.
const SETTLE_DEPTH: usize = 3;
/// A regrouping that finds the faces changed under it starts again, this many times.
const TIDY_TRIES: usize = 3;

const VISIBLE: &str = "h.missing = 0 AND h.dup = 0";

/// A condition on a face `f`: it is a candidate to be someone. Not artwork (a painting, drawing or statue is not a person
/// to name), and not ignored by the user.
const NOT_ART: &str = "f.art IS NOT 1 AND f.ignored = 0";

struct OpenFace {
    id: i64,
    photo: i64,
    /// Artwork: never grouped, never attached to anyone.
    art: bool,
    vector: Vec<f32>,
    person: Option<i64>,
}

/// Everything a regrouping needs, read in one go so the work can happen without holding the database.
struct Snapshot {
    rev: i64,
    open: Vec<OpenFace>,
    anchors: Vec<(i64, Vec<Vec<f32>>)>,
    rejections: HashMap<i64, Vec<i64>>,
    groups: Vec<i64>,
    /// The user asked to try harder to group what is left over.
    loose: bool,
}

/// What a regrouping decided.
struct Plan {
    attach: Vec<(i64, i64)>,
    /// (existing group to reuse, faces)
    groups: Vec<(Option<i64>, Vec<i64>)>,
    unsorted: Vec<i64>,
    /// The faces in no group, look-alikes together.
    order: Vec<i64>,
}

fn rev_of(c: &Connection) -> Result<i64> {
    c.query_row("SELECT rev FROM face_meta WHERE id = 1", [], |r| r.get(0)).map_err(db_err)
}

/// The confirmed faces of confirmed people (named ones only when `named_only`), at most `ANCHOR_CAP` each.
fn anchors_of(c: &Connection, named_only: bool) -> Result<Vec<(i64, Vec<Vec<f32>>)>> {
    let mut st = c
        .prepare(&format!(
            "SELECT f.person_id, f.embedding FROM faces f JOIN people p ON p.id = f.person_id
              WHERE p.confirmed = 1 AND f.confirmed = 1 {} ORDER BY f.person_id, f.id",
            if named_only { "AND p.name IS NOT NULL AND p.hidden = 0" } else { "" }
        ))
        .map_err(db_err)?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))).map_err(db_err)?;
    let mut by: Vec<(i64, Vec<Vec<f32>>)> = Vec::new();
    for row in rows {
        let (person, blob) = row.map_err(db_err)?;
        match by.last_mut() {
            Some((p, v)) if *p == person => v.push(vector_of(&blob)),
            _ => by.push((person, vec![vector_of(&blob)])),
        }
    }
    for (_, v) in by.iter_mut() {
        if v.len() >= TRIM_FROM && trim_share() > 0.0 {
            let refs: Vec<&[f32]> = v.iter().map(Vec::as_slice).collect();
            let mean = regroup::centroid(&refs);
            let mut typical: Vec<(f32, usize)> = v.iter().enumerate().map(|(i, f)| (f.iter().zip(&mean).map(|(a, b)| a * b).sum::<f32>(), i)).collect();
            typical.sort_by(|a, b| b.0.total_cmp(&a.0));
            let keep = ((v.len() as f32 * (1.0 - trim_share())).ceil() as usize).max(1);
            let mut keep_ix: Vec<usize> = typical.iter().take(keep).map(|t| t.1).collect();
            keep_ix.sort_unstable();
            *v = keep_ix.into_iter().map(|i| v[i].clone()).collect();
        }
        if v.len() > ANCHOR_CAP {
            let step = v.len() as f32 / ANCHOR_CAP as f32;
            *v = (0..ANCHOR_CAP).map(|i| v[(i as f32 * step) as usize].clone()).collect();
        }
    }
    Ok(by)
}

fn rejections_of(c: &Connection) -> Result<HashMap<i64, Vec<i64>>> {
    let mut st = c.prepare("SELECT face_id, person_id FROM face_rejections").map_err(db_err)?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(db_err)?;
    let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
    for row in rows {
        let (face, person) = row.map_err(db_err)?;
        out.entry(face).or_default().push(person);
    }
    Ok(out)
}

fn anchor_refs(anchors: &[(i64, Vec<Vec<f32>>)]) -> Vec<Anchor<'_>> {
    anchors.iter().map(|(person, faces)| Anchor { person: *person, faces: faces.iter().map(Vec::as_slice).collect() }).collect()
}

/// The person a face clearly belongs to, if any.
fn confident(face: &[f32], anchors: &[Anchor], excluded: &[i64]) -> Option<i64> {
    let (person, score, margin) = regroup::best_person(face, anchors, excluded, ANCHOR_TOP)?;
    (score >= ATTACH_SCORE && margin >= ATTACH_MARGIN).then_some(person)
}

/// A cluster that looks like several people is cut into the pieces it is made of, again for any piece that still looks
/// like several; what is left is taken to be one person each. A cluster that will not come apart stays as it is.
fn settle(vectors: &[&[f32]], clusters: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    let mut done = Vec::new();
    let mut work: Vec<(Vec<usize>, usize)> = clusters.into_iter().map(|c| (c, 0)).collect();
    while let Some((cluster, depth)) = work.pop() {
        if cluster.len() < MIN_GROUP * 2 || depth >= SETTLE_DEPTH {
            done.push(cluster);
            continue;
        }
        let members: Vec<&[f32]> = cluster.iter().map(|&i| vectors[i]).collect();
        if regroup::mixed_fraction(&members) < MIXED_ABOVE {
            done.push(cluster);
            continue;
        }
        match regroup::split(&members, &SPLIT_LEVELS, MIN_GROUP, MAX_UNITS) {
            Some(pieces) => work.extend(pieces.into_iter().map(|p| (p.into_iter().map(|j| cluster[j]).collect(), depth + 1))),
            None => done.push(cluster),
        }
    }
    done.sort_by_key(|c| c.iter().min().copied());
    done
}

/// Decide where every open face goes. Pure: no database.
fn plan(snap: &Snapshot) -> Plan {
    let refs = anchor_refs(&snap.anchors);
    let mut attach = Vec::new();
    let mut rest: Vec<&OpenFace> = Vec::new();
    let mut artwork: Vec<i64> = Vec::new();
    for f in &snap.open {
        if f.art {
            artwork.push(f.id);
            continue;
        }
        let excluded = snap.rejections.get(&f.id).map(Vec::as_slice).unwrap_or(&[]);
        match confident(&f.vector, &refs, excluded) {
            Some(person) => attach.push((f.id, person)),
            None => rest.push(f),
        }
    }
    let vectors: Vec<&[f32]> = rest.iter().map(|f| f.vector.as_slice()).collect();
    let clusters = settle(&vectors, regroup::upgma(&vectors, GROUP_JOIN, MAX_UNITS));
    let mut taken: HashSet<i64> = HashSet::new();
    let (mut groups, mut unsorted) = make_groups(snap, &rest, &clusters, MIN_GROUP, MIN_GROUP, &mut taken);

    // Asked to try harder: what is left over is grouped again with a lower bar (two photos are enough).
    if snap.loose && !unsorted.is_empty() {
        let by_id: HashMap<i64, &OpenFace> = rest.iter().map(|f| (f.id, *f)).collect();
        let left: Vec<&OpenFace> = unsorted.iter().filter_map(|id| by_id.get(id).copied()).collect();
        let vectors: Vec<&[f32]> = left.iter().map(|f| f.vector.as_slice()).collect();
        let clusters = settle(&vectors, regroup::upgma(&vectors, LOOSE_JOIN, MAX_UNITS));
        let (more, still) = make_groups(snap, &left, &clusters, LOOSE_MIN_FACES, LOOSE_MIN_PHOTOS, &mut taken);
        groups.extend(more);
        unsorted = still;
    }

    // Faces in no group are listed with look-alikes together.
    let by_id: HashMap<i64, &OpenFace> = snap.open.iter().map(|f| (f.id, f)).collect();
    let loose: Vec<&OpenFace> = unsorted.iter().filter_map(|id| by_id.get(id).copied()).collect();
    let vectors: Vec<&[f32]> = loose.iter().map(|f| f.vector.as_slice()).collect();
    let order = regroup::similarity_order(&vectors, ORDER_JOIN, MAX_UNITS).into_iter().map(|i| loose[i].id).collect();
    unsorted.extend(artwork); // out of any group, and not listed in Unsure (see `NOT_ART`)
    Plan { attach, groups, unsorted, order }
}

/// Turn clusters into groups (a group needs `min_faces` faces from `min_photos` different photos: two faces in one photo
/// are two people, so they are no evidence that one person recurs) and the faces left over. A cluster keeps the id of the
/// old group most of its faces came from (not one in `taken`), so a group is the same group tomorrow.
fn make_groups(
    snap: &Snapshot,
    rest: &[&OpenFace],
    clusters: &[Vec<usize>],
    min_faces: usize,
    min_photos: usize,
    taken: &mut HashSet<i64>,
) -> (Vec<(Option<i64>, Vec<i64>)>, Vec<i64>) {
    let mut overlaps: Vec<(usize, usize, i64)> = Vec::new();
    for (ci, cluster) in clusters.iter().enumerate() {
        let mut counts: HashMap<i64, usize> = HashMap::new();
        for &m in cluster {
            if let Some(old) = rest[m].person {
                *counts.entry(old).or_default() += 1;
            }
        }
        overlaps.extend(counts.into_iter().map(|(old, n)| (n, ci, old)));
    }
    overlaps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut reuse: HashMap<usize, i64> = HashMap::new();
    for (n, ci, old) in overlaps {
        let big_enough = {
            let photos: HashSet<i64> = clusters[ci].iter().map(|&m| rest[m].photo).collect();
            clusters[ci].len() >= min_faces && photos.len() >= min_photos
        };
        if big_enough && n * 2 >= clusters[ci].len() && !reuse.contains_key(&ci) && !taken.contains(&old) {
            reuse.insert(ci, old);
            taken.insert(old);
        }
    }

    let mut groups = Vec::new();
    let mut unsorted = Vec::new();
    for (ci, cluster) in clusters.iter().enumerate() {
        let photos: HashSet<i64> = cluster.iter().map(|&m| rest[m].photo).collect();
        if cluster.len() < min_faces || photos.len() < min_photos {
            unsorted.extend(cluster.iter().map(|&m| rest[m].id));
            continue;
        }
        let id = reuse.get(&ci).copied();
        let mut members = Vec::new();
        for &m in cluster {
            let face = rest[m].id;
            // "Move to Unsure" on a group keeps the face out of it
            if id.is_some_and(|g| snap.rejections.get(&face).is_some_and(|r| r.contains(&g))) {
                unsorted.push(face);
            } else {
                members.push(face);
            }
        }
        groups.push((id, members));
    }
    (groups, unsorted)
}

fn same_name(c: &Connection, name: &str, except: i64) -> Result<Option<i64>> {
    c.query_row("SELECT id FROM people WHERE name = ?1 COLLATE NOCASE AND id <> ?2 ORDER BY id LIMIT 1", params![name, except], |r| r.get(0)).optional().map_err(db_err)
}

fn ids_sql(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

/// Fold `from` into `into`: its faces move (confirmed as `into` is), its rejections follow, and `from` goes.
fn merge_in(tx: &Connection, from: i64, into: i64) -> Result<()> {
    if from == into {
        return Ok(());
    }
    let confirmed: i64 = tx.query_row("SELECT confirmed FROM people WHERE id = ?1", params![into], |r| r.get(0)).map_err(db_err)?;
    tx.execute("UPDATE faces SET person_id = ?2, confirmed = ?3 WHERE person_id = ?1", params![from, into, confirmed]).map_err(db_err)?;
    tx.execute("UPDATE OR IGNORE face_rejections SET person_id = ?2 WHERE person_id = ?1", params![from, into]).map_err(db_err)?;
    tx.execute("DELETE FROM face_rejections WHERE person_id = ?1", params![from]).map_err(db_err)?;
    tx.execute("DELETE FROM face_rejections WHERE person_id = ?2 AND face_id IN (SELECT id FROM faces WHERE person_id = ?2)", params![from, into]).map_err(db_err)?;
    tx.execute("DELETE FROM people WHERE id = ?1", params![from]).map_err(db_err)?;
    recompute_person(tx, into)
}

/// Make a person confirmed, with all the faces they have now.
fn confirm_in(tx: &Connection, id: i64) -> Result<()> {
    tx.execute("UPDATE people SET confirmed = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
    tx.execute("UPDATE faces SET confirmed = 1 WHERE person_id = ?1", params![id]).map_err(db_err)?;
    Ok(())
}

fn new_person_in(tx: &Connection, seed_face: i64) -> Result<i64> {
    let embedding: Vec<u8> = tx.query_row("SELECT embedding FROM faces WHERE id = ?1", params![seed_face], |r| r.get(0)).map_err(db_err)?;
    tx.execute("INSERT INTO people(centroid, n) VALUES (?1, 0)", params![embedding]).map_err(db_err)?;
    Ok(tx.last_insert_rowid())
}

impl Db {
    /// Changes whenever who-is-who changes, so a screen can tell its picture is out of date.
    pub fn face_rev(&self) -> Result<i64> {
        rev_of(&self.conn())
    }

    /// How many faces each part of the People tab holds.
    pub fn face_overview(&self) -> Result<FaceOverview> {
        let c = self.conn();
        let count = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(db_err);
        Ok(FaceOverview {
            groups: count("SELECT COUNT(*) FROM people WHERE hidden = 0 AND name IS NULL")?,
            people: count("SELECT COUNT(*) FROM people WHERE hidden = 0 AND name IS NOT NULL")?,
            unsorted: count(&format!("SELECT COUNT(*) FROM faces f JOIN photos h ON h.id = f.photo_id WHERE f.person_id IS NULL AND {VISIBLE} AND {NOT_ART}"))?,
            hidden: count("SELECT COUNT(*) FROM people WHERE hidden = 1")?
                + count(&format!("SELECT COUNT(*) FROM faces f JOIN photos h ON h.id = f.photo_id WHERE f.ignored = 1 AND {VISIBLE}"))?,
            unchecked: count(&format!(
                "SELECT COUNT(*) FROM faces f JOIN people p ON p.id = f.person_id JOIN photos h ON h.id = f.photo_id
                  WHERE p.confirmed = 1 AND p.hidden = 0 AND f.confirmed = 0 AND {VISIBLE}"
            ))?,
            rev: rev_of(&c)?,
        })
    }

    // ---- joining and regrouping ----

    /// New faces that are clearly a confirmed person's join them, to be checked. `only` limits it to those faces
    /// (the ones just found); None looks at every face in no group. Returns how many joined.
    pub fn attach_confident(&self, only: Option<&[i64]>) -> Result<usize> {
        let (faces, anchors, rejections) = {
            let c = self.conn();
            let anchors = anchors_of(&c, false)?;
            if anchors.is_empty() {
                return Ok(0);
            }
            let sql = match only {
                Some(ids) if ids.is_empty() => return Ok(0),
                Some(ids) => format!("SELECT id, embedding FROM faces WHERE person_id IS NULL AND art IS NOT 1 AND ignored = 0 AND id IN ({})", ids_sql(ids)),
                None => "SELECT id, embedding FROM faces WHERE person_id IS NULL AND art IS NOT 1 AND ignored = 0".to_string(),
            };
            let mut st = c.prepare(&sql).map_err(db_err)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, vector_of(&r.get::<_, Vec<u8>>(1)?)))).map_err(db_err)?;
            let faces = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)?;
            (faces, anchors, rejections_of(&c)?)
        };
        let refs = anchor_refs(&anchors);
        let joins: Vec<(i64, i64)> = faces
            .iter()
            .filter_map(|(id, v)| confident(v, &refs, rejections.get(id).map(Vec::as_slice).unwrap_or(&[])).map(|p| (*id, p)))
            .collect();
        if joins.is_empty() {
            return Ok(0);
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let mut touched = HashSet::new();
        let mut joined = 0;
        for (face, person) in joins {
            // a person may have gone while this was thinking
            let n = tx
                .execute(
                    "UPDATE faces SET person_id = ?2, confirmed = 0 WHERE id = ?1 AND person_id IS NULL AND EXISTS (SELECT 1 FROM people WHERE id = ?2)",
                    params![face, person],
                )
                .map_err(db_err)?;
            if n > 0 {
                touched.insert(person);
                joined += n;
            }
        }
        for p in &touched {
            recompute_person(&tx, *p)?;
        }
        if joined > 0 {
            bump_rev(&tx)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(joined)
    }

    /// Faces in no group or in an unconfirmed group, with what they could join.
    fn snapshot(&self) -> Result<Snapshot> {
        let c = self.conn();
        let rev = rev_of(&c)?;
        let mut st = c
            .prepare(&format!(
                "SELECT f.id, f.embedding, f.person_id, f.photo_id, f.art IS 1 FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE}
                   LEFT JOIN people p ON p.id = f.person_id
                  WHERE f.ignored = 0 AND (f.person_id IS NULL OR p.confirmed = 0) ORDER BY f.id"
            ))
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok(OpenFace { id: r.get(0)?, vector: vector_of(&r.get::<_, Vec<u8>>(1)?), person: r.get(2)?, photo: r.get(3)?, art: r.get::<_, i64>(4)? != 0 })).map_err(db_err)?;
        let open = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)?;
        let groups = {
            let mut st = c.prepare("SELECT id FROM people WHERE confirmed = 0").map_err(db_err)?;
            let rows = st.query_map([], |r| r.get(0)).map_err(db_err)?;
            rows.collect::<rusqlite::Result<Vec<i64>>>().map_err(db_err)?
        };
        Ok(Snapshot {
            rev,
            open,
            anchors: anchors_of(&c, false)?,
            rejections: rejections_of(&c)?,
            groups,
            loose: c.query_row("SELECT loose FROM face_meta WHERE id = 1", [], |r| r.get::<_, i64>(0)).map_err(db_err)? != 0,
        })
    }

    /// Regroup every face the user has not settled: those clearly a confirmed person's join them (to be checked), the
    /// rest are grouped by how alike they are, and what is too few to be a group stays in no group. Groups keep their
    /// identity from one pass to the next. Returns None when the faces kept changing under it (try again later).
    pub fn tidy(&self) -> Result<Option<TidyReport>> {
        for _ in 0..TIDY_TRIES {
            let snap = self.snapshot()?;
            let plan = plan(&snap);
            if let Some(report) = self.apply(&snap, plan)? {
                return Ok(Some(report));
            }
        }
        Ok(None)
    }

    fn apply(&self, snap: &Snapshot, plan: Plan) -> Result<Option<TidyReport>> {
        let old: HashMap<i64, Option<i64>> = snap.open.iter().map(|f| (f.id, f.person)).collect();
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        if rev_of(&tx)? != snap.rev {
            return Ok(None); // the user (or a rescan) changed who-is-who while this was thinking
        }
        let mut touched: HashSet<i64> = HashSet::new();
        let mut changes = 0usize;
        for (face, person) in &plan.attach {
            changes += tx.execute("UPDATE faces SET person_id = ?2, confirmed = 0 WHERE id = ?1", params![face, person]).map_err(db_err)?;
            touched.insert(*person);
        }
        let mut kept: HashSet<i64> = HashSet::new();
        for (id, members) in &plan.groups {
            let id = match id {
                Some(id) => *id,
                None => {
                    changes += 1;
                    new_person_in(&tx, members[0])?
                }
            };
            kept.insert(id);
            for face in members {
                if old.get(face) != Some(&Some(id)) {
                    changes += tx.execute("UPDATE faces SET person_id = ?2, confirmed = 0 WHERE id = ?1", params![face, id]).map_err(db_err)?;
                }
            }
            touched.insert(id);
        }
        for face in &plan.unsorted {
            if old.get(face).copied().flatten().is_some() {
                changes += tx.execute("UPDATE faces SET person_id = NULL, confirmed = 0 WHERE id = ?1", params![face]).map_err(db_err)?;
            }
        }
        for (rank, face) in plan.order.iter().enumerate() {
            tx.execute("UPDATE faces SET unsure_rank = ?2 WHERE id = ?1", params![face, rank as i64]).map_err(db_err)?;
        }
        // groups nobody kept (their faces are placed above)
        for g in snap.groups.iter().filter(|g| !kept.contains(g)) {
            changes += tx.execute("DELETE FROM people WHERE id = ?1 AND confirmed = 0", params![g]).map_err(db_err)?;
        }
        if changes > 0 {
            for p in &touched {
                recompute_person(&tx, *p)?;
            }
            prune_people(&tx)?;
            bump_rev(&tx)?; // a pass that changed nothing leaves what is on screen current
        }
        tx.commit().map_err(db_err)?;
        Ok(Some(TidyReport { faces: snap.open.len(), groups: plan.groups.len(), unsorted: plan.unsorted.len(), attached: plan.attach.len() }))
    }

    /// "Try to group": regroup, grouping what is left over more loosely (and keep doing so on later regroupings).
    /// Returns how many faces left Unsure.
    pub fn try_group(&self) -> Result<i64> {
        let before = self.face_overview()?.unsorted;
        self.conn().execute("UPDATE face_meta SET loose = 1", []).map_err(db_err)?;
        self.tidy()?;
        Ok(before - self.face_overview()?.unsorted)
    }

    /// Who this face (or these faces, or this group's faces) looks most like among the named people: up to three, most
    /// alike first, each at least `SUGGEST_SCORE`. `except` is a person that cannot be the answer (the group asking).
    fn suggest(&self, face_ids: &[i64], except: Option<i64>) -> Result<Vec<Suggestion>> {
        let c = self.conn();
        // the faces' average look (at most 300 of them, spread evenly)
        let step = (face_ids.len() / 300).max(1);
        let ids: Vec<i64> = face_ids.iter().copied().step_by(step).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut st = c.prepare(&format!("SELECT embedding FROM faces WHERE id IN ({})", ids_sql(&ids))).map_err(db_err)?;
        let rows = st.query_map([], |r| r.get::<_, Vec<u8>>(0)).map_err(db_err)?;
        let vectors: Vec<Vec<f32>> = rows.map(|r| r.map(|b| vector_of(&b))).collect::<rusqlite::Result<_>>().map_err(db_err)?;
        let refs: Vec<&[f32]> = vectors.iter().map(Vec::as_slice).collect();
        if refs.is_empty() {
            return Ok(Vec::new());
        }
        let mean = regroup::centroid(&refs);
        let anchors = anchors_of(&c, true)?;
        let ranked = regroup::rank_people(&mean, &anchor_refs(&anchors), &except.into_iter().collect::<Vec<_>>(), ANCHOR_TOP);
        let mut out = Vec::new();
        for (person, score) in ranked.into_iter().filter(|r| r.1 >= SUGGEST_SCORE).take(3) {
            let name: Option<String> = c.query_row("SELECT name FROM people WHERE id = ?1", params![person], |r| r.get(0)).optional().map_err(db_err)?.flatten();
            out.push(Suggestion { person_id: person, name, score });
        }
        Ok(out)
    }

    /// "This is…" candidates for some faces.
    pub fn suggest_for_faces(&self, face_ids: &[i64]) -> Result<Vec<Suggestion>> {
        self.suggest(face_ids, None)
    }

    /// "This is…" candidates for a whole group (or person).
    pub fn suggest_for_group(&self, id: i64) -> Result<Vec<Suggestion>> {
        let ids: Vec<i64> = {
            let c = self.conn();
            let mut st = c.prepare("SELECT id FROM faces WHERE person_id = ?1 ORDER BY id").map_err(db_err)?;
            let rows = st.query_map(params![id], |r| r.get(0)).map_err(db_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(db_err)?
        };
        self.suggest(&ids, Some(id))
    }

    /// Ignore these faces, each on its own: they leave whatever group they were in and are left out of grouping and of every
    /// list but "Ignored".
    pub fn ignore_faces(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let list = ids_sql(ids);
        let before = people_of_faces(&tx, &list)?;
        tx.execute(&format!("UPDATE faces SET ignored = 1, person_id = NULL, confirmed = 0 WHERE id IN ({list})"), []).map_err(db_err)?;
        for p in before {
            recompute_person(&tx, p)?;
        }
        prune_people(&tx)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Stop ignoring these faces: they are loose again (in Unsure) until they are grouped or named.
    pub fn unignore_faces(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute(&format!("UPDATE faces SET ignored = 0 WHERE id IN ({})", ids_sql(ids)), []).map_err(db_err)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// The faces the user ignored, in visible photos.
    pub fn ignored_faces(&self, offset: i64, limit: i64) -> Result<Vec<FaceTile>> {
        let c = self.conn();
        let mut st = c
            .prepare(&format!(
                "SELECT f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE}
                  WHERE f.ignored = 1 ORDER BY f.id DESC LIMIT ?1 OFFSET ?2"
            ))
            .map_err(db_err)?;
        let rows = st.query_map(params![limit, offset], row_to_tile).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    // ---- what the user can do ----

    /// Name a person (and so confirm them: their faces become the fixed points new faces are compared with). `None`
    /// or a blank name clears the name. A name another person has means the same person: the two become one, and the
    /// id kept is returned.
    pub fn name_person(&self, id: i64, name: Option<&str>) -> Result<i64> {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let mut kept = id;
        match name {
            Some(n) => {
                if let Some(other) = same_name(&tx, n, id)? {
                    merge_in(&tx, id, other)?;
                    kept = other;
                } else {
                    tx.execute("UPDATE people SET name = ?2 WHERE id = ?1", params![id, n]).map_err(db_err)?;
                }
                confirm_in(&tx, kept)?;
            }
            None => {
                tx.execute("UPDATE people SET name = NULL WHERE id = ?1", params![id]).map_err(db_err)?;
            }
        }
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(kept)
    }

    /// Say a group is one person without naming them yet.
    pub fn confirm_person(&self, id: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        confirm_in(&tx, id)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// "This is the same person as ...": `from` becomes part of `into`.
    pub fn assign_group(&self, from: i64, into: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        merge_in(&tx, from, into)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Say these faces are who they are filed under.
    pub fn confirm_faces(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute(&format!("UPDATE faces SET confirmed = 1 WHERE person_id IS NOT NULL AND id IN ({})", ids_sql(ids)), []).map_err(db_err)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Give faces to a person (from another person, a group, or no group). They count as the user's say-so when the
    /// person is confirmed.
    pub fn move_faces(&self, ids: &[i64], person: i64) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let confirmed: i64 = tx.query_row("SELECT confirmed FROM people WHERE id = ?1", params![person], |r| r.get(0)).map_err(db_err)?;
        let list = ids_sql(ids);
        let before = people_of_faces(&tx, &list)?;
        tx.execute(&format!("UPDATE faces SET person_id = ?1, confirmed = ?2, ignored = 0 WHERE id IN ({list})"), params![person, confirmed]).map_err(db_err)?;
        tx.execute(&format!("DELETE FROM face_rejections WHERE person_id = ?1 AND face_id IN ({list})"), params![person]).map_err(db_err)?;
        for p in before.into_iter().chain([person]) {
            recompute_person(&tx, p)?;
        }
        prune_people(&tx)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Start a new person from these faces and return their id. The user has set them apart, so they are confirmed
    /// (named or not) and stay apart; a name someone has already means these are that person's faces.
    pub fn new_person_from_faces(&self, ids: &[i64], name: Option<&str>) -> Result<i64> {
        let first = *ids.first().ok_or_else(|| PixieError::Db("no faces to start a person from".into()))?;
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let list = ids_sql(ids);
        let before = people_of_faces(&tx, &list)?;
        let existing = match name {
            Some(n) => same_name(&tx, n, -1)?,
            None => None,
        };
        let person = match existing {
            Some(p) => p,
            None => {
                let p = new_person_in(&tx, first)?;
                tx.execute("UPDATE people SET name = ?2, confirmed = 1 WHERE id = ?1", params![p, name]).map_err(db_err)?;
                p
            }
        };
        let confirmed: i64 = tx.query_row("SELECT confirmed FROM people WHERE id = ?1", params![person], |r| r.get(0)).map_err(db_err)?;
        tx.execute(&format!("UPDATE faces SET person_id = ?1, confirmed = ?2, ignored = 0 WHERE id IN ({list})"), params![person, confirmed]).map_err(db_err)?;
        tx.execute(&format!("DELETE FROM face_rejections WHERE person_id = ?1 AND face_id IN ({list})"), params![person]).map_err(db_err)?;
        for p in before.into_iter().chain([person]) {
            recompute_person(&tx, p)?;
        }
        prune_people(&tx)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(person)
    }

    /// "Not this person": the faces leave who they were filed under, are remembered as not theirs, and wait in no
    /// group (nothing guesses them back).
    pub fn reject_faces(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let list = ids_sql(ids);
        tx.execute(
            &format!("INSERT OR IGNORE INTO face_rejections(face_id, person_id) SELECT id, person_id FROM faces WHERE person_id IS NOT NULL AND id IN ({list})"),
            [],
        )
        .map_err(db_err)?;
        let before = people_of_faces(&tx, &list)?;
        tx.execute(&format!("UPDATE faces SET person_id = NULL, confirmed = 0 WHERE id IN ({list})"), []).map_err(db_err)?;
        for p in before {
            recompute_person(&tx, p)?;
        }
        prune_people(&tx)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Leave a person out of the nav and the lists. Their faces stay theirs (and stay fixed points).
    pub fn hide_person(&self, id: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("UPDATE people SET hidden = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        confirm_in(&tx, id)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Bring a hidden person back. One nobody named goes back to being a group to look at.
    pub fn unhide_person(&self, id: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("UPDATE people SET hidden = 0 WHERE id = ?1", params![id]).map_err(db_err)?;
        tx.execute("UPDATE people SET confirmed = 0 WHERE id = ?1 AND name IS NULL", params![id]).map_err(db_err)?;
        tx.execute("UPDATE faces SET confirmed = 0 WHERE person_id = ?1 AND (SELECT confirmed FROM people WHERE id = ?1) = 0", params![id]).map_err(db_err)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Sort out a group that looks like several people: it is cut into pieces (the biggest keeps the group), each of
    /// which is a person to name. Faces left over in pieces too small to be a group wait in no group. Returns how many
    /// pieces there are, 0 when the group does not come apart (or is already confirmed).
    pub fn split_group(&self, id: i64) -> Result<usize> {
        let faces: Vec<(i64, Vec<f32>)> = {
            let c = self.conn();
            let confirmed: i64 = c.query_row("SELECT confirmed FROM people WHERE id = ?1", params![id], |r| r.get(0)).map_err(db_err)?;
            if confirmed == 1 {
                return Ok(0);
            }
            let mut st = c.prepare("SELECT id, embedding FROM faces WHERE person_id = ?1 ORDER BY id").map_err(db_err)?;
            let rows = st.query_map(params![id], |r| Ok((r.get::<_, i64>(0)?, vector_of(&r.get::<_, Vec<u8>>(1)?)))).map_err(db_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(db_err)?
        };
        let vectors: Vec<&[f32]> = faces.iter().map(|(_, v)| v.as_slice()).collect();
        let Some(mut pieces) = regroup::split(&vectors, &SPLIT_LEVELS, MIN_GROUP, MAX_UNITS) else {
            return Ok(0);
        };
        pieces.sort_by_key(|p| std::cmp::Reverse(p.len()));
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let mut count = 0;
        for (i, piece) in pieces.iter().enumerate() {
            let ids: Vec<i64> = piece.iter().map(|&m| faces[m].0).collect();
            if piece.len() < MIN_GROUP {
                tx.execute(&format!("UPDATE faces SET person_id = NULL, confirmed = 0 WHERE id IN ({})", ids_sql(&ids)), []).map_err(db_err)?;
                continue;
            }
            // each piece is something the user has seen and set apart, so it stays apart: it is confirmed
            let person = if i == 0 { id } else { new_person_in(&tx, ids[0])? };
            tx.execute(&format!("UPDATE faces SET person_id = ?1 WHERE id IN ({})", ids_sql(&ids)), params![person]).map_err(db_err)?;
            confirm_in(&tx, person)?;
            recompute_person(&tx, person)?;
            count += 1;
        }
        prune_people(&tx)?;
        bump_rev(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(count)
    }

    // ---- what the People tab shows ----

    /// The cards for one section of the People tab. Groups come biggest first (most photos); each card has faces chosen to show how the person looks, and (for a group) who it looks like.
    pub fn list_face_people(&self, view: FaceView) -> Result<Vec<FacePerson>> {
        let cond = match view {
            FaceView::Groups => "p.hidden = 0 AND p.name IS NULL",
            FaceView::People => "p.hidden = 0 AND p.name IS NOT NULL",
            FaceView::Hidden => "p.hidden = 1",
        };
        let c = self.conn();
        let mut st = c
            .prepare(&format!(
                "SELECT p.id, p.name, p.hidden, p.confirmed, p.mixed, p.centroid, COUNT(DISTINCT f.photo_id), COUNT(f.id),
                        COALESCE(SUM(p.confirmed = 1 AND f.confirmed = 0), 0)
                   FROM people p JOIN faces f ON f.person_id = p.id JOIN photos h ON h.id = f.photo_id AND {VISIBLE}
                  WHERE {cond} GROUP BY p.id"
            ))
            .map_err(db_err)?;
        struct Row {
            card: FacePerson,
            centroid: Vec<f32>,
        }
        let rows = st
            .query_map([], |r| {
                let mixed: Option<f32> = r.get(4)?;
                Ok(Row {
                    card: FacePerson {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        hidden: r.get::<_, i64>(2)? != 0,
                        confirmed: r.get::<_, i64>(3)? != 0,
                        photos: r.get(6)?,
                        faces: r.get(7)?,
                        unchecked: r.get(8)?,
                        mixed,
                        looks_mixed: mixed.is_some_and(|m| m >= MIXED_ABOVE),
                        samples: Vec::new(),
                        suggestion: None,
                    },
                    centroid: vector_of(&r.get::<_, Vec<u8>>(5)?),
                })
            })
            .map_err(db_err)?;
        let mut cards = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)?;
        match view {
            FaceView::Groups => cards.sort_by(|a, b| b.card.photos.cmp(&a.card.photos).then(a.card.id.cmp(&b.card.id))),
            FaceView::People => cards.sort_by(|a, b| {
                (b.card.unchecked > 0).cmp(&(a.card.unchecked > 0)).then_with(|| a.card.name.as_deref().unwrap_or("").to_lowercase().cmp(&b.card.name.as_deref().unwrap_or("").to_lowercase())).then(a.card.id.cmp(&b.card.id))
            }),
            FaceView::Hidden => cards.sort_by(|a, b| b.card.faces.cmp(&a.card.faces).then(a.card.id.cmp(&b.card.id))),
        }

        let named = if view == FaceView::Groups { anchors_of(&c, true)? } else { Vec::new() };
        let names: HashMap<i64, Option<String>> = if named.is_empty() {
            HashMap::new()
        } else {
            let mut st = c.prepare("SELECT id, name FROM people WHERE name IS NOT NULL").map_err(db_err)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))).map_err(db_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(db_err)?
        };
        let refs = anchor_refs(&named);

        let mut faces = c
            .prepare(&format!(
                "SELECT f.id, f.embedding, f.score * f.w * f.h FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE} WHERE f.person_id = ?1 ORDER BY f.id"
            ))
            .map_err(db_err)?;
        let mut out = Vec::with_capacity(cards.len());
        for Row { mut card, centroid } in cards {
            let rows: Vec<(i64, Vec<f32>, f32)> = faces
                .query_map(params![card.id], |r| Ok((r.get(0)?, vector_of(&r.get::<_, Vec<u8>>(1)?), r.get(2)?)))
                .map_err(db_err)?
                .collect::<rusqlite::Result<_>>()
                .map_err(db_err)?;
            let vectors: Vec<&[f32]> = rows.iter().map(|(_, v, _)| v.as_slice()).collect();
            let quality: Vec<f32> = rows.iter().map(|(_, _, q)| *q).collect();
            card.samples = regroup::spread(&vectors, &quality, SAMPLES, SAMPLE_POOL).into_iter().map(|i| rows[i].0).collect();
            if !refs.is_empty() {
                card.suggestion = regroup::best_person(&centroid, &refs, &[card.id], ANCHOR_TOP)
                    .filter(|(_, score, _)| *score >= SUGGEST_SCORE)
                    .map(|(person, score, _)| Suggestion { person_id: person, name: names.get(&person).cloned().flatten(), score });
            }
            out.push(card);
        }
        Ok(out)
    }

    /// A person's faces in visible photos, `limit` from `offset`.
    pub fn faces_of_person(&self, person: i64, offset: i64, limit: i64, order: FaceOrder) -> Result<Vec<FaceTile>> {
        let c = self.conn();
        let cols = "f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h";
        if order == FaceOrder::Best {
            let mut st = c
                .prepare(&format!(
                    "SELECT {cols} FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE}
                      WHERE f.person_id = ?1 ORDER BY f.score * f.w * f.h DESC, f.id LIMIT ?2 OFFSET ?3"
                ))
                .map_err(db_err)?;
            let rows = st.query_map(params![person, limit, offset], row_to_tile).map_err(db_err)?;
            return rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err);
        }
        let centroid: Vec<f32> = c
            .query_row("SELECT centroid FROM people WHERE id = ?1", params![person], |r| r.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(db_err)?
            .map(|b| vector_of(&b))
            .unwrap_or_default();
        let only_unchecked = if order == FaceOrder::Unchecked { "AND f.confirmed = 0" } else { "" };
        let mut st = c
            .prepare(&format!(
                "SELECT {cols}, f.embedding FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE} WHERE f.person_id = ?1 {only_unchecked}"
            ))
            .map_err(db_err)?;
        let mut rows = st
            .query_map(params![person], |r| {
                let sim: f32 = vector_of(&r.get::<_, Vec<u8>>(8)?).iter().zip(&centroid).map(|(a, b)| a * b).sum();
                Ok((sim, row_to_tile(r)?))
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        rows.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        Ok(rows.into_iter().skip(offset.max(0) as usize).take(limit.max(0) as usize).map(|(_, t)| t).collect())
    }

    /// Faces in no group, the clearest first, each with who it looks like.
    pub fn unsorted_faces(&self, offset: i64, limit: i64) -> Result<Vec<UnsortedFace>> {
        let c = self.conn();
        let mut st = c
            .prepare(&format!(
                "SELECT f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h, f.embedding FROM faces f JOIN photos h ON h.id = f.photo_id AND {VISIBLE}
                  WHERE f.person_id IS NULL AND {NOT_ART} ORDER BY f.unsure_rank IS NULL, f.unsure_rank, f.score * f.w * f.h DESC, f.id LIMIT ?1 OFFSET ?2"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map(params![limit, offset], |r| Ok((row_to_tile(r)?, vector_of(&r.get::<_, Vec<u8>>(8)?))))
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        let anchors = anchors_of(&c, true)?;
        let refs = anchor_refs(&anchors);
        let rejections = rejections_of(&c)?;
        let names: HashMap<i64, Option<String>> = {
            let mut st = c.prepare("SELECT id, name FROM people WHERE name IS NOT NULL").map_err(db_err)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))).map_err(db_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(db_err)?
        };
        Ok(rows
            .into_iter()
            .map(|(tile, v)| {
                let excluded = rejections.get(&tile.id).map(Vec::as_slice).unwrap_or(&[]);
                let suggestion = regroup::best_person(&v, &refs, excluded, ANCHOR_TOP)
                    .filter(|(_, score, _)| *score >= SUGGEST_SCORE)
                    .map(|(person, score, _)| Suggestion { person_id: person, name: names.get(&person).cloned().flatten(), score });
                UnsortedFace { tile, suggestion }
            })
            .collect())
    }
}

/// The people the faces in the comma-separated list belong to now.
fn people_of_faces(tx: &Connection, list: &str) -> Result<Vec<i64>> {
    let mut st = tx.prepare(&format!("SELECT DISTINCT person_id FROM faces WHERE person_id IS NOT NULL AND id IN ({list})")).map_err(db_err)?;
    let rows = st.query_map([], |r| r.get(0)).map_err(db_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{FolderId, NewFace, PhotoId, PhotoMeta};

    fn meta() -> PhotoMeta {
        PhotoMeta { taken_at: None, ..Default::default() }
    }

    /// A unit vector at `deg` degrees: faces of one person are close in angle, different people far apart.
    fn at(deg: f32) -> Vec<f32> {
        let r = deg.to_radians();
        vec![r.cos(), r.sin()]
    }

    fn nf(deg: f32) -> NewFace {
        NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score: 0.9, embedding: at(deg) }
    }

    /// One photo per angle, each with one face; returns the face ids in order.
    fn library(db: &Db, angles: &[f32]) -> Vec<i64> {
        let f: FolderId = db.add_folder("/p").unwrap();
        angles
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let p: PhotoId = db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &meta()).unwrap();
                db.replace_faces(p, "fv", &[nf(*a)]).unwrap()[0]
            })
            .collect()
    }

    fn person_of(db: &Db, face: i64) -> Option<i64> {
        db.conn().query_row("SELECT person_id FROM faces WHERE id = ?1", params![face], |r| r.get(0)).unwrap()
    }

    fn confirmed_of(db: &Db, face: i64) -> bool {
        db.conn().query_row("SELECT confirmed FROM faces WHERE id = ?1", params![face], |r| r.get::<_, i64>(0)).unwrap() == 1
    }

    // two people 90 degrees apart (cosine 0), a pair that is too small to be a group, and one face on its own
    const ANGLES: [f32; 10] = [0.0, 3.0, 6.0, 90.0, 93.0, 96.0, 99.0, 180.0, 183.0, 270.0];

    #[test]
    fn tidy_makes_groups_of_three_or_more_and_leaves_the_rest_unsorted() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        let r = db.tidy().unwrap().unwrap();
        assert_eq!((r.groups, r.unsorted, r.attached), (2, 3, 0));
        assert_eq!(person_of(&db, ids[0]), person_of(&db, ids[2]));
        assert_eq!(person_of(&db, ids[3]), person_of(&db, ids[6]));
        assert_ne!(person_of(&db, ids[0]), person_of(&db, ids[3]));
        assert!(ids[7..].iter().all(|f| person_of(&db, *f).is_none()), "a pair and a single are in no group");
        let o = db.face_overview().unwrap();
        assert_eq!((o.groups, o.unsorted, o.people), (2, 3, 0));
    }

    #[test]
    fn faces_that_are_all_in_one_photo_do_not_make_a_group() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let crowd = db.upsert_photo(f, "/p/crowd.jpg", 1, 1, &meta()).unwrap();
        db.replace_faces(crowd, "fv", &[nf(0.0), nf(2.0), nf(4.0), nf(6.0)]).unwrap();
        let r = db.tidy().unwrap().unwrap();
        assert_eq!((r.groups, r.unsorted), (0, 4));
        // the same faces in three different photos are a person
        for (i, a) in [8.0f32, 10.0].iter().enumerate() {
            let p = db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &meta()).unwrap();
            db.replace_faces(p, "fv", &[nf(*a)]).unwrap();
        }
        let r = db.tidy().unwrap().unwrap();
        assert_eq!((r.groups, r.unsorted), (1, 0), "three photos now, so a group");
    }

    #[test]
    fn try_group_groups_the_leftovers_more_loosely_and_keeps_doing_so() {
        let db = Db::open_in_memory().unwrap();
        // two people each in only two photos (the strict bar is three), and one face on its own
        let ids = library(&db, &[0.0, 8.0, 100.0, 108.0, 250.0]);
        let r = db.tidy().unwrap().unwrap();
        assert_eq!((r.groups, r.unsorted), (0, 5));
        assert_eq!(db.try_group().unwrap(), 4, "two pairs left Unsure; the single face stays");
        assert_eq!(db.face_overview().unwrap().groups, 2);
        assert_eq!(person_of(&db, ids[0]), person_of(&db, ids[1]));
        assert_ne!(person_of(&db, ids[0]), person_of(&db, ids[2]));
        assert_eq!(person_of(&db, ids[4]), None);
        // later regroupings keep the looser bar, so the groups are not dissolved again
        db.tidy().unwrap();
        assert_eq!(db.face_overview().unwrap().groups, 2);
    }

    #[test]
    fn artwork_faces_are_left_out_of_unsure_and_out_of_groups() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 100.0, 200.0]);
        assert_eq!(db.face_overview().unwrap().unsorted, 3);
        assert_eq!(db.faces_needing_art(db.face_location(ids[0]).unwrap().unwrap().0.id).unwrap().len(), 1, "not looked at yet");
        db.set_face_art(ids[1], true).unwrap();
        assert_eq!(db.face_overview().unwrap().unsorted, 2);
        let listed: Vec<i64> = db.unsorted_faces(0, 10).unwrap().iter().map(|u| u.tile.id).collect();
        assert_eq!(listed.len(), 2);
        assert!(!listed.contains(&ids[1]));

        // three look-alike faces in three photos are a group, unless one of them is a painting
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 3.0, 6.0, 9.0]);
        db.set_face_art(ids[3], true).unwrap();
        let r = db.tidy().unwrap().unwrap();
        assert_eq!((r.groups, r.unsorted), (1, 1));
        assert_eq!(person_of(&db, ids[3]), None);
        assert!(person_of(&db, ids[0]).is_some());
        // and a face marked artwork after it was grouped leaves its group at the next regrouping
        db.set_face_art(ids[2], true).unwrap();
        db.conn().execute("UPDATE face_meta SET rev = rev + 1", []).unwrap();
        db.tidy().unwrap();
        assert_eq!(person_of(&db, ids[2]), None);
    }

    #[test]
    fn faces_in_no_group_are_listed_with_look_alikes_together() {
        let db = Db::open_in_memory().unwrap();
        // interleaved in the order they were found, so only the ordering can bring each person together
        let ids = library(&db, &[0.0, 100.0, 4.0, 104.0, 250.0]);
        db.tidy().unwrap();
        let order: Vec<i64> = db.unsorted_faces(0, 20).unwrap().iter().map(|u| u.tile.id).collect();
        assert_eq!(order.len(), 5);
        let person = |id: i64| ids.iter().position(|x| *x == id).map(|i| [0, 1, 0, 1, 2][i]).unwrap();
        let flips = order.windows(2).filter(|w| person(w[0]) != person(w[1])).count();
        assert!(flips <= 2, "each person is one run: {order:?}");
    }

    #[test]
    fn a_group_keeps_its_identity_when_more_faces_join_it() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let before = person_of(&db, ids[0]).unwrap();
        let f = db.add_folder("/p").unwrap();
        let p = db.upsert_photo(f, "/p/new.jpg", 1, 1, &meta()).unwrap();
        let new = db.replace_faces(p, "fv", &[nf(9.0)]).unwrap()[0];
        db.tidy().unwrap();
        assert_eq!(person_of(&db, ids[0]), Some(before));
        assert_eq!(person_of(&db, new), Some(before));
    }

    #[test]
    fn naming_a_group_confirms_it_and_new_faces_that_clearly_match_join_it_unchecked() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let joe = person_of(&db, ids[0]).unwrap();
        db.name_person(joe, Some("Joe")).unwrap();
        assert!(ids[0..3].iter().all(|f| confirmed_of(&db, *f)));
        assert_eq!(db.list_people().unwrap().len(), 1);

        let f = db.add_folder("/p").unwrap();
        let p = db.upsert_photo(f, "/p/new.jpg", 1, 1, &meta()).unwrap();
        let new = db.replace_faces(p, "fv", &[nf(4.0)]).unwrap()[0];
        let far = db.replace_faces(db.upsert_photo(f, "/p/far.jpg", 1, 1, &meta()).unwrap(), "fv", &[nf(150.0)]).unwrap()[0];
        assert_eq!(db.attach_confident(Some(&[new, far])).unwrap(), 1);
        assert_eq!(person_of(&db, new), Some(joe));
        assert!(!confirmed_of(&db, new), "it is a guess until the user looks");
        assert_eq!(person_of(&db, far), None);
        assert_eq!(db.face_overview().unwrap().unchecked, 1);

        db.confirm_faces(&[new]).unwrap();
        assert_eq!(db.face_overview().unwrap().unchecked, 0);
    }

    #[test]
    fn a_confirmed_person_is_not_regrouped_and_pulls_unsorted_faces_in_on_tidy() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let joe = person_of(&db, ids[0]).unwrap();
        db.name_person(joe, Some("Joe")).unwrap();
        let other = person_of(&db, ids[3]).unwrap();
        // two loose faces next to Joe's join him at the next tidy, and Joe's own faces never move
        let f = db.add_folder("/p").unwrap();
        let near: Vec<i64> = (0..2).map(|i| db.replace_faces(db.upsert_photo(f, &format!("/p/n{i}.jpg"), 1, 1, &meta()).unwrap(), "fv", &[nf(2.0 + i as f32)]).unwrap()[0]).collect();
        let r = db.tidy().unwrap().unwrap();
        assert_eq!(r.attached, 2);
        assert!(near.iter().all(|f| person_of(&db, *f) == Some(joe)));
        assert!(ids[0..3].iter().all(|f| person_of(&db, *f) == Some(joe)));
        assert_eq!(person_of(&db, ids[3]), Some(other), "the other group is still the same group");
    }

    #[test]
    fn naming_a_group_with_a_name_somebody_has_merges_the_two() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 2.0, 4.0, 100.0, 102.0, 104.0]);
        db.tidy().unwrap();
        let (a, b) = (person_of(&db, ids[0]).unwrap(), person_of(&db, ids[3]).unwrap());
        assert_ne!(a, b);
        db.name_person(a, Some("Sam")).unwrap();
        assert_eq!(db.name_person(b, Some(" sam ")).unwrap(), a);
        assert!(ids.iter().all(|f| person_of(&db, *f) == Some(a) && confirmed_of(&db, *f)));
        assert_eq!(db.list_face_people(FaceView::People).unwrap().len(), 1);
    }

    #[test]
    fn assign_group_moves_a_group_into_a_person() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 2.0, 4.0, 90.0, 92.0, 94.0]);
        db.tidy().unwrap();
        let (a, b) = (person_of(&db, ids[0]).unwrap(), person_of(&db, ids[3]).unwrap());
        db.name_person(a, Some("Sam")).unwrap();
        db.assign_group(b, a).unwrap();
        assert!(ids.iter().all(|f| person_of(&db, *f) == Some(a) && confirmed_of(&db, *f)));
        assert_eq!(db.list_face_people(FaceView::Groups).unwrap().len(), 0);
    }

    #[test]
    fn rejecting_a_face_leaves_it_in_no_group_and_nothing_guesses_it_back() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let joe = person_of(&db, ids[0]).unwrap();
        db.name_person(joe, Some("Joe")).unwrap();
        db.reject_faces(&[ids[1]]).unwrap();
        assert_eq!(person_of(&db, ids[1]), None);
        assert_eq!(db.attach_confident(Some(&[ids[1]])).unwrap(), 0, "it was rejected from Joe");
        db.tidy().unwrap();
        assert_eq!(person_of(&db, ids[1]), None);
        let u = db.unsorted_faces(0, 10).unwrap();
        let mine = u.iter().find(|u| u.tile.id == ids[1]).unwrap();
        assert!(mine.suggestion.is_none(), "and Joe is not suggested for it either");
        // put back by hand
        db.move_faces(&[ids[1]], joe).unwrap();
        assert_eq!(person_of(&db, ids[1]), Some(joe));
        assert!(confirmed_of(&db, ids[1]));
    }

    #[test]
    fn faces_in_no_group_come_with_who_they_look_like() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        db.name_person(person_of(&db, ids[0]).unwrap(), Some("Joe")).unwrap();
        let f = db.add_folder("/p").unwrap();
        let p = db.upsert_photo(f, "/p/m.jpg", 1, 1, &meta()).unwrap();
        // 45 degrees from Joe: a cosine of 0.7 with his faces is alike enough to suggest but a score the wrong side of a clear match
        let maybe = db.replace_faces(p, "fv", &[nf(50.0)]).unwrap()[0];
        let u = db.unsorted_faces(0, 20).unwrap();
        let s = u.iter().find(|u| u.tile.id == maybe).unwrap().suggestion.clone();
        assert!(s.is_none() || s.unwrap().name.as_deref() == Some("Joe"));
        let nearby = u.iter().find(|u| u.tile.id == ids[7]).unwrap();
        assert!(nearby.suggestion.is_none(), "180 degrees away is nobody Joe looks like");
    }

    #[test]
    fn a_card_shows_faces_that_spread_across_the_person_and_says_when_it_looks_mixed() {
        let db = Db::open_in_memory().unwrap();
        // one consistent group and one that spans two people 70 degrees apart (cosine 0.34, below "same person")
        let mut a: Vec<f32> = (0..8).map(|i| i as f32).collect();
        a.extend((0..8).map(|i| 200.0 + i as f32));
        let ids = library(&db, &a);
        db.tidy().unwrap();
        let cards = db.list_face_people(FaceView::Groups).unwrap();
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().all(|c| c.samples.len() >= 3 && c.samples.len() <= SAMPLES));
        assert!(cards.iter().all(|c| !c.looks_mixed && !c.confirmed));
        assert_eq!(cards[0].faces, 8);
        assert!(cards[0].samples.iter().all(|s| ids.contains(s)));
    }

    #[test]
    fn a_mixed_group_can_be_split_into_people_to_name() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 100.0, 101.0, 102.0, 103.0, 300.0]);
        db.tidy().unwrap();
        // force one group out of everything to stand in for a group the grouping got wrong
        let all = &ids[..10];
        let person = db.new_person_from_faces(all, None).unwrap();
        // stand in for a group the grouping made (not yet confirmed)
        db.conn().execute("UPDATE people SET confirmed = 0", []).unwrap();
        db.conn().execute("UPDATE faces SET confirmed = 0", []).unwrap();
        db.conn().execute("UPDATE people SET mixed = (SELECT 0.5) WHERE id = ?1", params![person]).unwrap();
        assert_eq!(person_of(&db, ids[0]), Some(person));
        assert_eq!(db.list_face_people(FaceView::Groups).unwrap().iter().find(|c| c.id == person).unwrap().looks_mixed, true);
        let pieces = db.split_group(person).unwrap();
        assert_eq!(pieces, 2);
        assert_eq!(person_of(&db, ids[0]), Some(person), "the biggest piece keeps the group");
        assert_ne!(person_of(&db, ids[6]), Some(person));
        assert_eq!(person_of(&db, ids[6]), person_of(&db, ids[9]));
        // the pieces are settled: a later tidy leaves them alone
        db.tidy().unwrap();
        assert_ne!(person_of(&db, ids[0]), person_of(&db, ids[6]));
        assert_eq!(db.split_group(person).unwrap(), 0, "a confirmed person is not split again");
    }

    #[test]
    fn suggestions_name_who_some_faces_or_a_group_look_like_best_first() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 3.0, 6.0, 90.0, 93.0, 96.0, 60.0, 64.0, 68.0]);
        let (joe, sam) = (db.new_person_from_faces(&ids[0..3], Some("Joe")).unwrap(), db.new_person_from_faces(&ids[3..6], Some("Sam")).unwrap());
        // a face at 30 degrees looks like Joe (cos 0.87 to 6) and a bit like Sam (cos 0.5); at 60 like Sam more
        let near_joe = db.suggest_for_faces(&[ids[0]]).unwrap();
        assert_eq!(near_joe.first().map(|s| (s.person_id, s.name.clone())), Some((joe, Some("Joe".to_string()))));
        // a group of faces between the two: ranked, best first, never the group itself
        let group = db.new_person_from_faces(&ids[6..9], None).unwrap();
        db.conn().execute("UPDATE people SET confirmed = 0 WHERE id = ?1", params![group]).unwrap();
        let s = db.suggest_for_group(group).unwrap();
        assert!(s.iter().all(|x| x.person_id != group));
        assert_eq!(s.first().map(|x| x.person_id), Some(sam), "{s:?}");
        assert!(s.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(db.suggest_for_faces(&[]).unwrap().is_empty());
    }

    #[test]
    fn ignored_faces_keep_their_own_picture_and_leave_their_group() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 3.0, 6.0, 9.0, 100.0]);
        db.tidy().unwrap();
        let group = person_of(&db, ids[0]).unwrap();
        assert_eq!(db.face_overview().unwrap().unsorted, 1);
        // ignore one grouped face and the loose one: each stays its own face, nothing is made of them
        db.ignore_faces(&[ids[1], ids[4]]).unwrap();
        assert_eq!(person_of(&db, ids[1]), None);
        assert_eq!(person_of(&db, ids[0]), Some(group), "the rest of the group is untouched");
        let o = db.face_overview().unwrap();
        assert_eq!((o.unsorted, o.hidden), (0, 2));
        assert_eq!(db.ignored_faces(0, 10).unwrap().len(), 2);
        assert!(db.list_face_people(FaceView::Hidden).unwrap().is_empty(), "no group was made of them");
        // regrouping leaves them alone
        db.tidy().unwrap();
        assert_eq!(person_of(&db, ids[1]), None);
        assert_eq!(db.ignored_faces(0, 10).unwrap().len(), 2);
        // stop ignoring one: it is loose again; give the other to someone: it is no longer ignored
        db.unignore_faces(&[ids[4]]).unwrap();
        assert_eq!(db.face_overview().unwrap().unsorted, 1);
        db.move_faces(&[ids[1]], group).unwrap();
        assert_eq!(db.ignored_faces(0, 10).unwrap().len(), 0);
        assert_eq!(person_of(&db, ids[1]), Some(group));
    }

    #[test]
    fn hiding_confirms_and_unhiding_an_unnamed_group_makes_it_a_group_again() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let g = person_of(&db, ids[0]).unwrap();
        db.hide_person(g).unwrap();
        assert_eq!(db.list_face_people(FaceView::Hidden).unwrap().len(), 1);
        assert_eq!(db.face_overview().unwrap().groups, 1);
        db.tidy().unwrap();
        assert_eq!(person_of(&db, ids[0]), Some(g), "a hidden person is not regrouped");
        db.unhide_person(g).unwrap();
        assert!(!confirmed_of(&db, ids[0]));
        assert_eq!(db.face_overview().unwrap().groups, 2);
    }

    #[test]
    fn a_rescan_that_replaces_faces_makes_a_regrouping_that_was_in_progress_start_again() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let snap = db.snapshot().unwrap();
        let pl = plan(&snap);
        // a photo with a grouped face is scanned again while the plan was being made
        let photo = db.faces_of(1).unwrap();
        assert!(!photo.is_empty());
        db.replace_faces(1, "fv", &[nf(0.5)]).unwrap();
        assert!(db.apply(&snap, pl).unwrap().is_none());
        assert!(person_of(&db, ids[1]).is_some());
    }

    #[test]
    fn a_pass_that_changes_nothing_leaves_the_revision_alone() {
        let db = Db::open_in_memory().unwrap();
        library(&db, &ANGLES);
        db.tidy().unwrap();
        let rev = db.face_rev().unwrap();
        db.tidy().unwrap();
        assert_eq!(db.face_rev().unwrap(), rev);
    }

    #[test]
    fn removing_a_photo_with_faces_updates_who_it_was_filed_under() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &ANGLES);
        db.tidy().unwrap();
        let g = person_of(&db, ids[0]).unwrap();
        let before = db.list_face_people(FaceView::Groups).unwrap().iter().find(|c| c.id == g).unwrap().faces;
        db.replace_faces(1, "fv", &[]).unwrap();
        let after = db.list_face_people(FaceView::Groups).unwrap().iter().find(|c| c.id == g).unwrap().faces;
        assert_eq!((before, after), (3, 2));
    }

    #[test]
    fn a_persons_faces_come_best_first_or_most_doubtful_first_or_only_those_to_check() {
        let db = Db::open_in_memory().unwrap();
        let ids = library(&db, &[0.0, 2.0, 4.0, 6.0, 40.0]);
        let joe = db.new_person_from_faces(&ids, Some("Joe")).unwrap();
        let order = |o| db.faces_of_person(joe, 0, 10, o).unwrap().iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(order(FaceOrder::Best).len(), 5);
        assert_eq!(order(FaceOrder::Doubtful)[0], ids[4], "the face far from the rest is first");
        assert!(order(FaceOrder::Unchecked).is_empty(), "the user said they were all Joe");
        assert_eq!(db.faces_of_person(joe, 4, 10, FaceOrder::Best).unwrap().len(), 1, "paging");
    }

    #[test]
    fn the_least_typical_faces_of_a_named_group_are_not_used_as_reference_points() {
        let db = Db::open_in_memory().unwrap();
        // 20 faces of one person around 0 degrees, and one wrong face (someone else) wrongly in the group
        let mut a: Vec<f32> = (0..20).map(|i| i as f32 * 0.5).collect();
        a.push(80.0);
        let ids = library(&db, &a);
        let person = db.new_person_from_faces(&ids, Some("Rose")).unwrap();
        let anchors = anchors_of(&db.conn(), false).unwrap();
        let kept = &anchors.iter().find(|(p, _)| *p == person).unwrap().1;
        assert_eq!(kept.len(), 18, "21 faces, the least typical 15% left out");
        let wrong = at(80.0);
        assert!(kept.iter().all(|f| f.iter().zip(&wrong).map(|(x, y)| x * y).sum::<f32>() < 0.9), "the wrong face is not a reference point");
    }

    /// Run on a copy of a real library: `PIXIE_LIVE_DB=path cargo test -p pixie-index live_library -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_library() {
        let path = std::env::var("PIXIE_LIVE_DB").expect("PIXIE_LIVE_DB");
        let db = Db::open(std::path::Path::new(&path)).unwrap();
        let t = std::time::Instant::now();
        let report = db.tidy().unwrap().unwrap();
        println!("tidy {:?}: {:?}", t.elapsed(), report);
        println!("overview {:?}", db.face_overview().unwrap());
        let t = std::time::Instant::now();
        let cards = db.list_face_people(FaceView::Groups).unwrap();
        println!("cards {:?}: {}", t.elapsed(), cards.len());
        let mixed = cards.iter().filter(|c| c.looks_mixed).count();
        let faces: i64 = cards.iter().map(|c| c.faces).sum();
        let mixed_faces: i64 = cards.iter().filter(|c| c.looks_mixed).map(|c| c.faces).sum();
        println!("groups {} (mixed {}), faces in groups {} (in mixed {})", cards.len(), mixed, faces, mixed_faces);
        for c in cards.iter().take(8) {
            println!("  #{} faces {} photos {} mixed {:?} samples {}", c.id, c.faces, c.photos, c.mixed, c.samples.len());
        }
        let t = std::time::Instant::now();
        let u = db.unsorted_faces(0, 60).unwrap();
        println!("unsorted page {:?}: {}", t.elapsed(), u.len());
        let t = std::time::Instant::now();
        let again = db.tidy().unwrap().unwrap();
        println!("second tidy {:?}: {:?} (should change nothing)", t.elapsed(), again);
    }

    /// How many faces name-then-tidy attaches for each share of reference faces left out:
    /// `PIXIE_LIVE_DB=path cargo test --release -p pixie-index live_trim -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_trim() {
        let path = std::env::var("PIXIE_LIVE_DB").expect("PIXIE_LIVE_DB");
        for share in [0.0f32, 0.15, 0.30] {
            TRIM.store(share.to_bits(), std::sync::atomic::Ordering::Relaxed);
            let copy = format!("{path}.trim.db");
            std::fs::copy(&path, &copy).unwrap();
            let db = Db::open(std::path::Path::new(&copy)).unwrap();
            db.tidy().unwrap();
            let cards = db.list_face_people(FaceView::Groups).unwrap();
            // name the biggest consistent groups, as a user would
            let mut named = 0;
            for c in cards.iter().filter(|c| !c.looks_mixed).take(6) {
                db.name_person(c.id, Some(&format!("P{}", c.id))).unwrap();
                named += c.faces;
            }
            let before = db.face_overview().unwrap();
            let r = db.tidy().unwrap().unwrap();
            let after = db.face_overview().unwrap();
            println!("trim {share}: named {named} faces; tidy attached {} (groups {} -> {}, unsorted {} -> {}, unchecked {})", r.attached, before.groups, after.groups, before.unsorted, after.unsorted, after.unchecked);
        }
        TRIM.store(ANCHOR_TRIM.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    /// `PIXIE_LIVE_DB=path cargo test --release -p pixie-index live_try_group -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_try_group() {
        let path = std::env::var("PIXIE_LIVE_DB").expect("PIXIE_LIVE_DB");
        let copy = format!("{path}.loose.db");
        std::fs::copy(&path, &copy).unwrap();
        let db = Db::open(std::path::Path::new(&copy)).unwrap();
        db.tidy().unwrap();
        let before = db.face_overview().unwrap();
        let t = std::time::Instant::now();
        let moved = db.try_group().unwrap();
        let after = db.face_overview().unwrap();
        println!("try_group {:?}: {} faces left Unsure; groups {} -> {}, unsure {} -> {}", t.elapsed(), moved, before.groups, after.groups, before.unsorted, after.unsorted);
        let cards = db.list_face_people(FaceView::Groups).unwrap();
        let small: Vec<i64> = cards.iter().filter(|c| c.photos <= 3).map(|c| c.faces).collect();
        println!("groups with 3 or fewer photos: {}; mixed-looking: {}", small.len(), cards.iter().filter(|c| c.looks_mixed).count());
        let t = std::time::Instant::now();
        let u = db.unsorted_faces(0, 60).unwrap();
        println!("unsure page {:?}: {}", t.elapsed(), u.len());
    }
}
