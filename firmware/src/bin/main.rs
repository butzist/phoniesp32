#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::ops::Coroutine;
use core::pin::Pin;

use audio_codec_algorithms::{decode_adpcm_ima, AdpcmImaState};
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embedded_io_async::{Read, Seek};
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use esp_hal::i2s::master::{DataFormat, I2s, Standard};
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, gpio::Level};
use esp_hal::{dma_buffers, dma_buffers_chunk_size, spi};
use esp_wifi::ble::controller::BleConnector;
use firmware::sd::{init_sd, SdFileSystem};
use firmware::PrintErr;
use static_cell::make_static;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 48 * 1024);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG1);

    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    let spi_bus = {
        let dma_channel = peripherals.DMA_SPI2;
        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
            dma_buffers_chunk_size!(4 * 1024, 1024);

        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        make_static!(Spi::new(
            peripherals.SPI2,
            spi::master::Config::default()
                .with_frequency(Rate::from_khz(400))
                .with_mode(spi::Mode::_0),
        )
        .unwrap()
        .with_sck(peripherals.GPIO18)
        .with_mosi(peripherals.GPIO23)
        .with_miso(peripherals.GPIO19)
        .with_dma(dma_channel)
        .with_buffers(dma_rx_buf, dma_tx_buf)
        .into_async())
    };

    let sd_cs = make_static!(Output::new(
        peripherals.GPIO5,
        Level::High,
        OutputConfig::default()
    ));
    let (device_config, fs) = make_static!(init_sd(spi_bus, sd_cs).await);
    info!("Config: {:?}", &device_config);

    if false {
        let rng = esp_hal::rng::Rng::new(peripherals.RNG);
        let timer1 = TimerGroup::new(peripherals.TIMG0);
        let wifi_init =
            make_static!(esp_wifi::init(timer1.timer0, rng)
                .expect("Failed to initialize WIFI/BLE controller"));

        let _connector = BleConnector::new(wifi_init, peripherals.BT);

        let wifi_led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
        let stack =
            firmware::wifi::start_wifi(wifi_init, peripherals.WIFI, wifi_led, rng, None, &spawner)
                .await;

        let web_app = firmware::web::WebApp::default();
        for id in 0..firmware::web::WEB_TASK_POOL_SIZE {
            spawner.must_spawn(firmware::web::web_task(
                id,
                stack,
                web_app.router,
                web_app.config,
            ));
        }
        info!("Web server started...");
    }

    let dma_transfer = {
        let dma_channel = peripherals.DMA_I2S0;
        let (_, _, tx_buffer, tx_descriptors) = dma_buffers!(0, 4 * 4096);

        let i2s = I2s::new(
            peripherals.I2S0,
            Standard::Philips,
            DataFormat::Data16Channel16,
            Rate::from_hz(44100),
            dma_channel,
        )
        .into_async();

        let i2s_tx = i2s
            .i2s_tx
            .with_bclk(peripherals.GPIO27)
            .with_ws(peripherals.GPIO26)
            .with_dout(peripherals.GPIO25)
            .build(tx_descriptors);

        i2s_tx.write_dma_circular_async(tx_buffer).unwrap()
    };

    spawner.must_spawn(player(dma_transfer, fs));

    loop {
        embassy_time::Timer::after_millis(1000).await;
    }
}

enum BlockReadResult {
    Full, // full buffer
    Partial(usize),
    Eof, // buffer is empty
}

async fn read_block<R>(mut file: R, mut buf: &mut [u8]) -> Result<BlockReadResult, R::Error>
where
    R: embedded_io_async::Read,
    R::Error: defmt::Format,
{
    let full_size = buf.len();

    while !buf.is_empty() {
        match file.read(buf).await {
            Ok(0) => {
                return Ok(if buf.len() == full_size {
                    BlockReadResult::Eof
                } else {
                    BlockReadResult::Partial(full_size - buf.len())
                });
            }
            Ok(n) => buf = &mut buf[n..],
            Err(err) => return Err(err),
        }
    }

    Ok(BlockReadResult::Full)
}

#[embassy_executor::task]
async fn player(
    mut dma_transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; 4 * 4096]>,
    fs: &'static SdFileSystem<'static>,
) {
    let root = fs.root_dir();
    let wav = root.open_file("test.wav").await.unwrap();
    play_file(wav, &mut dma_transfer).await;
}

async fn play_file<R: Read + Seek>(
    file: R,
    dma_transfer: &mut I2sWriteDmaTransferAsync<'static, &'static mut [u8; 4 * 4096]>,
) where
    R::Error: defmt::Format,
{
    // skip header
    file.seek(embedded_io::SeekFrom::Start(48)).await.unwrap();

    let mut pending_buffer = make_static!([0u8; 1024]);
    let mut ready_buffer = make_static!([0u8; 1024]);

    let (next_block, eof) = match read_block(&mut file, ready_buffer).await {
        Ok(BlockReadResult::Full) => (&ready_buffer[..], false),
        Ok(BlockReadResult::Eof) => return, // cleanup DMA buffer afterwards
        Ok(BlockReadResult::Partial(size)) => {
            if size < 4 {
                return; // would crash the decoder
            } else {
                (&ready_buffer[..size], true)
            }
        }
        Err(err) => {
            error!("reading audio file: {}", err);
            return;
        }
    };
    let mut read_future = if !eof {
        Some(read_block(&mut file, pending_buffer))
    } else {
        None
    };

    loop {
        let mut decoder = #[coroutine]
        || {
            let mut state = AdpcmImaState::new();
            state.predictor = i16::from_le_bytes([next_block[0], next_block[1]]);
            state.step_index = next_block[2].min(88);
            let sample = state.predictor;
            yield sample as u8;
            yield (sample >> 8) as u8;
            yield 0;
            yield 0;

            for b in &next_block[4..] {
                let sample = decode_adpcm_ima(*b & 0x0f, &mut state);
                yield sample as u8;
                yield (sample >> 8) as u8;
                yield 0;
                yield 0;

                let sample = decode_adpcm_ima(*b >> 4, &mut state);
                yield sample as u8;
                yield (sample >> 8) as u8;
                yield 0;
                yield 0;
            }
        };

        let transfer_future = async {
            let mut decoding_done = false;
            while !decoding_done {
                dma_transfer
                    .push_with(|buf: &mut [u8]| {
                        for (position, val) in buf.iter_mut().enumerate() {
                            match Pin::new(&mut decoder).resume(()) {
                                core::ops::CoroutineState::Yielded(b) => {
                                    *val = b;
                                }
                                core::ops::CoroutineState::Complete(_) => {
                                    decoding_done = true;
                                    return position;
                                }
                            }
                        }

                        buf.len()
                    })
                    .await
                    .print_err("I2S DMA transfer");
            }
        };

        if let Some(read) = read_future {
            join(transfer_future, read).await;

            (pending_buffer, ready_buffer) = (ready_buffer, pending_buffer);

            let (next_block, eof) = match read_block(file, ready_buffer).await {
                Ok(BlockReadResult::Full) => (&ready_buffer[..], false),
                Ok(BlockReadResult::Eof) => return, // cleanup DMA buffer afterwards
                Ok(BlockReadResult::Partial(size)) => {
                    if size < 4 {
                        return; // would crash the decoder
                    } else {
                        (&ready_buffer[..size], true)
                    }
                }
                Err(err) => {
                    error!("reading audio file: {}", err);
                    return;
                }
            };

            read_future = if !eof {
                Some(read_block(&mut file, pending_buffer))
            } else {
                None
            };
        } else {
            transfer_future.await;
            return;
        }
    }
}
