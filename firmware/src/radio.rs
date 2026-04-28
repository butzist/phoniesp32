use core::net::Ipv4Addr;

use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_hal::{
    gpio::{AnyPin, Level, Output, OutputConfig},
    peripherals::WIFI,
    rng::Rng,
};
use esp_println::println;
use esp_radio::wifi::{
    self, WifiController, WifiDevice, WifiError,
    ap::AccessPointConfig,
};

use crate::DeviceConfig;

const NUM_SOCKETS: usize = crate::services::web::WEB_TASK_POOL_SIZE + 5;

pub struct Radio {
    wifi: WIFI<'static>,
    led: AnyPin<'static>,
    config: Option<DeviceConfig>,
}

impl Radio {
    pub fn new(wifi: WIFI<'static>, led: AnyPin<'static>, config: Option<DeviceConfig>) -> Self {
        Self { wifi, led, config }
    }

    pub async fn spawn(
        self,
        spawner: &Spawner,
    ) -> (
        Stack<'static>,
        WifiController<'static>,
        Output<'static>,
        bool,
    ) {
        let mut rng = Rng::new();
        let wifi_led = Output::new(self.led, Level::Low, OutputConfig::default());

        let (mut controller, interfaces) =
            esp_radio::wifi::new(self.wifi, Default::default()).unwrap();
        println!("Device capabilities: {:?}", controller.capabilities());

        if let Some(config) = self.config {
            let stack_resources =
                mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());
            match try_start_wifi_sta(
                controller,
                wifi_led,
                interfaces.station,
                &mut rng,
                stack_resources,
                config.clone(),
                spawner,
            )
            .await
            {
                Ok((stack, controller, led)) => {
                    return (stack, controller, led, false);
                }
                Err((ctrl, led)) => {
                    controller = ctrl;
                    let stack_resources =
                        mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());
                    let (stack, ctrl, led) = start_wifi_ap(
                        controller,
                        led,
                        interfaces.access_point,
                        &mut rng,
                        stack_resources,
                        spawner,
                    )
                    .await;
                    return (stack, ctrl, led, true);
                }
            }
        }

        let stack_resources = mk_static!(StackResources::<NUM_SOCKETS>, StackResources::new());
        let (stack, ctrl, led) = start_wifi_ap(
            controller,
            wifi_led,
            interfaces.access_point,
            &mut rng,
            stack_resources,
            spawner,
        )
        .await;
        (stack, ctrl, led, true)
    }
}

async fn try_start_wifi_sta(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    stack_resources: &'static mut StackResources<NUM_SOCKETS>,
    config: DeviceConfig,
    spawner: &Spawner,
) -> Result<
    (Stack<'static>, WifiController<'static>, Output<'static>),
    (WifiController<'static>, Output<'static>),
> {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

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

    spawner.spawn(net_task(runner)).unwrap();

    Ok((stack, controller, led))
}

async fn start_wifi_ap(
    mut controller: WifiController<'static>,
    led: Output<'static>,
    device: WifiDevice<'static>,
    rng: &mut Rng,
    stack_resources: &'static mut StackResources<NUM_SOCKETS>,
    spawner: &Spawner,
) -> (
    Stack<'static>,
    WifiController<'static>,
    Output<'static>,
) {
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let net_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Addr::new(192, 168, 42, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    });

    let (stack, runner) = embassy_net::new(device, net_config, stack_resources, net_seed);

    wifi_listen(&mut controller).await;

    spawner.spawn(net_task(runner)).unwrap();

    (stack, controller, led)
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

        println!("Starting wifi AP");
        controller.start_async().await.unwrap();
        println!("Wifi AP started!");
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
