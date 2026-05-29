use defmt::{debug, info};
use embassy_executor::Spawner;

use crate::controllers::playback::PlaybackHandle;
use crate::controllers::wifi::WifiManagerHandle;
use crate::drivers::rfid::RfidHandle;
use crate::drivers::sd::SdFsWrapper;
use crate::services::captive::CaptivePortal;
use crate::services::mdns::MdnsResponder;
use crate::services::web::WebTask;

pub struct NetworkController;

impl NetworkController {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn(
        self,
        spawner: &Spawner,
        wifi_handle: WifiManagerHandle,
        fs: &'static SdFsWrapper,
        player_handle: PlaybackHandle,
        rfid_handle: RfidHandle,
    ) {
        spawner.must_spawn(network_task(
            *spawner,
            wifi_handle,
            fs,
            player_handle,
            rfid_handle,
        ));
    }
}

#[embassy_executor::task]
async fn network_task(
    spawner: Spawner,
    wifi_handle: WifiManagerHandle,
    fs: &'static SdFsWrapper,
    player_handle: PlaybackHandle,
    rfid_handle: RfidHandle,
) {
    debug!("NetworkController: waiting for WiFi stack");
    let stack = wifi_handle.wait_for_stack().await;

    debug!("NetworkController: setting up services");
    let is_ap = stack
        .config_v4()
        .map(|c| c.address.address() == core::net::Ipv4Addr::new(192, 168, 42, 1))
        .unwrap_or(false);

    if is_ap {
        info!("NetworkController: starting CaptivePortal");
        let captive = CaptivePortal::new(stack);
        captive.spawn(&spawner);
    }

    info!("NetworkController: starting mDNS responder");
    let mdns = MdnsResponder::new(stack);
    mdns.spawn(&spawner);

    let web_app = crate::services::web::WebApp::default();
    let web_app_state = mk_static!(
        crate::services::web::AppState,
        crate::services::web::AppState::new(fs, player_handle, rfid_handle, wifi_handle.clone())
    );

    for id in 0..crate::services::web::WEB_TASK_POOL_SIZE {
        info!("NetworkController: starting web task {}", id);
        let web_task = WebTask::new(id, stack, web_app.router, web_app.config, web_app_state);
        web_task.spawn(&spawner);
    }

    info!("NetworkController: web server started");
}
