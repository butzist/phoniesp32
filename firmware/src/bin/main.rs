#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to use with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use defmt::{debug, info};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::dma::DmaChannelConvert;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use firmware::mdns::MdnsResponder;
use firmware::player::Player;
use firmware::radio::Radio;
use firmware::rfid::Rfid;
use firmware::sd::Sd;
use firmware::web::WebTask;
use firmware::{mk_static, player::PlayerCommand, sd::SdFsWrapper, spi_bus};
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    esp_alloc::heap_allocator!(size: 65536);

    let timer0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer0.timer0, sw_int.software_interrupt0);

    info!("Embassy initialized!");

    let shared_bus = spi_bus::make_shared_spi(
        peripherals.SPI2.into(),
        peripherals.DMA_CH1.degrade(),
        peripherals.GPIO7.into(),
        peripherals.GPIO6.into(),
        peripherals.GPIO0.into(),
    );
    let sd = Sd::new(shared_bus, peripherals.GPIO5.into()).await;

    let (device_config, fs) = sd.init().await;
    let fs = mk_static!(SdFsWrapper, fs);
    info!("Config: {:?}", &device_config);

    let commands = mk_static!(Channel<NoopRawMutex, PlayerCommand, 2>, Channel::new());

    let player = Player::new(
        peripherals.I2S0.into(),
        peripherals.DMA_CH0,
        peripherals.GPIO23.into(),
        peripherals.GPIO15.into(),
        peripherals.GPIO22.into(),
        commands.receiver(),
        fs,
        spawner,
    );
    info!("Starting player");
    player.spawn(&spawner);

    //let controls = Controls::new(
    //    peripherals.LPWR,
    //    peripherals.TOUCH,
    //    AnyTouchPin::GPIO15(peripherals.GPIO15),
    //    AnyTouchPin::GPIO4(peripherals.GPIO4),
    //    AnyTouchPin::GPIO33(peripherals.GPIO33),
    //    AnyTouchPin::GPIO32(peripherals.GPIO32), // FIXME: GPIO32 touch does not work
    //    commands.sender(),
    //);

    //controls.spawn(&spawner);

    info!("Starting RFID");
    let rfid = Rfid::new(
        shared_bus,
        peripherals.GPIO1.into(),
        peripherals.GPIO10.into(),
        commands.sender(),
    )
    .await;
    rfid.spawn(&spawner);

    let radio = Radio::new(peripherals.WIFI, peripherals.GPIO2.into(), device_config);
    info!("Starting radio");
    let stack = radio.spawn(&spawner).await;

    info!("Starting mDNS responder");
    let mdns = MdnsResponder::new(stack);
    mdns.spawn(&spawner);

    let web_app = firmware::web::WebApp::default();
    let web_app_state = mk_static!(
        firmware::web::AppState,
        firmware::web::AppState::new(fs, commands.sender())
    );
    for id in 0..firmware::web::WEB_TASK_POOL_SIZE {
        info!("Starting web task");
        let web_task = WebTask::new(id, stack, web_app.router, web_app.config, web_app_state);
        web_task.spawn(&spawner);
    }
    info!("Web server started...");

    loop {
        Timer::after_secs(1).await;
        debug!("Still alive :)");
    }
}
