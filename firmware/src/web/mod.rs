use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use embassy_time::Duration;
use esp_alloc as _;
use picoserve::{
    response::Redirect,
    routing::{self},
    AppRouter, AppWithStateBuilder, Router,
};
use serde::{Deserialize, Serialize};

use crate::{player::PlayerCommand, sd::SdFileSystem};

mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}
mod config;
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

pub struct Application;
impl AppWithStateBuilder for Application {
    type PathRouter = impl routing::PathRouter<AppState>;
    type State = AppState;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, AppState> {
        let router = picoserve::Router::new()
            .route("/", routing::get(|| async { Redirect::to("index.html") }))
            .route(
                ("/api/files", routing::parse_path_segment()),
                routing::put_service(upload::UploadService),
            )
            .route(
                ("/api/files", routing::parse_path_segment(), "info"),
                routing::put(upload::put_info),
            )
            .route("/api/last_fob", routing::get(fob::last))
            .route("/api/associations", routing::post(fob::associate))
            .route("/api/playback/status", routing::get(playback::status))
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
            .route("/api/config", routing::put(config::put))
            .route("/api/config", routing::delete(config::delete));
        assets::add_asset_routes(router)
    }
}

pub struct WebApp {
    pub router: &'static Router<<Application as AppWithStateBuilder>::PathRouter, AppState>,
    pub config: &'static picoserve::Config<Duration>,
}

impl Default for WebApp {
    fn default() -> Self {
        let router = picoserve::make_static!(AppRouter<Application>, Application.build_app());

        let config = picoserve::make_static!(
            picoserve::Config<Duration>,
            picoserve::Config::new(picoserve::Timeouts {
                start_read_request: Some(Duration::from_secs(5)),
                persistent_start_read_request: Some(Duration::from_secs(5)),
                read_request: Some(Duration::from_secs(1)),
                write: Some(Duration::from_secs(1)),
            })
            .keep_connection_alive()
        );

        Self { router, config }
    }
}

#[embassy_executor::task]
pub async fn web_task(
    stack: Stack<'static>,
    router: &'static AppRouter<Application>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = alloc::vec![0; 1024];
    let mut tcp_tx_buffer = alloc::vec![0; 1024];
    let mut http_buffer = alloc::vec![0; 2048];

    picoserve::listen_and_serve_with_state(
        0,
        router,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
}
