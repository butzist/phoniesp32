use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{gpio::Output, peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_wifi::{
    wifi::{self, WifiController, WifiDevice, WifiEvent, WifiState},
    EspWifiController,
};

use crate::{mk_static, DeviceConfig};

pub async fn start_wifi(
    esp_wifi_ctrl: &'static EspWifiController<'static>,
    wifi: WIFI<'static>,
    led: Output<'static>,
    mut rng: Rng,
    config: Option<DeviceConfig>,
    spawner: &Spawner,
) -> Stack<'static> {
    let (controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, wifi).unwrap();
    let wifi_interface = interfaces.sta;
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    spawner.spawn(connection_task(config, controller, led)).ok();
    spawner.spawn(net_task(runner)).ok();

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
) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    let config = config.unwrap(); // FIXME start AP, if config missing or connection
                                  // fails
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            pin.set_high();
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            pin.set_low();
            Timer::after(Duration::from_millis(5000)).await
        }
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

        match controller.connect_async().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {:?}", e);
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
