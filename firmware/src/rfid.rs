use alloc::format;
use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Sender;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::AnyPin;
use esp_hal::time::Rate;
use heapless::String;
use mfrc522::comm::blocking::spi::{DummyDelay, SpiInterface};
use mfrc522::{Initialized, Mfrc522};

use crate::entities::playlist::PlayListRef;
use crate::player::PlayerCommand;
use crate::spi_bus;
use crate::spi_wrapper::AsyncToBlockingSpiDevice;

use {esp_backtrace as _, esp_println as _};

use spi_bus::SpiDevice;

type MyMfrc522 =
    Mfrc522<SpiInterface<AsyncToBlockingSpiDevice<SpiDevice>, DummyDelay>, Initialized>;

pub static LAST_FOB: Mutex<CriticalSectionRawMutex, Option<String<8>>> = Mutex::new(None);

pub struct Rfid;

impl Rfid {
    pub fn new(
        shared_spi: spi_bus::SharedSpi,
        cs: AnyPin<'static>,
        spawner: &Spawner,
        commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    ) -> Self {
        let spi_dev = spi_bus::make_spi_device(shared_spi, cs, Rate::from_mhz(10));
        let blocking_spi = AsyncToBlockingSpiDevice::new(spi_dev);

        let spi_interface = SpiInterface::new(blocking_spi);
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
                info!("FOB scanned: {}", fob_str);

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
