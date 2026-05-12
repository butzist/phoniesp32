#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to use with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::dma::DmaChannelConvert;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::rtc_cntl::sleep::{GpioWakeupSource, TimerWakeupSource};
use esp_hal::system::{SleepSource, wakeup_cause};
use esp_hal::timer::timg::TimerGroup;
use firmware::controllers::network::NetworkController;
use firmware::controllers::playback::PlaybackController;
use firmware::controllers::playback::status::State;
use firmware::drivers::audio::Player;
use firmware::drivers::charger::Charger;
use firmware::drivers::controls::Controls;
use firmware::drivers::indicator::IndicatorLed;
use firmware::drivers::rfid::Rfid;
use firmware::drivers::sd::{Sd, SdFsWrapper};
use firmware::drivers::spi_bus;
use firmware::mk_static;
use firmware::peripherals::create_peripherals;
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

    let peripherals = create_peripherals(peripherals);

    let timer0 = TimerGroup::new(peripherals.timer0);
    let sw_int = SoftwareInterruptControl::new(peripherals.sw_interrupt);
    esp_rtos::start(timer0.timer0, sw_int.software_interrupt0);

    let mut rtc = Rtc::new(peripherals.lpwr);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    esp_alloc::heap_allocator!(size: 65536);

    info!("Main: Embassy initialized!");

    let shared_bus = spi_bus::make_shared_spi(
        peripherals.spi_spi2.into(),
        peripherals.spi_dma.degrade(),
        peripherals.spi.sck,
        peripherals.spi.mosi,
        peripherals.spi.miso,
    );
    let sd = Sd::new(shared_bus, peripherals.sd_cs).await;

    let (device_config, fs) = sd.init().await;
    let fs = mk_static!(SdFsWrapper, fs);
    info!("Main: Config: {:?}", &device_config);

    info!("Main: Starting player");
    let player = Player::new(
        peripherals.player_i2s.into(),
        peripherals.player_dma,
        peripherals.player.bclk,
        peripherals.player.ws,
        peripherals.player.dout,
        peripherals.audio_enable,
    );
    let player_handle = PlaybackController::new(player, fs).spawn(&spawner);

    info!("Main: Starting controls");
    let controls = Controls::new(
        peripherals.controls.btn_a,
        peripherals.controls.btn_b,
        peripherals.controls.btn_c,
        peripherals.controls.btn_d,
    );
    controls.spawn(&spawner, player_handle.clone());

    info!("Main: Starting RFID");
    let rfid = Rfid::new(
        shared_bus,
        peripherals.rfid_cs,
        peripherals.rfid_irq,
        peripherals.rfid_enable,
    )
    .await;
    let rfid_handle = rfid.spawn(&spawner);

    info!("Main: Starting charger monitor");
    let charger = Charger::new(peripherals.charger.pin, peripherals.charger.connected_level);
    let mut charger_monitor = charger.spawn(&spawner);

    let indicator = IndicatorLed::new(peripherals.radio.pin);
    let indicator_handle = indicator.spawn(&spawner);

    let radio = firmware::drivers::radio::Radio::new(peripherals.radio_wifi);
    let radio_handle = radio.spawn(&spawner).await;

    let wifi_handle = firmware::controllers::wifi::WifiManager::new(
        radio_handle,
        indicator_handle,
        device_config,
        charger_monitor.clone(),
    )
    .spawn(&spawner);

    info!("Main: Starting network controller");
    let network_controller =
        NetworkController::new(wifi_handle, fs, player_handle.clone(), rfid_handle.clone());
    network_controller.spawn(&spawner);

    loop {
        rfid_handle.trigger_scan();

        let mut is_playing = player_handle.status().get_playback_status().state == State::Playing;
        let is_charging = charger_monitor.is_connected();

        let scan_interval_ms = match rfid_handle.wait_for_scan_result().await {
            firmware::drivers::rfid::RfidScanResult::Found(fob) => {
                player_handle
                    .play_playlist_ref(firmware::entities::playlist::PlayListRef::new(fob))
                    .await;
                is_playing = true;

                5000
            }
            firmware::drivers::rfid::RfidScanResult::NotFound => 500,
            firmware::drivers::rfid::RfidScanResult::Error => 1000,
        };

        if is_charging || is_playing {
            Timer::after(Duration::from_millis(scan_interval_ms)).await;
        } else {
            let timer_wakeup =
                TimerWakeupSource::new(Duration::from_millis(scan_interval_ms).into());
            let gpio_wakeup = GpioWakeupSource::new();
            rtc.sleep_light(&[&timer_wakeup, &gpio_wakeup]);

            let source = wakeup_cause();
            if matches!(source, SleepSource::Gpio) {
                Timer::after(Duration::from_millis(500)).await;
            }
        }
    }
}
