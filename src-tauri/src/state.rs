use pixie_image::service::ThumbService;
use pixie_index::Db;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub thumbs: ThumbService,
    pub tiles: pixie_geo::TileService,
    pub places: Arc<pixie_geo::PlaceService>,
    pub posters_dir: std::path::PathBuf,
    pub cancel_scan: Arc<AtomicBool>,
    pub gate: pixie_index::ScanGate,
}
