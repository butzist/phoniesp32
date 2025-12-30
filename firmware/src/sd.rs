use crate::player::playback::STOP_SIGNAL;
use crate::{DeviceConfig, PrintErr, spi_bus};

use alloc::vec::Vec;
use block_device_adapters::BufStream;
use core::sync::atomic::{AtomicBool, Ordering};
use defmt::{info, warn};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};

use embassy_time::Delay;
use embedded_fatfs::{FileSystem, FsOptions, LossyOemCpConverter, NullTimeProvider};
use embedded_hal_async::delay::DelayNs;
use embedded_io_async::Read;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use esp_hal::time::Rate;
use sdspi::SdSpi;

use {esp_backtrace as _, esp_println as _};

use spi_bus::SpiDevice;

pub type SdFileSystem = FileSystem<
    BufStream<SdSpi<SpiDevice, Delay, aligned::A1>, 512>,
    NullTimeProvider,
    LossyOemCpConverter,
>;

pub type FileHandle<'a> = embedded_fatfs::File<
    'a,
    BufStream<SdSpi<SpiDevice, Delay, aligned::A1>, 512>,
    NullTimeProvider,
    LossyOemCpConverter,
>;

// Simple wrapper for controlled filesystem access
pub struct SdFsWrapper {
    fs: Mutex<CriticalSectionRawMutex, SdFileSystem>,
    playback_borrowed: AtomicBool,
}

impl SdFsWrapper {
    pub fn new(fs: SdFileSystem) -> Self {
        Self {
            fs: Mutex::new(fs),
            playback_borrowed: AtomicBool::new(false),
        }
    }

    pub async fn borrow_for_playback(&self) -> PlaybackGuard<'_> {
        let was_borrowed = self.playback_borrowed.swap(true, Ordering::SeqCst);
        if was_borrowed {
            warn!("Stopping running player");
            self.stop_playback().await;
        }

        self.playback_borrowed.store(true, Ordering::SeqCst); // previous task has probably
        // finished and written false here

        let guard = self.fs.lock().await;

        PlaybackGuard {
            wrapper: self,
            fs: guard,
        }
    }

    pub async fn borrow_mut(&self) -> MutexGuard<'_, CriticalSectionRawMutex, SdFileSystem> {
        if self.playback_borrowed.load(Ordering::SeqCst) {
            warn!("Stopping running player");
            self.stop_playback().await;
        }

        self.fs.lock().await
    }

    async fn stop_playback(&self) {
        STOP_SIGNAL.signal(());
        embassy_time::Timer::after_millis(100).await;
        STOP_SIGNAL.reset();
    }
}

pub struct PlaybackGuard<'a> {
    wrapper: &'a SdFsWrapper,
    fs: MutexGuard<'a, CriticalSectionRawMutex, SdFileSystem>,
}

impl<'a> core::ops::Deref for PlaybackGuard<'a> {
    type Target = SdFileSystem;

    fn deref(&self) -> &Self::Target {
        &self.fs
    }
}

impl<'a> Drop for PlaybackGuard<'a> {
    fn drop(&mut self) {
        self.wrapper
            .playback_borrowed
            .store(false, Ordering::SeqCst);
    }
}

pub struct Sd {
    device: SpiDevice,
}

impl Sd {
    pub async fn new(spi: spi_bus::SharedSpi, mut cs: AnyPin<'static>) -> Self {
        let mut cs_init = Output::new(cs.reborrow(), Level::High, OutputConfig::default());
        loop {
            match sdspi::sd_init(&mut *spi.lock().await, &mut cs_init).await {
                Ok(_) => break,
                Err(e) => {
                    warn!("Sd init error: {:?}", e);
                    embassy_time::Timer::after_millis(10).await;
                }
            }
        }

        let device = spi_bus::make_spi_device(spi, cs, Rate::from_mhz(10));

        Self { device }
    }

    pub async fn init(self) -> (Option<DeviceConfig>, SdFsWrapper) {
        let spid = self.device;
        let mut sd = SdSpi::<_, _, aligned::A1>::new(spid, Delay);

        loop {
            // Initialize the card
            if sd.init().await.is_ok() {
                sd.spi().set_config(
                    esp_hal::spi::master::Config::default()
                        .with_frequency(Rate::from_mhz(10))
                        .with_mode(esp_hal::spi::Mode::_0),
                );

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

        let fs_wrapper = SdFsWrapper::new(fs);

        (config, fs_wrapper)
    }
}
