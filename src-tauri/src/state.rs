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
    /// Folders added while a scan job runs; the job takes them.
    pub queue: pixie_index::ScanQueue,
    pub analysis: Arc<pixie_analysis::service::AnalysisService>,
    pub people: Arc<pixie_analysis::people_service::PeopleService>,
    /// Reads, hashes, decodes and analyses every photo once. Only kept alive here: the services drive it.
    #[allow(dead_code)]
    pub pipeline: Arc<pixie_analysis::pipeline::PhotoPipeline>,
    /// Hashes files in the background so copies can be found; stops when the app state is dropped.
    pub dupes: Arc<pixie_index::DupeService>,
}
