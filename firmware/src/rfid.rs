use alloc::format;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::time::Rate;
use heapless::String;
use mfrc522_async::{EnableError, Mfrc522};
use unwrap_infallible::UnwrapInfallible;

use crate::entities::playlist::PlayListRef;
use crate::player::PlayerHandle;
use crate::spi_bus;

use {esp_backtrace as _, esp_println as _};

use spi_bus::SpiDevice;

type MyMfrc522 = Mfrc522<SpiDevice, Input<'static>, Output<'static>, mfrc522_async::Unknown>;

pub static LAST_FOB: Mutex<CriticalSectionRawMutex, Option<String<8>>> = Mutex::new(None);

pub struct Rfid {
    rfid: MyMfrc522,
    commands: PlayerHandle,
}

impl Rfid {
    pub async fn new(
        shared_spi: spi_bus::SharedSpi,
        cs: AnyPin<'static>,
        irq: AnyPin<'static>,
        rfid_enable: AnyPin<'static>,
        commands: PlayerHandle,
    ) -> Self {
        let rfid = Self::create_device(shared_spi, cs, irq, rfid_enable);
        Self { rfid, commands }
    }

    fn create_device(
        shared_spi: spi_bus::SharedSpi,
        cs: AnyPin<'static>,
        irq: AnyPin<'static>,
        rfid_enable: AnyPin<'static>,
    ) -> Mfrc522<SpiDevice, Input<'static>, Output<'static>, mfrc522_async::Unknown> {
        let spi_dev = spi_bus::make_spi_device(shared_spi, cs, Rate::from_mhz(10));
        let irq = Input::new(irq, InputConfig::default().with_pull(Pull::Up));
        let enable = Output::new(rfid_enable, Level::Low, OutputConfig::default());
        Mfrc522::new(spi_dev, irq, enable)
    }

    pub fn spawn(self, spawner: &Spawner) {
        spawner.must_spawn(rfid_task(self.rfid, self.commands));
    }
}

#[embassy_executor::task]
async fn rfid_task(mut rfid: MyMfrc522, commands: PlayerHandle) {
    loop {
        let rfid_device = rfid
            .enable()
            .map_err(EnableError::into_error)
            .unwrap_infallible();

        match rfid_device.init().await {
            Ok(mut rfid_device) => {
                let delay = match rfid_device.reqa().await {
                    Ok(atqa) => {
                        Timer::after(Duration::from_millis(50)).await;
                        if let Ok(uid) = rfid_device.select(&atqa).await {
                            let uid_bytes = uid.as_bytes();

                            let hex_str = format!(
                                "{:02x}{:02x}{:02x}{:02x}",
                                uid_bytes[0], uid_bytes[1], uid_bytes[2], uid_bytes[3]
                            );

                            let fob_str = String::try_from(hex_str.as_str()).unwrap();
                            LAST_FOB.lock().await.replace(fob_str.clone());
                            info!("FOB scanned: {}", fob_str);

                            commands.play_playlist_ref(PlayListRef::new(fob_str)).await;

                            Duration::from_millis(5000)
                        } else {
                            Duration::from_millis(500)
                        }
                    }
                    Err(err) => {
                        if err != mfrc522_async::Error::Timeout {
                            error!("RFID error: {:?}", defmt::Debug2Format(&err));
                        }

                        Duration::from_millis(500)
                    }
                };

                rfid = rfid_device
                    .disable()
                    .map_err(EnableError::into_error)
                    .unwrap_infallible()
                    .into_unknown();

                Timer::after(delay).await;
            }
            Err(e) => {
                error!(
                    "RFID initialization failed: {:?}",
                    defmt::Debug2Format(e.error())
                );

                let uninitialized_device = e.into_device();
                rfid = uninitialized_device
                    .disable()
                    .map_err(EnableError::into_error)
                    .unwrap_infallible()
                    .into_unknown();

                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}
