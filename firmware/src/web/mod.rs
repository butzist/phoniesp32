use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use embassy_time::Duration;
use esp_alloc as _;
use picoserve::{AppWithStateBuilder, Router, routing};
use serde::{Deserialize, Serialize};

use crate::{entities::audio_file::AudioMetadata, player::PlayerCommand, sd::SdFileSystem};

mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}
mod config;
mod files;
mod fob;
mod playback;
mod upload;

#[derive(Clone)]
pub struct AppState {
    fs: &'static SdFileSystem<'static>,
    commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
}

impl AppState {
    pub fn new(
        fs: &'static SdFileSystem<'static>,
        commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    ) -> Self {
        Self { fs, commands }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Test {
    x: u16,
}

#[derive(Clone, Serialize)]
pub struct FileEntry {
    pub name: heapless::String<8>,
    pub metadata: AudioMetadata,
}

pub struct Application;
impl AppWithStateBuilder for Application {
    type PathRouter = impl routing::PathRouter<AppState>;
    type State = AppState;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, AppState> {
        let router = picoserve::Router::new()
            .route(
                ("/api/files", routing::parse_path_segment()),
                routing::put_service(upload::UploadService).get_service(files::GetMetadataService),
            )
            .route("/api/files", routing::get(files::list))
            .route("/api/last_fob", routing::get(fob::last))
            .route(
                "/api/associations",
                routing::get_service(fob::ListAssociationsService).post(fob::associate),
            )
            .route("/api/playback/status", routing::get(playback::status))
            .route(
                "/api/playback/current_playlist",
                routing::get(playback::current_playlist),
            )
            .route("/api/playback/play", routing::post(playback::play))
            .route("/api/playback/stop", routing::post(playback::stop))
            .route("/api/playback/pause", routing::post(playback::pause))
            .route(
                "/api/playback/volume_up",
                routing::post(playback::volume_up),
            )
            .route(
                "/api/playback/volume_down",
                routing::post(playback::volume_down),
            )
            .route("/api/playback/next", routing::post(playback::next))
            .route("/api/playback/previous", routing::post(playback::previous))
            .route(
                "/api/config",
                routing::put(config::put).delete(config::delete),
            );
        assets::add_asset_routes(router)
    }
}

pub struct WebApp {
    pub router: &'static Router<<Application as AppWithStateBuilder>::PathRouter, AppState>,
    pub config: &'static picoserve::Config<Duration>,
}

impl Default for WebApp {
    fn default() -> Self {
        let router = mk_static!(Router<<Application as AppWithStateBuilder>::PathRouter, AppState>, Application.build_app());

        let config = mk_static!(
            picoserve::Config<Duration>,
            picoserve::Config::new(picoserve::Timeouts {
                start_read_request: None,
                persistent_start_read_request: None,
                read_request: None,
                write: None,
            })
            .keep_connection_alive()
        );

        Self { router, config }
    }
}

pub const WEB_TASK_POOL_SIZE: usize = 4;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    id: usize,
    stack: Stack<'static>,
    router: &'static Router<<Application as AppWithStateBuilder>::PathRouter, AppState>,
    config: &'static picoserve::Config<Duration>,
    state: &'static AppState,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = alloc::vec![0; 1024];
    let mut tcp_tx_buffer = alloc::vec![0; 1024];
    let mut http_buffer = alloc::vec![0; 2048];

    let app_with_state = &router.shared().with_state(state);

    loop {
        let _shutdown_reason = picoserve::Server::new(app_with_state, config, &mut http_buffer)
            .listen_and_serve(id, stack, port, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
            .await;

        // NoGracefulShutdown has no variants, so we always continue
        // This is expected behavior for servers that run forever
    }
}
