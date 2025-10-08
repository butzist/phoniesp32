#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use alloc::vec::Vec;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{LfnBuffer, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, gpio::Level};
use esp_hal::{spi, Async};
use esp_wifi::{ble::controller::BleConnector, EspWifiController};
use firmware::{mk_static, DeviceConfig};
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    let mut spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(400))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO18)
    .with_mosi(peripherals.GPIO23)
    .with_miso(peripherals.GPIO19)
    .into_async();
    let mut sd_cs = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let device_config = init_sd(&mut spi_bus, &mut sd_cs);
    info!("Config: {:?}", &device_config);

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let wifi_init = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng).expect("Failed to initialize WIFI/BLE controller")
    );
    let _connector = BleConnector::new(wifi_init, peripherals.BT);

    let wifi_led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let stack = firmware::wifi::start_wifi(
        wifi_init,
        peripherals.WIFI,
        wifi_led,
        rng,
        device_config,
        &spawner,
    )
    .await;

    let web_app = firmware::web::WebApp::default();
    for id in 0..firmware::web::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(firmware::web::web_task(
            id,
            stack,
            web_app.router,
            web_app.config,
        ));
    }
    info!("Web server started...");

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}

/// Code from https://github.com/rp-rs/rp-hal-boards/blob/main/boards/rp-pico/examples/pico_spi_sd_card.rs
/// A dummy timesource, which is mostly important for creating files.
#[derive(Default)]
pub struct DummyTimesource();

impl TimeSource for DummyTimesource {
    // In theory you could use the RTC of the rp2040 here, if you had
    // any external time synchronizing device.
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

fn init_sd(spi_bus: &mut Spi<'static, Async>, sd_cs: &mut Output<'static>) -> Option<DeviceConfig> {
    let spi_dev = ExclusiveDevice::new(spi_bus, sd_cs, Delay).unwrap();

    let sdcard = SdCard::new(spi_dev, Delay);
    let sd_size = sdcard.num_bytes().unwrap();
    info!("Card size is {} bytes\r\n", sd_size);

    sdcard
        .spi(|spi| {
            spi.bus_mut().apply_config(
                &spi::master::Config::default()
                    .with_frequency(Rate::from_mhz(8))
                    .with_mode(spi::Mode::_0),
            )
        })
        .unwrap();
    let volume_mgr = VolumeManager::new(sdcard, DummyTimesource::default());
    let volume0 = volume_mgr.open_volume(VolumeIdx(0)).unwrap();
    let root_dir = volume0.open_root_dir().unwrap();

    let mut buffer = [0; 128];
    let mut lfn_buffer = LfnBuffer::new(&mut buffer);
    root_dir
        .iterate_dir_lfn(&mut lfn_buffer, |f, s| info!("{} - {}", f.name, s))
        .unwrap();

    let config_file = root_dir
        .open_file_in_dir("CONFIG~1.JSO", Mode::ReadOnly)
        .ok();

    if let Some(config_file) = config_file {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 128];
        while !config_file.is_eof() {
            let n = config_file.read(&mut buffer).unwrap();
            bytes.extend(&buffer[..n]);
        }

        serde_json::from_slice(&bytes).ok()
    } else {
        None
    }
}
