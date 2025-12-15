use core::net::Ipv4Addr;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_net::{DhcpConfig, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use enumset::EnumSet;
use esp_hal::{
    gpio::{AnyPin, Level, Output, OutputConfig},
    peripherals::WIFI,
    rng::Rng,
};
use esp_println::println;
use esp_radio::wifi::{
    self, WifiAccessPointState, WifiController, WifiDevice, WifiError, WifiEvent, WifiStationState,
    ap::AccessPointConfig,
};

use crate::{DeviceConfig, charger::ChargerMonitor, extend_to_static};

const NUM_SOCKETS: usize = crate::web::WEB_TASK_POOL_SIZE + 3;

pub struct Radio {
    wifi: WIFI<'static>,
    led: AnyPin<'static>,
    config: Option<DeviceConfig>,
}

impl Radio {
    pub fn new(wifi: WIFI<'static>, led: AnyPin<'static>, config: Option<DeviceConfig>) -> Self {
        Self { wifi, led, config }
    }

    pub async fn spawn(self, charger_monitor: ChargerMonitor, spawner: &Spawner) -> Stack<'static> {
        let mut rng = esp_hal::rng::Rng::new();
        let wifi_led = Output::new(self.led, Level::Low, OutputConfig::default());

        let (controller, interfaces) = esp_radio::wifi::new(self.wifi, Default::default()).unwrap();
        println!("Device capabilities: {:?}", controller.capabilities());

        let stack_resources = mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());

        // try to connect to AP if credentials are configured
        if let Some(config) = self.config {
            match try_start_wifi_sta(
                controller,
                wifi_led,
                interfaces.station,
                &mut rng,
                // SAFETY: will only use stack resources if connection is successful
                unsafe { extend_to_static(stack_resources) },
                config.clone(),
                charger_monitor.clone(),
                spawner,
            )
            .await
            {
                Ok(stack) => {
                    return stack;
                }
                Err((controller, led)) => {
                    // Fall through to AP mode with the returned controller and led
                    return start_wifi_ap(
                        controller,
                        led,
                        interfaces.access_point,
                        &mut rng,
                        stack_resources,
                        charger_monitor,
                        spawner,
                    )
                    .await;
                }
            }
        }

        // Start in AP mode if no config
        start_wifi_ap(
            controller,
            wifi_led,
            interfaces.access_point,
            &mut rng,
            stack_resources,
            charger_monitor,
            spawner,
        )
        .await
    }
}

async fn try_start_wifi_sta(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    stack_resources: &'static mut StackResources<NUM_SOCKETS>,
    config: DeviceConfig,
    charger_monitor: ChargerMonitor,
    spawner: &Spawner,
) -> Result<Stack<'static>, (WifiController<'static>, Output<'static>)> {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    // Init network stack
    let (stack, runner) = embassy_net::new(device, net_config, stack_resources, net_seed);

    let mut last_error = None;
    for _ in 0..5 {
        match wifi_connect(&config, &mut controller).await {
            Ok(_) => {
                last_error = None;
                break;
            }
            Err(err) => last_error = Some(err),
        }
    }

    if last_error.is_some() {
        controller.stop_async().await.unwrap();
        return Err((controller, led));
    }

    spawner
        .spawn(connection_task(
            Some(config),
            controller,
            led,
            charger_monitor,
        ))
        .unwrap();
    spawner.spawn(net_task(runner)).unwrap();
    wait_for_connection(stack).await;

    Ok(stack)
}

pub async fn start_wifi_ap(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    stack_resources: &'static mut StackResources<NUM_SOCKETS>,
    charger_monitor: ChargerMonitor,
    spawner: &Spawner,
) -> Stack<'static> {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let net_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 42, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    });

    // Init network stack
    let (stack, runner) = embassy_net::new(device, net_config, stack_resources, net_seed);

    wifi_listen(&mut controller).await;

    spawner
        .spawn(connection_task(None, controller, led, charger_monitor))
        .unwrap();
    spawner.spawn(net_task(runner)).unwrap();

    wait_for_connection(stack).await;

    stack
}

async fn wait_for_connection(stack: Stack<'_>) {
    println!("Waiting for link to be up");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn connection_task(
    config: Option<DeviceConfig>,
    mut controller: WifiController<'static>,
    mut pin: Output<'static>,
    mut charger_monitor: ChargerMonitor,
) {
    println!("start connection task");
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

                    // wait until we're no longer connected
                    let controller_future = controller.wait_for_events(stopped_events, false);
                    let charger_future = charger_monitor.wait_for_event();
                    match select(controller_future, charger_future).await {
                        Either::First(_) => {
                            // disconnected
                        }
                        Either::Second(_) => {
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
                println!("Wifi off - waiting for charger plugged in");

                pin.set_low();
                charger_monitor.wait_for_event().await;
            }
        }
    }
}

async fn wifi_connect(
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
        println!("Starting wifi");
        controller.start_async().await.unwrap();
        println!("Wifi started!");
    }
    println!("About to connect...");

    let connect_result = controller.connect_async().await;
    match connect_result {
        Ok(_) => {
            println!("Wifi connected!");
        }
        Err(e) => {
            println!("Failed to connect to wifi: {:?}", e);
            Timer::after(Duration::from_millis(5000)).await;
        }
    }

    connect_result
}

async fn wifi_listen(controller: &mut WifiController<'static>) {
    if !matches!(controller.is_started(), Ok(true)) {
        let ap_config = AccessPointConfig::default()
            .with_ssid(alloc::string::String::from("phoniesp32"))
            .with_password(alloc::string::String::from("12345678"))
            .with_channel(6)
            .with_auth_method(wifi::AuthMethod::Wpa2Personal);

        controller
            .set_config(&wifi::ModeConfig::AccessPoint(ap_config))
            .unwrap();

        println!("Starting wifi AP");
        controller.start_async().await.unwrap();
        println!("Wifi AP started!");
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
