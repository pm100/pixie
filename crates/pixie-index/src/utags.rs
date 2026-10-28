//! Tags the user puts on photos, and Picked and those tags as a browse filter.
use crate::db::{db_err, order_sql, refresh_hits, row_to_photo, Db, PHOTO_COLS};
use pixie_core::{normalize_tag, Photo, PhotoId, PixieError, Result, SortOrder, TagUse, UserTagCount, UserTagFilter};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Does any photo that Browse lists (not missing, not a hidden duplicate) have this tag?
fn tag_is_listed(c: &Connection, name: &str) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM user_tags t JOIN photos p ON p.id = t.photo_id WHERE t.tag = ?1 AND p.missing = 0 AND p.dup = 0)",
        params![name],
        |r| r.get(0),
    )
    .map_err(db_err)
}

/// Fill `utag_hits` for the chosen filter again (picks and tags change under it), then fold it into `search_hits`. A chosen
/// tag that no photo has any more is dropped, so the view does not stay on an empty tag.
pub(crate) fn refresh_utag(tx: &Transaction) -> Result<()> {
    let key: Option<String> = tx.query_row("SELECT value FROM filter_state WHERE name = 'utag'", [], |r| r.get(0)).optional().map_err(db_err)?;
    let Some(filter) = key.as_deref().and_then(UserTagFilter::parse) else { return Ok(()) };
    if let UserTagFilter::Tag(name) = &filter {
        if !tag_is_listed(tx, name)? {
            tx.execute("DELETE FROM utag_hits", []).map_err(db_err)?;
            tx.execute("DELETE FROM filter_state WHERE name = 'utag'", []).map_err(db_err)?;
            return refresh_hits(tx);
        }
    }
    tx.execute("DELETE FROM utag_hits", []).map_err(db_err)?;
    match &filter {
        UserTagFilter::Picked => tx.execute("INSERT INTO utag_hits(photo_id) SELECT photo_id FROM picks", []),
        UserTagFilter::Tag(name) => tx.execute("INSERT INTO utag_hits(photo_id) SELECT photo_id FROM user_tags WHERE tag = ?1", params![name]),
    }
    .map_err(db_err)?;
    refresh_hits(tx)
}

/// `refresh_utag` for a caller that holds the connection rather than a transaction.
pub(crate) fn refresh_utag_hits(c: &mut Connection) -> Result<()> {
    let tx = c.transaction().map_err(db_err)?;
    refresh_utag(&tx)?;
    tx.commit().map_err(db_err)
}

impl Db {
    /// Give all of these photos the tag; the name it is stored under (the first spelling used) is returned. Ids that are
    /// not in the library are skipped. A name that cannot be a tag is an error.
    pub fn tag_photos(&self, ids: &[PhotoId], tag: &str) -> Result<String> {
        let name = normalize_tag(tag).ok_or_else(|| PixieError::Invalid("Type a name for the tag, up to 60 characters.".into()))?;
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let stored = tx.query_row("SELECT tag FROM user_tags WHERE tag = ?1 LIMIT 1", params![name], |r| r.get::<_, String>(0)).optional().map_err(db_err)?;
        let name = stored.unwrap_or(name);
        {
            let mut ins = tx.prepare("INSERT OR IGNORE INTO user_tags(photo_id, tag) SELECT id, ?2 FROM photos WHERE id = ?1").map_err(db_err)?;
            for id in ids {
                ins.execute(params![id, name]).map_err(db_err)?;
            }
        }
        refresh_utag(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(name)
    }

    /// Take the tag off all of these photos.
    pub fn untag_photos(&self, ids: &[PhotoId], tag: &str) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        {
            let mut del = tx.prepare("DELETE FROM user_tags WHERE photo_id = ?1 AND tag = ?2").map_err(db_err)?;
            for id in ids {
                del.execute(params![id, tag.trim()]).map_err(db_err)?;
            }
        }
        refresh_utag(&tx)?;
        tx.commit().map_err(db_err)
    }

    /// Show only the picked photos, or only the photos with one tag; `None` turns it off. Combines with the other filters.
    pub fn set_user_tag_filter(&self, f: Option<&UserTagFilter>) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        match f {
            Some(f) => {
                tx.execute("INSERT OR REPLACE INTO filter_state(name, value) VALUES ('utag', ?1)", params![f.key()]).map_err(db_err)?;
                refresh_utag(&tx)?;
            }
            None => {
                tx.execute("DELETE FROM utag_hits", []).map_err(db_err)?;
                tx.execute("DELETE FROM filter_state WHERE name = 'utag'", []).map_err(db_err)?;
                refresh_hits(&tx)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    /// The Picked / tag filter the listings are really restricted to, as `UserTagFilter::key` (the chip is drawn from this).
    /// A chosen tag that no listed photo has any more (they were untagged, hidden as duplicates, or their folder was removed)
    /// is dropped here, so the chip cannot stay over an empty view.
    pub fn active_user_tag(&self) -> Result<Option<String>> {
        let mut c = self.conn();
        let key = c.query_row("SELECT value FROM filter_state WHERE name = 'utag'", [], |r| r.get::<_, Option<String>>(0)).optional().map_err(db_err)?.flatten();
        let Some(key) = key else { return Ok(None) };
        if let Some(UserTagFilter::Tag(name)) = UserTagFilter::parse(&key) {
            if !tag_is_listed(&c, &name)? {
                let tx = c.transaction().map_err(db_err)?;
                tx.execute("DELETE FROM utag_hits", []).map_err(db_err)?;
                tx.execute("DELETE FROM filter_state WHERE name = 'utag'", []).map_err(db_err)?;
                refresh_hits(&tx)?;
                tx.commit().map_err(db_err)?;
                return Ok(None);
            }
        }
        Ok(Some(key))
    }

    /// Every tag that has a photo listed in Browse, with how many, by name.
    pub fn user_tags(&self) -> Result<Vec<UserTagCount>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT t.tag, COUNT(*) FROM user_tags t JOIN photos p ON p.id = t.photo_id
                 WHERE p.missing = 0 AND p.dup = 0 GROUP BY t.tag ORDER BY t.tag COLLATE NOCASE",
            )
            .map_err(db_err)?;
        let rows = s.query_map([], |r| Ok(UserTagCount { name: r.get(0)?, count: r.get(1)? })).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
        Ok(rows)
    }

    /// Every tag in the library with how many of these photos have it (0 for one they do not).
    pub fn tag_usage(&self, ids: &[PhotoId]) -> Result<Vec<TagUse>> {
        let json = format!("[{}]", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT t.tag, SUM(t.photo_id IN (SELECT value FROM json_each(?1))) FROM user_tags t
                 GROUP BY t.tag ORDER BY t.tag COLLATE NOCASE",
            )
            .map_err(db_err)?;
        let rows = s.query_map(params![json], |r| Ok(TagUse { name: r.get(0)?, on: r.get(1)? })).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
        Ok(rows)
    }

    /// The photos with the tag, newest first, like the picked ones.
    pub fn list_tagged(&self, tag: &str) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS} FROM photos WHERE missing = 0 AND dup = 0 AND id IN (SELECT photo_id FROM user_tags WHERE tag = ?1) ORDER BY {}",
                order_sql(SortOrder::Newest)
            ))
            .map_err(db_err)?;
        let rows = s.query_map(params![tag.trim()], row_to_photo).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Db;
    use pixie_core::{PhotoId, PhotoMeta, TagUse, UserTagCount};
    use rusqlite::params;

    fn photos(n: usize) -> (Db, Vec<PhotoId>) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let ids = (0..n).map(|i| db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &PhotoMeta::default()).unwrap()).collect();
        (db, ids)
    }
    fn count(name: &str, count: i64) -> UserTagCount {
        UserTagCount { name: name.into(), count }
    }

    #[test]
    fn the_first_spelling_is_kept() {
        let (db, p) = photos(2);
        assert_eq!(db.tag_photos(&[p[0]], "Holiday").unwrap(), "Holiday");
        assert_eq!(db.tag_photos(&[p[1]], " #holiday ").unwrap(), "Holiday");
        assert_eq!(db.user_tags().unwrap(), vec![count("Holiday", 2)]);
    }

    #[test]
    fn a_name_that_cannot_be_a_tag_is_refused() {
        let (db, p) = photos(1);
        assert!(db.tag_photos(&[p[0]], "  # ").is_err());
        assert!(db.user_tags().unwrap().is_empty());
    }

    #[test]
    fn ids_that_are_not_in_the_library_are_ignored() {
        let (db, p) = photos(1);
        db.tag_photos(&[p[0], 9999], "x").unwrap();
        assert_eq!(db.user_tags().unwrap(), vec![count("x", 1)]);
    }

    #[test]
    fn a_big_selection_is_one_call() {
        let (db, p) = photos(3);
        let mut ids: Vec<PhotoId> = (100_000..110_000).collect();
        ids.extend(&p);
        db.tag_photos(&ids, "big").unwrap();
        assert_eq!(db.user_tags().unwrap(), vec![count("big", 3)]);
    }

    #[test]
    fn counts_leave_out_missing_and_duplicate_photos() {
        let (db, p) = photos(4);
        db.tag_photos(&p, "a").unwrap();
        db.tag_photos(&[p[2], p[3]], "b").unwrap();
        db.conn().execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![p[0]]).unwrap();
        db.conn().execute("UPDATE photos SET dup = 1 WHERE id = ?1", params![p[1]]).unwrap();
        db.conn().execute("UPDATE photos SET dup = 1 WHERE id IN (?1, ?2)", params![p[2], p[3]]).unwrap();
        // p0 is missing and p1, p2, p3 are duplicates: nothing is listed under either tag.
        assert_eq!(db.user_tags().unwrap(), vec![]);
        db.conn().execute("UPDATE photos SET dup = 0 WHERE id = ?1", params![p[2]]).unwrap();
        assert_eq!(db.user_tags().unwrap(), vec![count("a", 1), count("b", 1)]);
    }

    #[test]
    fn usage_says_how_many_of_the_given_photos_have_each_tag() {
        let (db, p) = photos(3);
        db.tag_photos(&[p[0], p[1]], "both").unwrap();
        db.tag_photos(&[p[0]], "one").unwrap();
        db.tag_photos(&[p[2]], "other").unwrap();
        let u = db.tag_usage(&[p[0], p[1]]).unwrap();
        let use_of = |n: &str, on: i64| TagUse { name: n.into(), on };
        assert_eq!(u, vec![use_of("both", 2), use_of("one", 1), use_of("other", 0)]);
    }

    #[test]
    fn untagging_removes_the_tag_and_the_last_removal_removes_the_tag_itself() {
        let (db, p) = photos(2);
        db.tag_photos(&p, "t").unwrap();
        db.untag_photos(&[p[0]], "T").unwrap();
        assert_eq!(db.user_tags().unwrap(), vec![count("t", 1)]);
        db.untag_photos(&[p[1]], "t").unwrap();
        assert!(db.user_tags().unwrap().is_empty());
        assert!(db.tag_usage(&p).unwrap().is_empty());
    }

    #[test]
    fn tagged_photos_are_listed_newest_first() {
        let (db, p) = photos(3);
        db.tag_photos(&[p[0], p[2]], "t").unwrap();
        let ids: Vec<PhotoId> = db.list_tagged("T").unwrap().iter().map(|x| x.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&p[0]) && ids.contains(&p[2]));
    }

    // ---- Picked and tags as a browse filter ----
    use crate::db::FacetAsk;
    use pixie_core::{PhotoQuery, UserTagFilter};

    fn listed(db: &Db) -> Vec<PhotoId> {
        let mut ids: Vec<PhotoId> = db.list_photos_in(&PhotoQuery { search: true, ..Default::default() }, 0, 100).unwrap().iter().map(|p| p.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn choosing_picked_lists_only_picked_photos() {
        let (db, p) = photos(3);
        db.set_pick(p[1], true).unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Picked)).unwrap();
        assert_eq!(listed(&db), vec![p[1]]);
        assert_eq!(db.active_user_tag().unwrap().as_deref(), Some("picked"));
        db.set_user_tag_filter(None).unwrap();
        assert_eq!(db.active_user_tag().unwrap(), None);
    }

    #[test]
    fn the_filter_follows_picks_and_tags_at_once() {
        let (db, p) = photos(3);
        db.set_user_tag_filter(Some(&UserTagFilter::Picked)).unwrap();
        db.set_pick(p[2], true).unwrap();
        assert_eq!(listed(&db), vec![p[2]]);
        db.set_picks(&[p[0]]).unwrap();
        assert_eq!(listed(&db), vec![p[0], p[2]]);
        db.clear_picks().unwrap();
        assert!(listed(&db).is_empty());
        // (a tag exists only while a photo has it, so one photo has it before the tag is chosen)
        db.tag_photos(&[p[0]], "Trip").unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Tag("Trip".into()))).unwrap();
        db.tag_photos(&[p[1]], "trip").unwrap();
        assert_eq!(listed(&db), vec![p[0], p[1]]);
        db.untag_photos(&[p[0]], "Trip").unwrap();
        assert_eq!(listed(&db), vec![p[1]]);
    }

    #[test]
    fn it_combines_with_the_other_filters() {
        let (db, p) = photos(3);
        db.conn().execute("UPDATE photos SET camera = 'Pixel 7' WHERE id IN (?1, ?2)", params![p[0], p[1]]).unwrap();
        db.tag_photos(&[p[1], p[2]], "t").unwrap();
        db.set_camera_filter(Some("Pixel 7")).unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Tag("t".into()))).unwrap();
        assert_eq!(listed(&db), vec![p[1]]);
        db.set_camera_filter(None).unwrap();
        assert_eq!(listed(&db), vec![p[1], p[2]]);
    }

    #[test]
    fn the_last_photo_off_the_active_tag_clears_the_filter() {
        let (db, p) = photos(2);
        db.tag_photos(&[p[0]], "solo").unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Tag("solo".into()))).unwrap();
        assert_eq!(db.active_user_tag().unwrap().as_deref(), Some("tag:solo"));
        db.untag_photos(&[p[0]], "solo").unwrap();
        assert_eq!(db.active_user_tag().unwrap(), None);
        assert!(listed(&db).is_empty(), "no filter is active, so the search flag has nothing to list");
    }

    #[test]
    fn facet_counts_narrow_to_the_chosen_tag_or_the_picked_photos() {
        let (db, p) = photos(4);
        db.tag_photos(&[p[0], p[1]], "t").unwrap();
        db.set_pick(p[3], true).unwrap();
        fn ask(utag: Option<&UserTagFilter>) -> FacetAsk<'_> {
            FacetAsk { tag: None, person: None, camera: None, utag, trip_dates: false, tag_groups: &[], trip_windows: &[], recent_since: None }
        }
        let q = PhotoQuery::default();
        assert_eq!(db.facet_counts(&q, &ask(None)).unwrap().photos, 4);
        assert_eq!(db.facet_counts(&q, &ask(Some(&UserTagFilter::Tag("t".into())))).unwrap().photos, 2);
        assert_eq!(db.facet_counts(&q, &ask(Some(&UserTagFilter::Picked))).unwrap().photos, 1);
    }

    #[test]
    fn a_tag_whose_photos_are_all_hidden_drops_out_of_the_filter() {
        let (db, p) = photos(2);
        db.tag_photos(&[p[0], p[1]], "t").unwrap();
        db.conn().execute("UPDATE photos SET dup = 1 WHERE id = ?1", params![p[1]]).unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Tag("t".into()))).unwrap();
        db.untag_photos(&[p[0]], "t").unwrap(); // the duplicate still has the tag, but nothing is listed under it
        assert_eq!(db.active_user_tag().unwrap(), None);
    }

    #[test]
    fn a_tag_whose_photos_went_missing_drops_out_of_the_filter() {
        let (db, p) = photos(2);
        db.tag_photos(&[p[0]], "t").unwrap();
        db.set_user_tag_filter(Some(&UserTagFilter::Tag("t".into()))).unwrap();
        assert_eq!(db.active_user_tag().unwrap().as_deref(), Some("tag:t"));
        db.conn().execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![p[0]]).unwrap(); // its folder was removed
        assert_eq!(db.active_user_tag().unwrap(), None);
        assert!(listed(&db).is_empty());
    }
}
