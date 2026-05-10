use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select, select3};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use esp_radio::wifi::{WifiAccessPointState, WifiStationState};

use crate::DeviceConfig;
use crate::drivers::charger::ChargerMonitor;
use crate::drivers::indicator::IndicatorLedHandle;
use crate::drivers::radio::RadioHandle;

pub enum WifiCommand {
    PowerSave,
    PowerOn,
}

#[derive(Clone)]
pub struct WifiManagerHandle {
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 4>,
    stack_signal: &'static Signal<NoopRawMutex, Stack<'static>>,
}

impl WifiManagerHandle {
    pub async fn power_on(&self) {
        self.cmd_channel.send(WifiCommand::PowerOn).await;
    }

    pub async fn power_save(&self) {
        self.cmd_channel.send(WifiCommand::PowerSave).await;
    }

    pub async fn wait_for_stack(&self) -> Stack<'static> {
        self.stack_signal.wait().await
    }
}

pub struct WifiManager {
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 4>,
    stack_signal: &'static Signal<NoopRawMutex, Stack<'static>>,
    radio_handle: RadioHandle,
    led_handle: IndicatorLedHandle,
    config: Option<DeviceConfig>,
    charger_monitor: ChargerMonitor,
}

impl WifiManager {
    pub fn new(
        radio_handle: RadioHandle,
        led_handle: IndicatorLedHandle,
        config: Option<DeviceConfig>,
        charger_monitor: ChargerMonitor,
    ) -> Self {
        let cmd_channel = mk_static!(Channel<NoopRawMutex, WifiCommand, 4>, Channel::new());
        let stack_signal = mk_static!(Signal<NoopRawMutex, Stack<'static>>, Signal::new());
        Self {
            cmd_channel,
            stack_signal,
            radio_handle,
            led_handle,
            config,
            charger_monitor,
        }
    }

    pub fn spawn(self, spawner: &Spawner) -> WifiManagerHandle {
        spawner.must_spawn(wifi_manager_task(
            self.radio_handle,
            self.led_handle,
            self.config,
            self.charger_monitor,
            self.cmd_channel,
            self.stack_signal,
        ));
        WifiManagerHandle {
            cmd_channel: self.cmd_channel,
            stack_signal: self.stack_signal,
        }
    }
}

#[embassy_executor::task]
async fn wifi_manager_task(
    mut radio_handle: RadioHandle,
    led_handle: IndicatorLedHandle,
    config: Option<DeviceConfig>,
    mut charger_monitor: ChargerMonitor,
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 4>,
    stack_signal: &'static Signal<NoopRawMutex, Stack<'static>>,
) {
    info!("WifiManager: initial startup");

    if let Some(ref config) = config {
        if radio_handle.try_start_wifi_sta(config).await.is_err() {
            radio_handle.start_wifi_ap().await;
        }
    } else {
        radio_handle.start_wifi_ap().await;
    }

    let stack = radio_handle.stack().unwrap();
    stack_signal.signal(stack);

    info!("WifiManager: task started");
    loop {
        let desired_state = charger_monitor.is_connected();
        let actual_state = radio_handle.is_started();

        match (desired_state, actual_state) {
            (true, true) => {
                info!("WifiManager: connected and running");

                let connected = esp_radio::wifi::station_state() == WifiStationState::Connected
                    || esp_radio::wifi::access_point_state() == WifiAccessPointState::Started;
                if connected {
                    led_handle.on().await;

                    let controller_future = radio_handle.wait_for_stopped();
                    let charger_future = charger_monitor.wait_for_event();
                    let cmd_future = cmd_channel.receive();
                    match select3(controller_future, charger_future, cmd_future).await {
                        embassy_futures::select::Either3::First(_) => {}
                        embassy_futures::select::Either3::Second(_) => {
                            continue;
                        }
                        embassy_futures::select::Either3::Third(_cmd) => {
                            continue;
                        }
                    }
                }

                info!("WifiManager: disconnected");
                radio_handle.set_wifi_enabled(false).await;
                led_handle.off().await;
            }
            (true, false) => {
                info!("WifiManager: charger connected — starting WiFi");
                radio_handle.set_wifi_enabled(true).await;
            }
            (false, true) => {
                info!("WifiManager: charger disconnected — stopping WiFi");
                radio_handle.set_wifi_enabled(false).await;
            }
            (false, false) => {
                info!("WifiManager: off — waiting for charger or command");

                led_handle.off().await;
                let charger_future = charger_monitor.wait_for_event();
                let cmd_future = cmd_channel.receive();
                match select(charger_future, cmd_future).await {
                    Either::First(_) => {}
                    Either::Second(_cmd) => {}
                }
            }
        }
    }
}
