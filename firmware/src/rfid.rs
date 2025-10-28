use alloc::format;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Sender;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use esp_hal::spi::master::{AnySpi, Spi};
use esp_hal::time::Rate;
use esp_hal::{spi, Blocking};
use heapless::String;
use mfrc522::comm::blocking::spi::{DummyDelay, SpiInterface};
use mfrc522::{Initialized, Mfrc522};

use crate::entities::playlist::PlayListRef;
use crate::player::PlayerCommand;

use {esp_backtrace as _, esp_println as _};

type MyMfrc522 = Mfrc522<
    SpiInterface<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>, DummyDelay>,
    Initialized,
>;

pub static LAST_FOB: Mutex<CriticalSectionRawMutex, Option<String<8>>> = Mutex::new(None);

pub struct Rfid;

impl Rfid {
    pub fn new(
        spi: AnySpi<'static>,
        sck: AnyPin<'static>,
        mosi: AnyPin<'static>,
        miso: AnyPin<'static>,
        cs: AnyPin<'static>,
        spawner: &Spawner,
        commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    ) -> Self {
        let rfid_spi_bus = Spi::new(
            spi,
            spi::master::Config::default()
                .with_frequency(Rate::from_mhz(5))
                .with_mode(spi::Mode::_0),
        )
        .unwrap()
        .with_sck(sck)
        .with_mosi(mosi)
        .with_miso(miso);

        let rfid_cs = Output::new(cs, Level::High, OutputConfig::default());

        let spi_dev = ExclusiveDevice::new(rfid_spi_bus, rfid_cs, Delay).unwrap();

        let spi_interface = SpiInterface::new(spi_dev);
        let rfid = Mfrc522::new(spi_interface).init().unwrap();

        spawner.must_spawn(rfid_task(rfid, commands));

        Self
    }
}

#[embassy_executor::task]
async fn rfid_task(mut rfid: MyMfrc522, commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>) {
    loop {
        if let Ok(atqa) = rfid.reqa() {
            Timer::after(Duration::from_millis(50)).await;
            if let Ok(uid) = rfid.select(&atqa) {
                let uid_bytes = uid.as_bytes();

                let hex_str = format!(
                    "{:02x}{:02x}{:02x}{:02x}",
                    uid_bytes[0], uid_bytes[1], uid_bytes[2], uid_bytes[3]
                );

                let fob_str = String::try_from(hex_str.as_str()).unwrap();
                LAST_FOB.lock().await.replace(fob_str.clone());

                let _ = commands
                    .send(PlayerCommand::PlaylistRef(PlayListRef::new(fob_str)))
                    .await;
                Timer::after(Duration::from_millis(5000)).await;
            }
        } else {
            Timer::after(Duration::from_millis(500)).await;
        }
    }
}
