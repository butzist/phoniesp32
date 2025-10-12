#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use alloc::format;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, gpio::Level};
use esp_hal::{dma_buffers_chunk_size, spi};
use esp_wifi::ble::controller::BleConnector;
use firmware::sd::init_sd;
use heapless::String;
use mfrc522::comm::blocking::spi::SpiInterface;
use mfrc522::Mfrc522;
use static_cell::make_static;
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

    let dma_channel = peripherals.DMA_SPI2;
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
        dma_buffers_chunk_size!(4092, 1024);

    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let mut sd_spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(400))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO18)
    .with_mosi(peripherals.GPIO23)
    .with_miso(peripherals.GPIO19)
    .with_dma(dma_channel)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let mut sd_cs = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let device_config = init_sd(&mut sd_spi_bus, &mut sd_cs).await;
    info!("Config: {:?}", &device_config);

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let wifi_init = &*make_static!(
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

    let rfid_spi_bus = Spi::new(
        peripherals.SPI3,
        spi::master::Config::default()
            .with_frequency(Rate::from_mhz(5))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO14)
    .with_mosi(peripherals.GPIO13)
    .with_miso(peripherals.GPIO12);

    let rfid_cs = Output::new(peripherals.GPIO21, Level::High, OutputConfig::default());

    let spi_dev = ExclusiveDevice::new(rfid_spi_bus, rfid_cs, Delay).unwrap();

    let spi_interface = SpiInterface::new(spi_dev);
    let mut rfid = Mfrc522::new(spi_interface).init().unwrap();

    loop {
        if let Ok(atqa) = rfid.reqa() {
            info!("Answer To reQuest code A");
            Timer::after(Duration::from_millis(50)).await;
            if let Ok(uid) = rfid.select(&atqa) {
                print_hex_bytes(uid.as_bytes());
            }
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}

fn print_hex_bytes(data: &[u8]) {
    let mut output = String::<32>::new();
    for &b in data.iter() {
        let byte = format!("{:02x} ", b);
        output.push_str(&byte).unwrap();
    }
    info!("{}", output);
}
