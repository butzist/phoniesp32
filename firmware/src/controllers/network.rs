use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;

use crate::controllers::wifi::{WifiCommand, WifiManager};
use crate::drivers::charger::ChargerMonitor;
use crate::drivers::rfid::RfidHandle;
use crate::drivers::sd::SdFsWrapper;
use crate::player::PlayerHandle;
use crate::radio::Radio;
use crate::services::captive::CaptivePortal;
use crate::services::mdns::MdnsResponder;
use crate::services::web::WebTask;
use crate::DeviceConfig;

pub struct NetworkController {
    radio: Radio,
    wifi_config: Option<DeviceConfig>,
    charger_monitor: ChargerMonitor,
    fs: &'static SdFsWrapper,
    player_handle: PlayerHandle,
    rfid_handle: RfidHandle,
}

impl NetworkController {
    pub fn new(
        radio: Radio,
        wifi_config: Option<DeviceConfig>,
        charger_monitor: ChargerMonitor,
        fs: &'static SdFsWrapper,
        player_handle: PlayerHandle,
        rfid_handle: RfidHandle,
    ) -> Self {
        Self {
            radio,
            wifi_config,
            charger_monitor,
            fs,
            player_handle,
            rfid_handle,
        }
    }

    pub fn spawn(self, spawner: &Spawner) {
        spawner.must_spawn(network_task(
            spawner.clone(),
            self.radio,
            self.wifi_config,
            self.charger_monitor,
            self.fs,
            self.player_handle,
            self.rfid_handle,
        ));
    }
}

#[embassy_executor::task]
async fn network_task(
    spawner: Spawner,
    radio: Radio,
    wifi_config: Option<DeviceConfig>,
    charger_monitor: ChargerMonitor,
    fs: &'static SdFsWrapper,
    player_handle: PlayerHandle,
    rfid_handle: RfidHandle,
) {
    defmt::info!("Network controller: starting radio");
    let (stack, controller, led, is_ap) = radio.spawn(&spawner).await;

    let effective_config = if is_ap { None } else { wifi_config };
    let cmd_channel =
        mk_static!(Channel<NoopRawMutex, WifiCommand, 4>, Channel::new());
    WifiManager::new(controller, led, effective_config).run(
        charger_monitor.clone(),
        cmd_channel,
        &spawner,
    );

    if is_ap {
        defmt::info!("Network controller: starting captive portal");
        let captive = CaptivePortal::new(stack);
        captive.spawn(&spawner);
    }

    defmt::info!("Network controller: starting mDNS responder");
    let mdns = MdnsResponder::new(stack);
    mdns.spawn(&spawner);

    let web_app = crate::services::web::WebApp::default();
    let web_app_state = mk_static!(
        crate::services::web::AppState,
        crate::services::web::AppState::new(fs, player_handle, rfid_handle)
    );

    for id in 0..crate::services::web::WEB_TASK_POOL_SIZE {
        defmt::info!("Network controller: starting web task {}", id);
        let web_task = WebTask::new(id, stack, web_app.router, web_app.config, web_app_state);
        web_task.spawn(&spawner);
    }

    defmt::info!("Network controller: web server started");
}
