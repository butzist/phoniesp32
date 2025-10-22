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
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use firmware::controls::{AnyTouchPin, Controls};
use firmware::player::{Player, PlayerCommand};
use firmware::radio::Radio;
use firmware::sd::Sd;
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

    let sd = Sd::new(
        peripherals.SPI2.into(),
        peripherals.DMA_SPI2.into(),
        peripherals.GPIO18.into(),
        peripherals.GPIO23.into(),
        peripherals.GPIO19.into(),
        peripherals.GPIO5.into(),
    );

    let (device_config, fs) = sd.init().await;
    let fs = make_static!(fs);
    info!("Config: {:?}", &device_config);

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

    let radio = Radio::new(
        peripherals.WIFI,
        peripherals.BT,
        peripherals.TIMG0,
        peripherals.RNG,
        peripherals.GPIO2.into(),
        device_config,
    );
    let (stack, _ble) = radio.start(&spawner).await;

    let web_app = firmware::web::WebApp::default();
    let web_app_state = firmware::web::AppState::new(fs, commands.sender());
    spawner.must_spawn(firmware::web::web_task(
        stack,
        web_app.router,
        web_app.config,
        web_app_state,
    ));
    info!("Web server started...");

    loop {
        Timer::after_secs(1).await;
    }
}
