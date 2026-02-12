use alloc::format;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Sender;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};
use esp_hal::time::Rate;
use heapless::String;
use mfrc522_async::{Initialized, Mfrc522};

use crate::entities::playlist::PlayListRef;
use crate::player::PlayerCommand;
use crate::spi_bus;

use {esp_backtrace as _, esp_println as _};

use spi_bus::SpiDevice;

type MyMfrc522 = Mfrc522<SpiDevice, Input<'static>, Initialized>;

pub static LAST_FOB: Mutex<CriticalSectionRawMutex, Option<String<8>>> = Mutex::new(None);

pub struct Rfid {
    rfid: MyMfrc522,
    commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
}

impl Rfid {
    pub async fn new(
        shared_spi: spi_bus::SharedSpi,
        cs: AnyPin<'static>,
        irq: AnyPin<'static>,
        commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    ) -> Self {
        let spi_dev = spi_bus::make_spi_device(shared_spi, cs, Rate::from_mhz(10));
        let irq = Input::new(irq, InputConfig::default().with_pull(Pull::Up));
        let rfid = Mfrc522::new(spi_dev, irq).init().await.unwrap();

        Self { rfid, commands }
    }

    pub fn spawn(self, spawner: &Spawner) {
        spawner.must_spawn(rfid_task(self.rfid, self.commands));
    }
}

#[embassy_executor::task]
async fn rfid_task(mut rfid: MyMfrc522, commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>) {
    loop {
        match rfid.reqa().await {
            Ok(atqa) => {
                Timer::after(Duration::from_millis(50)).await;
                if let Ok(uid) = rfid.select(&atqa).await {
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
            }
            Err(err) => {
                if err != mfrc522_async::Error::Timeout {
                    error!("RFID error: {:?}", defmt::Debug2Format(&err));
                }
                Timer::after(Duration::from_millis(500)).await;
            }
        }
    }
}
