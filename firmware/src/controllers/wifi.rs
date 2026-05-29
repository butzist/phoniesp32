use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either4, select, select4};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use esp_radio::wifi::{WifiAccessPointState, WifiStationState};

use crate::DeviceConfig;
use crate::drivers::charger::ChargerMonitor;
use crate::drivers::indicator::IndicatorLedHandle;
use crate::drivers::radio::RadioHandle;

pub enum WifiCommand {
    WifiOff,
    WifiOn,
}

#[derive(Clone)]
pub struct WifiManagerHandle {
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 1>,
    stack_signal: &'static Signal<NoopRawMutex, Stack<'static>>,
}

impl WifiManagerHandle {
    pub async fn wifi_on(&self) {
        self.cmd_channel.send(WifiCommand::WifiOn).await;
    }

    pub async fn wifi_off(&self) {
        self.cmd_channel.send(WifiCommand::WifiOff).await;
    }

    pub async fn wait_for_stack(&self) -> Stack<'static> {
        self.stack_signal.wait().await
    }
}

pub struct WifiManager {
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 1>,
    stack_signal: &'static Signal<NoopRawMutex, Stack<'static>>,
}

impl WifiManager {
    pub fn new() -> Self {
        let cmd_channel = mk_static!(Channel<NoopRawMutex, WifiCommand, 1>, Channel::new());
        let stack_signal = mk_static!(Signal<NoopRawMutex, Stack<'static>>, Signal::new());
        Self {
            cmd_channel,
            stack_signal,
        }
    }

    pub fn spawn(
        self,
        spawner: &Spawner,
        radio_handle: RadioHandle,
        led_handle: IndicatorLedHandle,
        config: Option<DeviceConfig>,
        charger_monitor: ChargerMonitor,
    ) -> WifiManagerHandle {
        spawner.must_spawn(wifi_manager_task(
            radio_handle,
            led_handle,
            config,
            charger_monitor,
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
    cmd_channel: &'static Channel<NoopRawMutex, WifiCommand, 1>,
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

    let mut override_until: Option<Instant> = None;

    loop {
        let charger_state = charger_monitor.is_connected();
        let has_override = override_until.map(|t| Instant::now() < t).unwrap_or(false);
        if let Some(t) = override_until
            && Instant::now() >= t
        {
            info!("WifiManager: powersave override expired");
            override_until = None;
        }
        let desired_state = charger_state || has_override;
        let actual_state = radio_handle.is_started();

        match (desired_state, actual_state) {
            (true, true) => {
                info!("WifiManager: connected and running");

                let connected = esp_radio::wifi::station_state() == WifiStationState::Connected
                    || esp_radio::wifi::access_point_state() == WifiAccessPointState::Started;
                if connected {
                    led_handle.on().await;

                    let controller_future = radio_handle.wait_for_disconnect();
                    let charger_future = charger_monitor.wait_for_event();
                    let cmd_future = cmd_channel.receive();
                    let check_future = Timer::after(Duration::from_secs(10));
                    match select4(controller_future, charger_future, cmd_future, check_future).await
                    {
                        Either4::First(_) => {
                            info!("WifiManager: disconnected");
                            radio_handle.set_wifi_enabled(false).await;
                            led_handle.off().await;
                        }
                        Either4::Second(_) => {}
                        Either4::Third(cmd) => {
                            match cmd {
                                WifiCommand::WifiOn => {
                                    info!("WifiManager: override on — 10 min");
                                    override_until =
                                        Some(Instant::now() + Duration::from_secs(600));
                                }
                                WifiCommand::WifiOff => {
                                    info!("WifiManager: override off — immediate stop");
                                    override_until = None;
                                }
                            };
                        }
                        Either4::Fourth(_) => {} // periodic check tick
                    }
                }
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
                    Either::Second(cmd) => match cmd {
                        WifiCommand::WifiOn => {
                            info!("WifiManager: override on — 10 min");
                            override_until = Some(Instant::now() + Duration::from_secs(600));
                        }
                        WifiCommand::WifiOff => {}
                    },
                }
            }
        }
    }
}
