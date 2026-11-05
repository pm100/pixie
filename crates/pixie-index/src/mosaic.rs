//! Photomosaic: which photos can be tiles, and what is kept of each to use it as one.
use crate::db::{db_err, id_list, Db};
use pixie_core::{PhotoId, Result, SHARE_PRIVATE};
use rusqlite::params;
use std::collections::HashMap;

/// Where the tiles come from. Browse's filters and the picked photos reach here as the list of their ids.
#[derive(Debug, Clone, PartialEq)]
pub enum TileSource {
    Library,
    Ids(Vec<PhotoId>),
}

/// A photo that can be a tile.
#[derive(Debug, Clone, PartialEq)]
pub struct TileCandidate {
    pub id: PhotoId,
    pub path: String,
    pub mtime: i64,
    pub size: i64,
}

/// What is kept of a photo to use it as a tile (see `pixie_image::mosaic::Signature`), with the file's time and size it was made from.
#[derive(Debug, Clone, PartialEq)]
pub struct TileSig {
    pub id: PhotoId,
    pub mtime: i64,
    pub size: i64,
    pub avg: [u8; 3],
    pub tiny: Vec<u8>,
}

impl Db {
    /// The photos that can be tiles, in id order: pictures only, not missing, not hidden as a duplicate, not private. For `Ids`, only
    /// those among the ids; any number of ids.
    pub fn tile_candidates(&self, src: &TileSource) -> Result<Vec<TileCandidate>> {
        let c = self.conn();
        let (filter, ids) = match src {
            TileSource::Library => ("", String::new()),
            TileSource::Ids(ids) => (" AND id IN (SELECT value FROM json_each(?2))", id_list(ids)),
        };
        let sql = format!("SELECT id, path, mtime, size FROM photos WHERE missing = 0 AND dup = 0 AND hidden = 0 AND kind = 0 AND share != ?1{filter} ORDER BY id");
        let mut s = c.prepare(&sql).map_err(db_err)?;
        let map = |r: &rusqlite::Row| Ok(TileCandidate { id: r.get(0)?, path: r.get(1)?, mtime: r.get(2)?, size: r.get(3)? });
        let rows = match src {
            TileSource::Library => s.query_map(params![SHARE_PRIVATE], map),
            TileSource::Ids(_) => s.query_map(params![SHARE_PRIVATE, ids], map),
        }
        .map_err(db_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_err)?;
        Ok(rows)
    }

    /// Those of `cands` with no signature yet, or one made from a file that has since changed.
    pub fn tiles_without_sig(&self, cands: &[TileCandidate]) -> Result<Vec<TileCandidate>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT photo_id, mtime, size FROM tile_sigs").map_err(db_err)?;
        let have: HashMap<PhotoId, (i64, i64)> = s
            .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))
            .map_err(db_err)?
            .collect::<std::result::Result<_, _>>()
            .map_err(db_err)?;
        Ok(cands.iter().filter(|t| have.get(&t.id) != Some(&(t.mtime, t.size))).cloned().collect())
    }

    /// Keep these signatures (replacing any the photos had). A signature for a photo that is not in the library is ignored.
    pub fn set_tile_sigs(&self, sigs: &[TileSig]) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        {
            let mut s = tx
                .prepare("INSERT OR REPLACE INTO tile_sigs(photo_id, mtime, size, avg, tiny) SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM photos WHERE id = ?1)")
                .map_err(db_err)?;
            for t in sigs {
                s.execute(params![t.id, t.mtime, t.size, &t.avg[..], &t.tiny[..]]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    /// The signatures of these photos, in the order of `ids`; photos with none are left out.
    pub fn tile_sigs(&self, ids: &[PhotoId]) -> Result<Vec<TileSig>> {
        let c = self.conn();
        let mut s = c
            .prepare("SELECT photo_id, mtime, size, avg, tiny FROM tile_sigs WHERE photo_id IN (SELECT value FROM json_each(?1))")
            .map_err(db_err)?;
        let mut found: HashMap<PhotoId, TileSig> = HashMap::new();
        let rows = s
            .query_map(params![id_list(ids)], |r| {
                let avg: Vec<u8> = r.get(3)?;
                Ok(TileSig { id: r.get(0)?, mtime: r.get(1)?, size: r.get(2)?, avg: avg.try_into().unwrap_or([0; 3]), tiny: r.get(4)? })
            })
            .map_err(db_err)?;
        for row in rows {
            let t = row.map_err(db_err)?;
            found.insert(t.id, t);
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{MediaKind, PhotoMeta};

    fn library(n: usize) -> (Db, Vec<PhotoId>) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let ids = (0..n).map(|i| db.upsert_photo(f, &format!("/p/{i}.jpg"), 10, 20, &PhotoMeta::default()).unwrap()).collect();
        (db, ids)
    }

    fn sig(id: PhotoId, mtime: i64, size: i64) -> TileSig {
        TileSig { id, mtime, size, avg: [1, 2, 3], tiny: vec![7; 432] }
    }

    #[test]
    fn candidates_leave_out_videos_private_duplicates_and_missing() {
        let (db, p) = library(5);
        let f = db.add_folder("/v").unwrap();
        let video = db.upsert_photo(f, "/v/a.mp4", 1, 1, &PhotoMeta { kind: MediaKind::Video, ..Default::default() }).unwrap();
        db.make_private(&[p[1]]).unwrap();
        db.conn().execute("UPDATE photos SET dup = 1 WHERE id = ?1", params![p[2]]).unwrap();
        db.conn().execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![p[3]]).unwrap();
        let got: Vec<PhotoId> = db.tile_candidates(&TileSource::Library).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(got, vec![p[0], p[4]]);
        assert!(!got.contains(&video));
    }

    #[test]
    fn ids_source_keeps_only_eligible_ids() {
        let (db, p) = library(3);
        db.make_private(&[p[1]]).unwrap();
        let got = db.tile_candidates(&TileSource::Ids(vec![p[0], p[1], 9999])).unwrap();
        assert_eq!(got.iter().map(|t| t.id).collect::<Vec<_>>(), vec![p[0]]);
        assert_eq!((got[0].path.as_str(), got[0].mtime, got[0].size), ("/p/0.jpg", 20, 10));
    }

    #[test]
    fn a_long_id_list_works() {
        let (db, p) = library(5_000);
        assert_eq!(db.tile_candidates(&TileSource::Ids(p)).unwrap().len(), 5_000);
    }

    #[test]
    fn a_changed_file_needs_a_new_signature() {
        let (db, p) = library(2);
        let cands = db.tile_candidates(&TileSource::Library).unwrap();
        assert_eq!(db.tiles_without_sig(&cands).unwrap().len(), 2);
        db.set_tile_sigs(&[sig(p[0], 20, 10), sig(p[1], 20, 10)]).unwrap();
        assert!(db.tiles_without_sig(&cands).unwrap().is_empty());
        db.conn().execute("UPDATE photos SET mtime = 99 WHERE id = ?1", params![p[1]]).unwrap();
        let cands = db.tile_candidates(&TileSource::Library).unwrap();
        assert_eq!(db.tiles_without_sig(&cands).unwrap().iter().map(|t| t.id).collect::<Vec<_>>(), vec![p[1]]);
    }

    #[test]
    fn signatures_come_back_in_order_and_round_trip() {
        let (db, p) = library(3);
        db.set_tile_sigs(&[sig(p[0], 20, 10), TileSig { avg: [9, 8, 7], ..sig(p[2], 20, 10) }]).unwrap();
        let got = db.tile_sigs(&[p[2], p[1], p[0]]).unwrap();
        assert_eq!(got.iter().map(|t| t.id).collect::<Vec<_>>(), vec![p[2], p[0]], "in the order asked, none for the photo without one");
        assert_eq!(got[0].avg, [9, 8, 7]);
        assert_eq!(got[1].tiny, vec![7; 432]);
        db.set_tile_sigs(&[TileSig { avg: [5, 5, 5], ..sig(p[0], 20, 10) }]).unwrap();
        assert_eq!(db.tile_sigs(&[p[0]]).unwrap()[0].avg, [5, 5, 5], "a new signature replaces the old");
    }

    #[test]
    fn deleting_a_photo_deletes_its_signature() {
        let (db, p) = library(1);
        db.set_tile_sigs(&[sig(p[0], 20, 10)]).unwrap();
        db.conn().execute("DELETE FROM photos WHERE id = ?1", params![p[0]]).unwrap();
        assert!(db.tile_sigs(&[p[0]]).unwrap().is_empty());
    }

    #[test]
    fn a_signature_for_a_photo_that_is_not_there_is_ignored() {
        let (db, _) = library(1);
        db.set_tile_sigs(&[sig(9999, 1, 1)]).unwrap();
        assert!(db.tile_sigs(&[9999]).unwrap().is_empty());
    }
}
