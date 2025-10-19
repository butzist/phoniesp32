#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, gpio::Level};
use esp_hal::{dma_buffers_chunk_size, spi};
use esp_wifi::ble::controller::BleConnector;
use firmware::controls::{AnyTouchPin, Controls};
use firmware::player::{Player, PlayerCommand};
use firmware::sd::init_sd;
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

    esp_alloc::heap_allocator!(size: 48 * 1024);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG1);

    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    let spi_bus = {
        let dma_channel = peripherals.DMA_SPI2;
        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
            dma_buffers_chunk_size!(4 * 1024, 1024);

        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        make_static!(Spi::new(
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
        .into_async())
    };

    let sd_cs = make_static!(Output::new(
        peripherals.GPIO5,
        Level::High,
        OutputConfig::default()
    ));
    let (device_config, fs) = make_static!(init_sd(spi_bus, sd_cs).await);
    info!("Config: {:?}", &device_config);

    if false {
        let rng = esp_hal::rng::Rng::new(peripherals.RNG);
        let timer1 = TimerGroup::new(peripherals.TIMG0);
        let wifi_init =
            make_static!(esp_wifi::init(timer1.timer0, rng)
                .expect("Failed to initialize WIFI/BLE controller"));

        let _connector = BleConnector::new(wifi_init, peripherals.BT);

        let wifi_led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
        let stack =
            firmware::wifi::start_wifi(wifi_init, peripherals.WIFI, wifi_led, rng, None, &spawner)
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
    }

    let player = Player::new(
        peripherals.I2S0.into(),
        peripherals.DMA_I2S0.into(),
        peripherals.GPIO27.into(),
        peripherals.GPIO26.into(),
        peripherals.GPIO25.into(),
    );
    let commands = make_static!(Channel::<NoopRawMutex, PlayerCommand, 2>::new());
    spawner.must_spawn(firmware::player::run_player(
        spawner,
        player,
        fs,
        commands.receiver(),
    ));

    let controls = Controls::new(
        peripherals.LPWR,
        peripherals.TOUCH,
        AnyTouchPin::GPIO15(peripherals.GPIO15),
        AnyTouchPin::GPIO4(peripherals.GPIO4),
        AnyTouchPin::GPIO33(peripherals.GPIO33),
        AnyTouchPin::GPIO32(peripherals.GPIO32), // FIXME: GPIO32 touch does not work
    );

    spawner.must_spawn(firmware::controls::run_controls(
        controls,
        commands.sender(),
    ));

    loop {
        Timer::after_secs(1).await;
    }
}
