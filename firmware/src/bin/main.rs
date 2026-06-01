#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to use with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use core::cell::RefCell;
use defmt::{debug, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::dma::DmaChannelConvert;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::rtc_cntl::sleep::{GpioWakeupSource, TimerWakeupSource};
use esp_hal::system::{SleepSource, wakeup_cause};
use esp_hal::timer::timg::TimerGroup;
use firmware::controllers::buttons::Buttons;
use firmware::controllers::network::NetworkController;
use firmware::controllers::playback::PlaybackController;
use firmware::controllers::playback::status::State;
use firmware::drivers::audio::Player;
use firmware::drivers::charger::Charger;
use firmware::drivers::control_button::Button;
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
    debug!("Main: peripherals initialized");

    let timer0 = TimerGroup::new(peripherals.timer0);
    let sw_int = SoftwareInterruptControl::new(peripherals.sw_interrupt);
    esp_rtos::start(timer0.timer0, sw_int.software_interrupt0);
    debug!("Main: RTOS started");

    let rtc = mk_static!(
        RefCell<Rtc<'static>>,
        RefCell::new(Rtc::new(peripherals.lpwr))
    );
    let rtc: &'static RefCell<Rtc<'static>> = &*rtc;

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    esp_alloc::heap_allocator!(size: 65536);
    debug!("Main: heap allocators initialized");

    info!("Main: Embassy initialized!");

    // ====== DRIVERS ======

    debug!("Main: initializing SPI bus");
    let shared_bus = spi_bus::make_shared_spi(
        peripherals.spi_spi2.into(),
        peripherals.spi_dma.degrade(),
        peripherals.spi.sck,
        peripherals.spi.mosi,
        peripherals.spi.miso,
    );
    debug!("Main: SPI bus created, initializing SD card");
    let sd = Sd::new(shared_bus, peripherals.sd_cs).await;

    debug!("Main: SD card ready, initializing filesystem");
    let (device_config, fs) = sd.init().await;
    let fs = mk_static!(SdFsWrapper, fs);
    info!("Main: Config: {:?}", &device_config);

    info!("Main: Initializing player");
    let player = Player::new(
        peripherals.player_i2s.into(),
        peripherals.player_dma,
        peripherals.player.bclk,
        peripherals.player.ws,
        peripherals.player.dout,
        peripherals.audio_enable,
    );

    info!("Main: Initializing charger monitor");
    let charger = Charger::new(peripherals.charger.pin, peripherals.charger.connected_level);

    info!("Main: Initializing indicator LED");
    let indicator = IndicatorLed::new(peripherals.radio.pin);

    info!("Main: Initializing radio");
    let radio = firmware::drivers::radio::Radio::new(peripherals.radio_wifi);

    info!("Main: Initializing controls");
    let btn_play_pause = Button::new(peripherals.controls.btn_a).with_long_press(1500);
    let btn_next_prev = Button::new(peripherals.controls.btn_b).with_long_press(1500);
    let btn_vol_down = Button::new(peripherals.controls.btn_c).with_repeat(500);
    let btn_vol_up = Button::new(peripherals.controls.btn_d).with_repeat(500);

    info!("Main: Initializing RFID");
    let rfid = Rfid::new(
        shared_bus,
        peripherals.rfid_cs,
        peripherals.rfid_irq,
        peripherals.rfid_enable,
    )
    .await;

    // Spawn driver tasks
    let player_handle = PlaybackController::new(player, fs).spawn(&spawner);
    let mut charger_monitor = charger.spawn(&spawner);
    let indicator_handle = indicator.spawn(&spawner);
    let radio_handle = radio.spawn(&spawner).await;
    let rfid_handle = rfid.spawn(&spawner);
    debug!("Main: all driver tasks spawned");

    // ====== CONTROLLERS ======

    info!("Main: Starting WiFi manager");
    let wifi_handle = firmware::controllers::wifi::WifiManager::new().spawn(
        &spawner,
        radio_handle,
        indicator_handle,
        device_config,
        charger_monitor.clone(),
        rtc,
    );

    info!("Main: Starting network controller");
    NetworkController::new().spawn(
        &spawner,
        wifi_handle.clone(),
        fs,
        player_handle.clone(),
        rfid_handle.clone(),
    );

    info!("Main: Starting buttons task");
    Buttons::new(rtc, btn_play_pause, btn_next_prev, btn_vol_down, btn_vol_up).spawn(
        &spawner,
        wifi_handle.clone(),
        player_handle.clone(),
    );

    debug!("Main: all controllers spawned, entering main loop");
    loop {
        rfid_handle.trigger_scan();

        let mut is_playing = player_handle.status().get_playback_status().state == State::Playing;
        let is_charging = charger_monitor.is_connected();
        let is_wifi_active = wifi_handle.is_wifi_active();
        debug!(
            "Main: scan loop - is_playing={}, is_charging={}, is_wifi_active={}",
            is_playing, is_charging, is_wifi_active
        );

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

        if is_charging || is_playing || is_wifi_active {
            Timer::after(Duration::from_millis(scan_interval_ms)).await;
        } else {
            let timer_wakeup =
                TimerWakeupSource::new(Duration::from_millis(scan_interval_ms).into());
            let gpio_wakeup = GpioWakeupSource::new();
            rtc.borrow_mut().sleep_light(&[&timer_wakeup, &gpio_wakeup]);

            let source = wakeup_cause();
            if matches!(source, SleepSource::Gpio) {
                Timer::after(Duration::from_millis(500)).await;
            }
        }
    }
}
