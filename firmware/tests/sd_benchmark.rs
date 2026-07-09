//! HIL: SD card read/write throughput benchmark.
//!
//! Runs on hardware (ESP32-C6). Requires a formatted SD card with a FAT
//! partition inserted in the socket.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::cell::RefCell;

use defmt::info;
use embassy_time::Instant;
use embedded_test::*;
use esp_hal::clock::CpuClock;
use esp_hal::dma::DmaChannelConvert;
use esp_hal::gpio::AnyPin;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::timer::timg::TimerGroup;
use firmware::drivers::sd::{Sd, SdFsWrapper};
use firmware::drivers::spi_bus::{self, SharedSpi};
use firmware::mk_static;
use firmware::peripherals::create_peripherals;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

const TEST_FILE: &str = "BENCH.TST";
const TEST_SIZE: usize = 256 * 1024;
const CHUNK_SIZE: usize = 4096;
const MIN_WRITE_KBPS: u32 = 50;
const MIN_READ_KBPS: u32 = 100;

struct BenchState {
    shared_spi: SharedSpi,
    sd_cs: AnyPin<'static>,
}

// Set once in init, read in test. Safe because tests run sequentially.
static mut BENCH: Option<BenchState> = None;

#[tests]
mod tests {
    use super::*;

    #[init]
    fn init() {
        let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        let esp_periphs = esp_hal::init(config);
        let periphs = create_peripherals(esp_periphs);

        let timer0 = TimerGroup::new(periphs.timer0);
        let sw_int = SoftwareInterruptControl::new(periphs.sw_interrupt);
        esp_rtos::start(timer0.timer0, sw_int.software_interrupt0);

        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
        esp_alloc::heap_allocator!(size: 65536);

        let _rtc = mk_static!(
            RefCell<Rtc<'static>>,
            RefCell::new(Rtc::new(periphs.lpwr))
        );

        let shared_spi = spi_bus::make_shared_spi(
            periphs.spi_spi2.into(),
            periphs.spi_dma.degrade(),
            periphs.spi.sck,
            periphs.spi.mosi,
            periphs.spi.miso,
        );

        unsafe {
            BENCH = Some(BenchState {
                shared_spi,
                sd_cs: periphs.sd_cs,
            });
        }

        info!("SD benchmark: hardware initialized");
    }

    #[test]
    fn sd_card_write_read_benchmark() {
        let state = unsafe { BENCH.as_mut().unwrap() };

        info!("SD benchmark: initialising card ...");
        let sd = embassy_futures::block_on(Sd::new(state.shared_spi, state.sd_cs.reborrow()));
        let (_config, fs) = embassy_futures::block_on(sd.init());
        let fs = mk_static!(SdFsWrapper, fs);

        // ---- write ----
        info!("SD benchmark: writing {} bytes ...", TEST_SIZE);
        let (written, write_ms) = {
            let guard = embassy_futures::block_on(fs.borrow_mut());
            let root = guard.root_dir();
            if embassy_futures::block_on(root.exists(TEST_FILE)).unwrap_or(false) {
                embassy_futures::block_on(root.remove(TEST_FILE)).ok();
            }
            let mut file =
                embassy_futures::block_on(root.create_file(TEST_FILE)).expect("create file");
            let data = alloc::vec![0xABu8; CHUNK_SIZE];
            let start = Instant::now();
            let mut written = 0u64;
            let target = TEST_SIZE as u64;
            while written < target {
                let chunk = core::cmp::min(CHUNK_SIZE, (target - written) as usize);
                embassy_futures::block_on(
                    embedded_io_async::Write::write_all(&mut file, &data[..chunk]),
                )
                .expect("write chunk");
                written += chunk as u64;
            }
            let elapsed = start.elapsed();
            (written, elapsed.as_millis())
        };

        let write_kbps = if write_ms > 0 {
            (written * 1000 / write_ms / 1024) as u32
        } else {
            u32::MAX
        };
        info!(
            "SD benchmark: wrote {} bytes in {} ms ({} KB/s)",
            written, write_ms, write_kbps
        );
        assert!(
            write_kbps >= MIN_WRITE_KBPS,
            "Write throughput too low: {} KB/s < min {} KB/s",
            write_kbps,
            MIN_WRITE_KBPS,
        );

        // ---- read ----
        info!("SD benchmark: reading {} bytes ...", TEST_SIZE);
        let (read, read_ms) = {
            let guard = embassy_futures::block_on(fs.borrow_mut());
            let root = guard.root_dir();
            let mut file =
                embassy_futures::block_on(root.open_file(TEST_FILE)).expect("open file");
            let mut buf = alloc::vec![0u8; CHUNK_SIZE];
            let start = Instant::now();
            let mut total = 0u64;
            loop {
                let n = embassy_futures::block_on(
                    embedded_io_async::Read::read(&mut file, &mut buf),
                )
                .expect("read chunk");
                if n == 0 {
                    break;
                }
                total += n as u64;
            }
            let elapsed = start.elapsed();
            (total, elapsed.as_millis())
        };

        let read_kbps = if read_ms > 0 {
            (read * 1000 / read_ms / 1024) as u32
        } else {
            u32::MAX
        };
        info!(
            "SD benchmark: read {} bytes in {} ms ({} KB/s)",
            read, read_ms, read_kbps
        );
        assert!(
            read_kbps >= MIN_READ_KBPS,
            "Read throughput too low: {} KB/s < min {} KB/s",
            read_kbps,
            MIN_READ_KBPS,
        );

        // ---- cleanup ----
        {
            let guard = embassy_futures::block_on(fs.borrow_mut());
            let root = guard.root_dir();
            embassy_futures::block_on(root.remove(TEST_FILE)).ok();
        }

        info!(
            "SD benchmark: PASS (write={} KB/s, read={} KB/s)",
            write_kbps, read_kbps
        );
    }
}
