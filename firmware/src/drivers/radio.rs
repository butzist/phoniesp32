use core::net::Ipv4Addr;

use defmt::{debug, info, warn};
use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use enumset::EnumSet;
use esp_hal::rng::Rng;
use esp_radio::wifi::WifiEvent;
use esp_radio::wifi::{self, WifiController, WifiDevice, WifiError, ap::AccessPointConfig};

use crate::DeviceConfig;

const NUM_SOCKETS: usize = crate::services::web::WEB_TASK_POOL_SIZE + 5;

pub struct Radio {
    wifi: esp_hal::peripherals::WIFI<'static>,
}

impl Radio {
    pub fn new(wifi: esp_hal::peripherals::WIFI<'static>) -> Self {
        Self { wifi }
    }

    pub async fn spawn(self, spawner: &Spawner) -> RadioHandle {
        let (controller, interfaces) = esp_radio::wifi::new(self.wifi, Default::default()).unwrap();
        info!(
            "Radio: device capabilities: {:?}",
            controller.capabilities()
        );

        RadioHandle {
            controller,
            station_device: Some(interfaces.station),
            ap_device: Some(interfaces.access_point),
            stack: None,
            mode: WifiMode::Uninitialized,
            spawner: *spawner,
        }
    }
}

enum WifiMode {
    Uninitialized,
    Sta(DeviceConfig),
    Ap,
}

pub struct RadioHandle {
    controller: WifiController<'static>,
    station_device: Option<WifiDevice<'static>>,
    ap_device: Option<WifiDevice<'static>>,
    stack: Option<Stack<'static>>,
    mode: WifiMode,
    spawner: Spawner,
}

impl RadioHandle {
    pub async fn try_start_wifi_sta(&mut self, config: &DeviceConfig) -> Result<(), ()> {
        let device = self.station_device.take().ok_or(())?;
        let stack_resources = mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());

        let rng = Rng::new();
        let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

        let dhcp_config = DhcpConfig::default();
        let net_config = embassy_net::Config::dhcpv4(dhcp_config);

        let (stack, runner) = embassy_net::new(device, net_config, stack_resources, net_seed);

        let mut last_error = None;
        for _ in 0..5 {
            match wifi_connect(config, &mut self.controller).await {
                Ok(_) => {
                    last_error = None;
                    break;
                }
                Err(err) => last_error = Some(err),
            }
        }

        if let Some(_err) = last_error {
            self.controller.stop_async().await.unwrap();
            return Err(());
        }

        self.spawner.spawn(net_task(runner)).unwrap();
        self.stack = Some(stack);
        self.mode = WifiMode::Sta(config.clone());
        Ok(())
    }

    pub async fn start_wifi_ap(&mut self) {
        let device = self.ap_device.take().unwrap();
        let stack_resources = mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());

        let rng = Rng::new();
        let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

        let net_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 42, 1), 24),
            gateway: None,
            dns_servers: Default::default(),
        });

        let (stack, runner) = embassy_net::new(device, net_config, stack_resources, net_seed);

        wifi_listen(&mut self.controller).await;

        self.spawner.spawn(net_task(runner)).unwrap();
        self.stack = Some(stack);
        self.mode = WifiMode::Ap;
    }

    pub async fn set_wifi_enabled(&mut self, enabled: bool) {
        if enabled {
            match &self.mode {
                WifiMode::Sta(config) => {
                    wifi_connect(config, &mut self.controller).await.ok();
                }
                WifiMode::Ap => {
                    wifi_listen(&mut self.controller).await;
                }
                WifiMode::Uninitialized => {}
            }
        } else {
            self.controller.stop_async().await.unwrap();
        }
    }

    pub fn is_started(&self) -> bool {
        matches!(self.controller.is_started(), Ok(true))
    }

    pub async fn wait_for_disconnect(&mut self) {
        let events = EnumSet::from_iter([
            WifiEvent::StationDisconnected,
            WifiEvent::StationStop,
            WifiEvent::AccessPointStop,
        ]);
        self.controller.wait_for_events(events, false).await;
    }

    pub fn stack(&self) -> Option<Stack<'static>> {
        self.stack
    }
}

pub async fn wifi_connect(
    config: &DeviceConfig,
    controller: &mut WifiController<'static>,
) -> Result<(), WifiError> {
    if !matches!(controller.is_started(), Ok(true)) {
        let station_config = wifi::sta::StationConfig::default()
            .with_ssid(config.ssid.clone())
            .with_password(config.password.clone());

        controller
            .set_config(&wifi::ModeConfig::Station(station_config))
            .unwrap();
        info!("Radio: starting WiFi");
        controller.start_async().await.unwrap();
        info!("Radio: WiFi started!");
    }
    debug!("Radio: connecting to SSID: {}", config.ssid.as_str());
    info!("Radio: about to connect...");

    let connect_result = controller.connect_async().await;
    match connect_result {
        Ok(_) => {
            info!("Radio: WiFi connected!");
        }
        Err(e) => {
            warn!("Radio: failed to connect to WiFi: {:?}", e);
            Timer::after(Duration::from_millis(5000)).await;
        }
    }

    connect_result
}

pub async fn wifi_listen(controller: &mut WifiController<'static>) {
    if !matches!(controller.is_started(), Ok(true)) {
        let ap_config = AccessPointConfig::default()
            .with_ssid(alloc::string::String::from("phoniesp32"))
            .with_password(alloc::string::String::from("12345678"))
            .with_channel(6)
            .with_auth_method(wifi::AuthMethod::Wpa2Personal);

        controller
            .set_config(&wifi::ModeConfig::AccessPoint(ap_config))
            .unwrap();

        info!("Radio: starting WiFi AP");
        controller.start_async().await.unwrap();
        info!("Radio: WiFi AP started!");
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
