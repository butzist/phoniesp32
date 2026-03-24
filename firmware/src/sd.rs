use crate::{DeviceConfig, PrintErr, spi_bus};

use aligned::Aligned;
use alloc::boxed::Box;
use alloc::vec::Vec;
use block_device_adapters::BufStream;
use block_device_driver::slice_to_blocks_mut;
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

const MBR_SIGNATURE: u16 = 0xAA55;
const PARTITION_TABLE_OFFSET: usize = 0x1BE;
const PARTITION_ENTRY_SIZE: usize = 16;

const FAT_PARTITION_TYPES: &[u8] = &[
    0x04, // FAT16 (<32MB)
    0x06, // FAT16 (>=32MB)
    0x0B, // FAT32 (CHS)
    0x0C, // FAT32 (LBA)
    0x0E, // FAT16 (LBA)
];

pub struct PartitionSlice<SPI, D, ALIGN>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    D: embedded_hal_async::delay::DelayNs + Clone,
    ALIGN: aligned::Alignment,
{
    inner: SdSpi<SPI, D, ALIGN>,
    offset: u32,
    size: u64,
}

impl<SPI, D, ALIGN> PartitionSlice<SPI, D, ALIGN>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    D: embedded_hal_async::delay::DelayNs + Clone,
    ALIGN: aligned::Alignment,
{
    async fn new(mut inner: SdSpi<SPI, D, ALIGN>) -> Self {
        let mut buffer: Aligned<ALIGN, [u8; 512]> = Aligned([0u8; 512]);
        if let Ok(()) = inner
            .read(0, slice_to_blocks_mut::<ALIGN, 512>(&mut buffer[..]))
            .await
        {
            let mbr = &buffer[..];
            let sig = u16::from_le_bytes([mbr[0x1FE], mbr[0x1FF]]);
            if sig == MBR_SIGNATURE {
                for i in 0..4 {
                    let entry_offset = PARTITION_TABLE_OFFSET + i * PARTITION_ENTRY_SIZE;
                    let partition_type = mbr[entry_offset + 4];
                    let start_lba = u32::from_le_bytes([
                        mbr[entry_offset + 8],
                        mbr[entry_offset + 9],
                        mbr[entry_offset + 10],
                        mbr[entry_offset + 11],
                    ]);
                    let sector_count = u32::from_le_bytes([
                        mbr[entry_offset + 12],
                        mbr[entry_offset + 13],
                        mbr[entry_offset + 14],
                        mbr[entry_offset + 15],
                    ]);

                    if FAT_PARTITION_TYPES.contains(&partition_type) && start_lba > 0 {
                        let partition_size = sector_count as u64 * 512;
                        info!(
                            "Found valid partition {}: type={:#02x}, start_lba={}, sectors={}, size={}",
                            i + 1,
                            partition_type,
                            start_lba,
                            sector_count,
                            partition_size
                        );
                        return Self {
                            inner,
                            offset: start_lba,
                            size: partition_size,
                        };
                    }
                }
            } else {
                info!("No valid MBR signature found, assuming raw FAT filesystem");
            }
        } else {
            info!("Failed to read MBR, assuming raw FAT filesystem");
        }

        let total_size = inner.size().await.unwrap_or(0);
        Self {
            inner,
            offset: 0,
            size: total_size,
        }
    }
}

impl<SPI, D, ALIGN, const SIZE: usize> block_device_driver::BlockDevice<SIZE>
    for PartitionSlice<SPI, D, ALIGN>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    D: embedded_hal_async::delay::DelayNs + Clone,
    ALIGN: aligned::Alignment,
    SdSpi<SPI, D, ALIGN>: block_device_driver::BlockDevice<SIZE>,
{
    type Error = sdspi::Error;
    type Align = ALIGN;

    async fn read(
        &mut self,
        block_address: u32,
        data: &mut [Aligned<ALIGN, [u8; SIZE]>],
    ) -> Result<(), Self::Error> {
        let block_offset = (block_address as u64) * SIZE as u64;
        if block_offset + (data.len() as u64 * SIZE as u64) > self.size {
            return Err(sdspi::Error::RegisterError(0xFF));
        }
        self.inner.read(self.offset + block_address, data).await
    }

    async fn write(
        &mut self,
        block_address: u32,
        data: &[Aligned<ALIGN, [u8; SIZE]>],
    ) -> Result<(), Self::Error> {
        let block_offset = (block_address as u64) * SIZE as u64;
        if block_offset + (data.len() as u64 * SIZE as u64) > self.size {
            return Err(sdspi::Error::RegisterError(0xFF));
        }
        self.inner.write(self.offset + block_address, data).await
    }

    async fn size(&mut self) -> Result<u64, Self::Error> {
        Ok(self.size)
    }
}

pub type SdFileSystem = FileSystem<
    BufStream<PartitionSlice<SpiDevice, Delay, aligned::A1>, 512>,
    NullTimeProvider,
    LossyOemCpConverter,
>;

pub type FileHandle<'a> = embedded_fatfs::File<
    'a,
    BufStream<PartitionSlice<SpiDevice, Delay, aligned::A1>, 512>,
    NullTimeProvider,
    LossyOemCpConverter,
>;

pub type StopFn = Box<dyn Fn() -> LocalBoxFuture<'static, ()>>;

// Simple wrapper for controlled filesystem access
pub struct SdFsWrapper {
    fs: Mutex<CriticalSectionRawMutex, SdFileSystem>,
    stop_fn: Mutex<CriticalSectionRawMutex, Option<StopFn>>,
}

impl SdFsWrapper {
    pub fn new(fs: SdFileSystem) -> Self {
        Self {
            fs: Mutex::new(fs),
            stop_fn: Mutex::new(None),
        }
    }

    pub async fn borrow_for_playback(&self, stop_fn: StopFn) -> PlaybackGuard<'_> {
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
        stop_fn_guard: &mut MutexGuard<'_, CriticalSectionRawMutex, Option<StopFn>>,
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
                        .with_frequency(Rate::from_mhz(20))
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

        let sd = PartitionSlice::new(sd).await;
        let inner = BufStream::<_, 512>::new(sd);
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
