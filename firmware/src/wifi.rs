use core::net::Ipv4Addr;

use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use enumset::EnumSet;
use esp_hal::{gpio::Output, peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_wifi::{
    wifi::{
        self, AccessPointConfiguration, WifiController, WifiDevice, WifiError, WifiEvent, WifiState,
    },
    EspWifiController,
};
use heapless::Vec;
use static_cell::make_static;

use crate::DeviceConfig;

pub async fn start_wifi(
    esp_wifi_ctrl: &'static EspWifiController<'static>,
    wifi: WIFI<'static>,
    led: Output<'static>,
    mut rng: Rng,
    config: Option<DeviceConfig>,
    spawner: &Spawner,
) -> Stack<'static> {
    let (controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, wifi).unwrap();
    println!("Device capabilities: {:?}", controller.capabilities());

    if let Some(config) = config {
        match start_wifi_sta(controller, led, interfaces.sta, &mut rng, config, spawner).await {
            Ok(stack) => stack,
            Err((_, controller, led)) => {
                start_wifi_ap(controller, led, interfaces.ap, &mut rng, spawner).await
            }
        }
    } else {
        start_wifi_ap(controller, led, interfaces.ap, &mut rng, spawner).await
    }
}

pub async fn start_wifi_ap(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    spawner: &Spawner,
) -> Stack<'static> {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let net_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 42, 1), 24),
        gateway: None,
        dns_servers: Vec::new(),
    });

    // Init network stack
    let (stack, runner) = embassy_net::new(
        device,
        net_config,
        make_static!(StackResources::<3>::new()),
        net_seed,
    );

    let ap_config = AccessPointConfiguration {
        ssid: "phoniesp32".into(),
        password: "12345678".into(),
        channel: 6,
        auth_method: wifi::AuthMethod::WPA2Personal,
        ..Default::default()
    };

    controller
        .set_configuration(&wifi::Configuration::AccessPoint(ap_config))
        .unwrap();
    controller.start_async().await.unwrap();

    spawner.spawn(connection_task(None, controller, led)).ok();
    spawner.spawn(net_task(runner)).ok();

    wait_for_connection(stack).await;

    stack
}

pub async fn start_wifi_sta(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    config: DeviceConfig,
    spawner: &Spawner,
) -> Result<Stack<'static>, (WifiError, WifiController<'static>, Output<'static>)> {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    // Init network stack
    let (stack, runner) = embassy_net::new(
        device,
        net_config,
        make_static!(StackResources::<3>::new()),
        net_seed,
    );

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
    if let Some(err) = last_error {
        controller.stop_async().await.unwrap();
        return Err((err, controller, led));
    }

    spawner
        .spawn(connection_task(Some(config), controller, led))
        .ok();
    spawner.spawn(net_task(runner)).ok();

    wait_for_connection(stack).await;

    Ok(stack)
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
) {
    println!("start connection task");
    let started_states = [WifiState::StaConnected, WifiState::ApStarted];
    let stopped_events = EnumSet::from_iter([WifiEvent::StaDisconnected, WifiEvent::ApStop]);
    loop {
        if started_states.contains(&esp_wifi::wifi::wifi_state()) {
            pin.set_high();
            // wait until we're no longer connected
            controller.wait_for_events(stopped_events, false).await;
            pin.set_low();
            Timer::after(Duration::from_millis(5000)).await
        }
        if let Some(config) = &config {
            wifi_connect(config, &mut controller).await.ok();
        }
    }
}

async fn wifi_connect(
    config: &DeviceConfig,
    controller: &mut WifiController<'static>,
) -> Result<(), WifiError> {
    if !matches!(controller.is_started(), Ok(true)) {
        let client_config = wifi::Configuration::Client(wifi::ClientConfiguration {
            ssid: config.ssid.as_str().into(),
            password: config.password.as_str().into(),
            ..Default::default()
        });
        controller.set_configuration(&client_config).unwrap();
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

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
