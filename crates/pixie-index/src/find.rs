//! A read-only search of the library for Ask Pixie: unlike the Browse filters it keeps no state and changes what nothing shows.
use crate::db::{db_err, order_sql, row_to_photo, Db, PHOTO_COLS};
use pixie_core::{FindSpec, MediaKind, Photo, PhotoId, Result, SortOrder};
use rusqlite::{params, params_from_iter, OptionalExtension, ToSql};

impl Db {
    /// The photos (not missing, not hidden duplicates) that match every part of `spec`, newest first.
    pub fn find(&self, spec: &FindSpec) -> Result<Vec<PhotoId>> {
        let mut sql = String::from("SELECT id FROM photos WHERE missing = 0 AND dup = 0");
        let mut args: Vec<Box<dyn ToSql>> = Vec::new();
        let marks = |n: usize| vec!["?"; n].join(", ");
        if let Some(among) = &spec.among {
            sql.push_str(&format!(" AND id IN ({})", marks(among.len())));
            args.extend(among.iter().map(|id| Box::new(*id) as Box<dyn ToSql>));
        }
        if let Some(kind) = spec.kind {
            sql.push_str(" AND kind = ?");
            args.push(Box::new(if kind == MediaKind::Video { 1_i64 } else { 0_i64 }));
        }
        if let Some(since) = spec.since {
            sql.push_str(" AND taken_at >= ?");
            args.push(Box::new(since));
        }
        if let Some(until) = spec.until {
            sql.push_str(" AND taken_at < ?");
            args.push(Box::new(until));
        }
        // The column compares without regard to case, so the names are made unique the same way.
        let mut tags: Vec<String> = Vec::new();
        for t in &spec.tags {
            let t = t.trim();
            if !t.is_empty() && !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                tags.push(t.to_string());
            }
        }
        if !tags.is_empty() {
            sql.push_str(&format!(" AND (SELECT COUNT(DISTINCT tag) FROM user_tags WHERE photo_id = photos.id AND tag IN ({})) = {}", marks(tags.len()), tags.len()));
            args.extend(tags.into_iter().map(|t| Box::new(t) as Box<dyn ToSql>));
        }
        let mut people = spec.people.clone();
        people.sort_unstable();
        people.dedup();
        if !people.is_empty() {
            sql.push_str(&format!(" AND (SELECT COUNT(DISTINCT person_id) FROM faces WHERE photo_id = photos.id AND person_id IN ({})) = {}", marks(people.len()), people.len()));
            args.extend(people.into_iter().map(|p| Box::new(p) as Box<dyn ToSql>));
        }
        if spec.with_people {
            sql.push_str(" AND EXISTS (SELECT 1 FROM faces WHERE photo_id = photos.id AND (art IS NULL OR art = 0))");
        }
        if spec.picked {
            sql.push_str(" AND id IN (SELECT photo_id FROM picks)");
        }
        sql.push_str(&format!(" ORDER BY {}", order_sql(SortOrder::Newest)));
        let c = self.conn();
        let mut s = c.prepare(&sql).map_err(db_err)?;
        let rows = s.query_map(params_from_iter(args.iter().map(|a| a.as_ref())), |r| r.get::<_, PhotoId>(0)).map_err(db_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)
    }

    /// These photos themselves, in the order of `ids`; ids that are not in the library (or are missing) are left out.
    pub fn photos_by_ids(&self, ids: &[PhotoId]) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c.prepare(&format!("SELECT {PHOTO_COLS} FROM photos WHERE id = ?1 AND missing = 0")).map_err(db_err)?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(p) = s.query_row(params![id], row_to_photo).optional().map_err(db_err)? {
                out.push(p);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Db;
    use pixie_core::{FindSpec, MediaKind, PhotoId, PhotoMeta};
    use rusqlite::params;

    fn lib() -> (Db, Vec<PhotoId>) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let day = |d: i64| Some(1_700_000_000 + d * 86_400);
        let mut ids = Vec::new();
        for i in 0..5 {
            let meta = PhotoMeta { taken_at: day(i), kind: if i == 4 { MediaKind::Video } else { MediaKind::Photo }, ..Default::default() };
            ids.push(db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &meta).unwrap());
        }
        (db, ids)
    }
    fn find(db: &Db, spec: FindSpec) -> Vec<PhotoId> {
        db.find(&spec).unwrap()
    }

    #[test]
    fn nothing_given_matches_everything_newest_first() {
        let (db, ids) = lib();
        assert_eq!(find(&db, FindSpec::default()), vec![ids[4], ids[3], ids[2], ids[1], ids[0]]);
    }

    #[test]
    fn kind_and_dates_narrow_it() {
        let (db, ids) = lib();
        assert_eq!(find(&db, FindSpec { kind: Some(MediaKind::Video), ..Default::default() }), vec![ids[4]]);
        let since = 1_700_000_000 + 86_400;
        let until = 1_700_000_000 + 3 * 86_400;
        assert_eq!(find(&db, FindSpec { since: Some(since), until: Some(until), ..Default::default() }), vec![ids[2], ids[1]]);
    }

    #[test]
    fn among_limits_it_to_those_photos() {
        let (db, ids) = lib();
        assert_eq!(find(&db, FindSpec { among: Some(vec![ids[0], ids[3]]), ..Default::default() }), vec![ids[3], ids[0]]);
        assert!(find(&db, FindSpec { among: Some(vec![]), ..Default::default() }).is_empty());
    }

    #[test]
    fn tags_must_all_match_and_ignore_case() {
        let (db, ids) = lib();
        db.tag_photos(&[ids[0], ids[1], ids[2]], "holiday").unwrap();
        db.tag_photos(&[ids[1], ids[2]], "beach").unwrap();
        assert_eq!(find(&db, FindSpec { tags: vec!["Holiday".into()], ..Default::default() }), vec![ids[2], ids[1], ids[0]]);
        assert_eq!(find(&db, FindSpec { tags: vec!["holiday".into(), "BEACH".into()], ..Default::default() }), vec![ids[2], ids[1]]);
        assert!(find(&db, FindSpec { tags: vec!["nothing".into()], ..Default::default() }).is_empty());
    }

    #[test]
    fn people_must_all_be_in_the_photo() {
        let (db, ids) = lib();
        {
            let c = db.conn();
            c.execute("INSERT INTO people(centroid, n) VALUES (x'00', 0)", []).unwrap();
            c.execute("INSERT INTO people(centroid, n) VALUES (x'00', 0)", []).unwrap();
            for (photo, person) in [(ids[0], 1), (ids[1], 1), (ids[1], 2), (ids[2], 2)] {
                c.execute("INSERT INTO faces(photo_id, x, y, w, h, score, embedding, person_id) VALUES (?1, 0, 0, 1, 1, 1, x'00', ?2)", params![photo, person]).unwrap();
            }
        }
        assert_eq!(find(&db, FindSpec { people: vec![1], ..Default::default() }), vec![ids[1], ids[0]]);
        assert_eq!(find(&db, FindSpec { people: vec![1, 2], ..Default::default() }), vec![ids[1]]);
    }

    #[test]
    fn with_people_wants_a_real_face_not_one_in_artwork_and_not_an_unchecked_one_ruled_out() {
        let (db, ids) = lib();
        {
            let c = db.conn();
            // photo 0: a real face (art = 0); photo 1: only a face in a painting (art = 1); photo 2: a face not yet checked (NULL);
            // photo 3: no face at all.
            for (photo, art) in [(ids[0], Some(0)), (ids[1], Some(1)), (ids[2], None)] {
                c.execute("INSERT INTO faces(photo_id, x, y, w, h, score, embedding, art) VALUES (?1, 0, 0, 1, 1, 1, x'00', ?2)", params![photo, art]).unwrap();
            }
        }
        assert_eq!(find(&db, FindSpec { with_people: true, ..Default::default() }), vec![ids[2], ids[0]]);
        assert_eq!(find(&db, FindSpec { with_people: false, ..Default::default() }).len(), 5);
    }

    #[test]
    fn picked_only_looks_among_the_picks() {
        let (db, ids) = lib();
        db.set_picks(&[ids[1], ids[3]]).unwrap();
        assert_eq!(find(&db, FindSpec { picked: true, ..Default::default() }), vec![ids[3], ids[1]]);
    }

    #[test]
    fn missing_photos_and_hidden_duplicates_are_never_found() {
        let (db, ids) = lib();
        {
            let c = db.conn();
            c.execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![ids[4]]).unwrap();
            c.execute("UPDATE photos SET dup = 1 WHERE id = ?1", params![ids[3]]).unwrap();
        }
        assert_eq!(find(&db, FindSpec::default()), vec![ids[2], ids[1], ids[0]]);
    }

    #[test]
    fn it_leaves_the_browse_filters_as_they_were() {
        let (db, ids) = lib();
        db.tag_photos(&[ids[0]], "holiday").unwrap();
        db.set_picks(&[ids[1]]).unwrap();
        find(&db, FindSpec { tags: vec!["holiday".into()], picked: true, ..Default::default() });
        let c = db.conn();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM filter_state", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn photos_by_ids_keeps_the_order_asked_for_and_skips_what_is_not_there() {
        let (db, ids) = lib();
        let got: Vec<PhotoId> = db.photos_by_ids(&[ids[3], 9999, ids[0]]).unwrap().iter().map(|p| p.id).collect();
        assert_eq!(got, vec![ids[3], ids[0]]);
    }
}
