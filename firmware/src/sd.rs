use crate::{DeviceConfig, PrintErr, spi_bus};

use alloc::boxed::Box;
use alloc::vec::Vec;
use block_device_adapters::BufStream;
use defmt::{info, warn};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};

use embassy_time::Delay;
use embedded_fatfs::{FileSystem, FsOptions, LossyOemCpConverter, NullTimeProvider};
use embedded_hal_async::delay::DelayNs;
use embedded_io_async::Read;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use esp_hal::time::Rate;
use futures::future::LocalBoxFuture;
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
    stop_fn: Mutex<CriticalSectionRawMutex, Option<Box<dyn Fn() -> LocalBoxFuture<'static, ()>>>>,
}

impl SdFsWrapper {
    pub fn new(fs: SdFileSystem) -> Self {
        Self {
            fs: Mutex::new(fs),
            stop_fn: Mutex::new(None),
        }
    }

    pub async fn borrow_for_playback(
        &self,
        stop_fn: Box<dyn Fn() -> LocalBoxFuture<'static, ()>>,
    ) -> PlaybackGuard<'_> {
        let mut stop_fn_guard = self.stop_fn.lock().await;

        // Stop any existing playback
        self.stop_playback(&mut stop_fn_guard).await;

        // Store the new stop function
        *stop_fn_guard = Some(stop_fn);

        // Keep stop_fn_guard locked while acquiring fs lock to prevent race conditions
        let fs_guard = self.fs.lock().await;

        PlaybackGuard {
            wrapper: self,
            fs: fs_guard,
        }
    }

    pub async fn borrow_mut(&self) -> MutexGuard<'_, CriticalSectionRawMutex, SdFileSystem> {
        // Stop any active playback before borrowing
        let mut stop_fn_guard = self.stop_fn.lock().await;
        self.stop_playback(&mut stop_fn_guard).await;
        self.fs.lock().await
    }

    /// Stop any existing playback and clear the handle.
    /// Returns true if playback was active and was stopped.
    async fn stop_playback(
        &self,
        stop_fn_guard: &mut MutexGuard<
            '_,
            CriticalSectionRawMutex,
            Option<Box<dyn Fn() -> LocalBoxFuture<'static, ()>>>,
        >,
    ) {
        if let Some(ref stop_fn) = **stop_fn_guard {
            warn!("Stopping running player");
            stop_fn().await;
            // Give player time to actually stop
            embassy_time::Timer::after_millis(100).await;
            **stop_fn_guard = None;
        }
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
        // Clear the handle to indicate playback is finished
        // TODO: Find better solution - we use try_lock since Drop can't be async
        // This is still safe, but will in the worst case issue an unnecessary call to
        // `Self::stop_playback()`
        if let Ok(mut guard) = self.wrapper.stop_fn.try_lock() {
            *guard = None;
        }
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
