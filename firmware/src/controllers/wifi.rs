use embassy_executor::Spawner;
use embassy_futures::select::{Either, select, select3};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use enumset::EnumSet;
use esp_hal::gpio::Output;
use esp_println::println;
use esp_radio::wifi::{
    WifiAccessPointState, WifiController, WifiEvent, WifiStationState,
};

use crate::drivers::charger::ChargerMonitor;
use crate::radio::{wifi_connect, wifi_listen};
use crate::DeviceConfig;

pub enum WifiCommand {
    PowerSave,
    PowerOn,
}

pub struct WifiManager {
    controller: WifiController<'static>,
    led: Output<'static>,
    config: Option<DeviceConfig>,
}

impl WifiManager {
    pub fn new(
        controller: WifiController<'static>,
        led: Output<'static>,
        config: Option<DeviceConfig>,
    ) -> Self {
        Self {
            controller,
            led,
            config,
        }
    }

    pub fn run(
        self,
        charger_monitor: ChargerMonitor,
        cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 4>,
        spawner: &Spawner,
    ) {
        spawner.must_spawn(wifi_manager_task(
            self.config,
            self.controller,
            self.led,
            charger_monitor,
            cmd_channel,
        ));
    }
}

#[embassy_executor::task]
async fn wifi_manager_task(
    config: Option<DeviceConfig>,
    mut controller: WifiController<'static>,
    mut pin: Output<'static>,
    mut charger_monitor: ChargerMonitor,
    rx: &'static Channel<NoopRawMutex, WifiCommand, 4>,
) {
    println!("start wifi manager task");
    let stopped_events = EnumSet::from_iter([
        WifiEvent::StationDisconnected,
        WifiEvent::StationStop,
        WifiEvent::AccessPointStop,
    ]);
    loop {
        let desired_state = charger_monitor.is_connected();
        let actual_state = matches!(controller.is_started(), Ok(true));

        match (desired_state, actual_state) {
            (true, true) => {
                println!("Wifi running");

                let connected = esp_radio::wifi::station_state() == WifiStationState::Connected
                    || esp_radio::wifi::access_point_state() == WifiAccessPointState::Started;
                if connected {
                    pin.set_high();

                    let controller_future = controller.wait_for_events(stopped_events, false);
                    let charger_future = charger_monitor.wait_for_event();
                    let cmd_future = rx.receive();
                    match select3(controller_future, charger_future, cmd_future).await {
                        embassy_futures::select::Either3::First(_) => {
                            // disconnected
                        }
                        embassy_futures::select::Either3::Second(_) => {
                            // charger changed — re-evaluate
                            continue;
                        }
                        embassy_futures::select::Either3::Third(_cmd) => {
                            // command received — re-evaluate
                            continue;
                        }
                    }
                }

                println!("Wifi disconnected");
                controller.stop_async().await.unwrap();
                pin.set_low();
            }
            (true, false) => {
                println!("Wifi needs to be started");

                if let Some(config) = &config {
                    wifi_connect(config, &mut controller).await.ok();
                } else {
                    wifi_listen(&mut controller).await;
                }
            }
            (false, true) => {
                println!("Wifi stopping...");

                controller.stop_async().await.unwrap();
            }
            (false, false) => {
                println!("Wifi off — waiting for charger or command");

                pin.set_low();
                let charger_future = charger_monitor.wait_for_event();
                let cmd_future = rx.receive();
                match select(charger_future, cmd_future).await {
                    Either::First(_) => {}
                    Either::Second(_cmd) => {}
                }
            }
        }
    }
}
