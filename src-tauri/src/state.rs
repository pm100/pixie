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
    /// Face pictures already cut (see `pixie_analysis::people::cached_face`).
    pub faces_dir: std::path::PathBuf,
    pub secrets: Arc<dyn pixie_mail::SecretStore>,
    pub mailer: Arc<dyn pixie_mail::Mailer>,
    pub cancel_scan: Arc<AtomicBool>,
    pub gate: pixie_index::ScanGate,
    pub analysis: Arc<pixie_analysis::service::AnalysisService>,
    pub people: Arc<pixie_analysis::people_service::PeopleService>,
}
