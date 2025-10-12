use crate::{DeviceConfig, PrintErr};

use alloc::vec::Vec;
use block_device_adapters::BufStream;
use defmt::{info, warn};
use embassy_time::Delay;
use embedded_fatfs::{FileSystem, FsOptions, LossyOemCpConverter, NullTimeProvider};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_io_async::Read;
use esp_hal::gpio::Output;
use esp_hal::spi::master::SpiDmaBus;
use esp_hal::time::Rate;
use esp_hal::{spi, Async};
use sdspi::SdSpi;

use {esp_backtrace as _, esp_println as _};

pub type SdFileSystem<'a> = FileSystem<
    BufStream<
        SdSpi<
            ExclusiveDevice<&'a mut SpiDmaBus<'static, Async>, &'a mut Output<'static>, Delay>,
            Delay,
            aligned::A1,
        >,
        512,
    >,
    NullTimeProvider,
    LossyOemCpConverter,
>;

pub async fn init_sd(
    spi_bus: &'static mut SpiDmaBus<'static, Async>,
    sd_cs: &'static mut Output<'static>,
) -> (Option<DeviceConfig>, SdFileSystem<'static>) {
    loop {
        match sdspi::sd_init(spi_bus, sd_cs).await {
            Ok(_) => break,
            Err(e) => {
                warn!("Sd init error: {:?}", e);
                embassy_time::Timer::after_millis(10).await;
            }
        }
    }

    let spid = ExclusiveDevice::new(spi_bus, sd_cs, Delay).unwrap();
    let mut sd = SdSpi::<_, _, aligned::A1>::new(spid, Delay);

    loop {
        // Initialize the card
        if sd.init().await.is_ok() {
            // Increase the speed up to the SD max
            sd.spi()
                .bus_mut()
                .apply_config(
                    &spi::master::Config::default()
                        .with_frequency(Rate::from_mhz(8))
                        .with_mode(spi::Mode::_0),
                )
                .unwrap();

            info!("Initialization complete!");

            break;
        }
        info!("Failed to init card, retrying...");
        embassy_time::Delay.delay_ns(5000u32).await;
    }

    let sd_size = sd.size().await.unwrap();
    info!("SD card size: {}", sd_size);

    let inner = BufStream::<_, 512>::new(sd);
    // FIXME - need to skip manually to first partition?
    let fs = embedded_fatfs::FileSystem::new(inner, FsOptions::new())
        .await
        .print_err("open filesystem")
        .unwrap();

    let root_dir = fs.root_dir();
    let mut config_file = root_dir.open_file("config.jsn").await.ok();

    let config = if let Some(config_file) = &mut config_file {
        let mut bytes = Vec::new();
        let mut buffer = alloc::vec![0u8; 128];
        loop {
            let n = config_file.read(&mut buffer).await.unwrap();
            if n == 0 {
                break;
            };
            bytes.extend(&buffer[..n]);
        }

        serde_json::from_slice(&bytes).ok()
    } else {
        None
    };

    drop(config_file);
    drop(root_dir);

    (config, fs)
}
