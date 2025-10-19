use core::ops::Coroutine;
use core::pin::Pin;
use core::sync::atomic::{AtomicU8, Ordering};

use crate::{extend_to_static, PrintErr};
use crate::{retry, sd::SdFileSystem};
use audio_codec_algorithms::{decode_adpcm_ima, AdpcmImaState};
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Receiver;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embedded_io_async::Seek;
use esp_hal::dma::{AnyI2sDmaChannel, DmaDescriptor};
use esp_hal::dma_buffers;
use esp_hal::gpio::AnyPin;
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use esp_hal::i2s::master::{DataFormat, I2s, Standard};
use esp_hal::i2s::AnyI2s;
use esp_hal::time::Rate;
use heapless::String;
use static_cell::make_static;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

const DMA_SIZE: usize = 4 * 4096;
const DMA_CHUNKS: usize = 5;

static STOP_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static VOLUME: AtomicU8 = AtomicU8::new(8);

pub enum PlayerCommand {
    Stop,
    Play(String<12>),
    VolumeUp,
    VolumeDown,
}

pub struct Player {
    i2s: AnyI2s<'static>,
    dma: AnyI2sDmaChannel<'static>,
    bclk: AnyPin<'static>,
    ws: AnyPin<'static>,
    dout: AnyPin<'static>,
    dma_buffer: &'static mut [u8; DMA_SIZE],
    dma_descriptors: &'static mut [DmaDescriptor; DMA_CHUNKS],
}

impl Player {
    pub fn new(
        i2s: AnyI2s<'static>,
        dma: AnyI2sDmaChannel<'static>,
        bclk: AnyPin<'static>,
        ws: AnyPin<'static>,
        dout: AnyPin<'static>,
    ) -> Self {
        let (_, _, tx_buffer, tx_descriptors) = dma_buffers!(0, 4 * 4096);
        Self {
            i2s,
            dma,
            bclk,
            ws,
            dout,
            dma_buffer: tx_buffer,
            dma_descriptors: tx_descriptors,
        }
    }

    pub fn transfer(&mut self) -> I2sWriteDmaTransferAsync<'_, &mut [u8; DMA_SIZE]> {
        let i2s = I2s::new(
            self.i2s.reborrow(),
            Standard::Philips,
            DataFormat::Data16Channel16,
            Rate::from_hz(44100),
            self.dma.reborrow(),
        )
        .into_async();

        // SAFETY: self.dma_descriptors live forever, the risk is rather that they will still be in
        // use when a new transfer is started. There does not seem to be any sane way to stop the
        // I2S peripheral and DMA transfer and retrieve the descriptor again.
        // TODO: validate that any pending transfer is really finished before we start a new one.
        // Hope that this happens on re-initialization.
        let reborrowed_dma_descriptors = unsafe { extend_to_static(self.dma_descriptors) };

        let i2s_tx = i2s
            .i2s_tx
            .with_bclk(self.bclk.reborrow())
            .with_ws(self.ws.reborrow())
            .with_dout(self.dout.reborrow())
            .build(reborrowed_dma_descriptors);

        i2s_tx
            .write_dma_circular_async::<&mut [u8; DMA_SIZE]>(self.dma_buffer)
            .unwrap()
    }
}

#[embassy_executor::task]
pub async fn run_player(
    spawner: Spawner,
    mut player: Player,
    fs: &'static SdFileSystem<'static>,
    commands: Receiver<'static, NoopRawMutex, PlayerCommand, 2>,
) {
    let pending_buffer = make_static!([0u8; 1024]);
    let ready_buffer = make_static!([0u8; 1024]);

    loop {
        match commands.receive().await {
            PlayerCommand::Stop => {
                info!("Player command: STOP");
                stop_player().await;
            }
            PlayerCommand::VolumeUp => {
                info!("Player command: VOLUME UP");
                VOLUME.update(Ordering::SeqCst, Ordering::SeqCst, |vol| 16.min(vol + 1));
            }
            PlayerCommand::VolumeDown => {
                info!("Player command: VOLUME DOWN");
                VOLUME.update(Ordering::SeqCst, Ordering::SeqCst, |vol| {
                    vol.saturating_sub(1)
                });
            }
            PlayerCommand::Play(file) => {
                info!("Player command: PLAY {}", file);
                stop_player().await;

                // SAFETY: a spawned task needs static lifetimes because it might run forever. In our case the task will be
                // terminated though, before it is restarted with a new file.
                let dma_transfer = unsafe {
                    core::mem::transmute::<
                        I2sWriteDmaTransferAsync<'_, &mut [u8; DMA_SIZE]>,
                        I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
                    >(player.transfer())
                };
                let pending_buffer = unsafe { extend_to_static(pending_buffer) };
                let ready_buffer = unsafe { extend_to_static(ready_buffer) };
                spawner.must_spawn(play_file(
                    fs,
                    file,
                    dma_transfer,
                    pending_buffer,
                    ready_buffer,
                ));
            }
        }
    }
}

async fn stop_player() {
    STOP_SIGNAL.signal(());
    Timer::after_millis(50).await;
    STOP_SIGNAL.reset();
}

#[embassy_executor::task]
async fn play_file(
    fs: &'static SdFileSystem<'static>,
    file_name: String<12>,
    mut dma_transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
    mut pending_buffer: &'static mut [u8; 1024],
    mut ready_buffer: &'static mut [u8; 1024],
) {
    let root = fs.root_dir();
    let mut file = root.open_file(&file_name).await.unwrap();
    // skip header
    file.seek(embedded_io::SeekFrom::Start(48)).await.unwrap();

    let mut next_block = match read_block(&mut file, ready_buffer).await {
        Ok(BlockReadResult::Full) => &ready_buffer[..],
        Ok(BlockReadResult::Eof) => return, // cleanup DMA buffer afterwards
        Ok(BlockReadResult::Partial(size)) => {
            if size < 4 {
                return; // would crash the decoder
            } else {
                &ready_buffer[..size]
            }
        }
        Err(err) => {
            error!("reading audio file: {}", err);
            return;
        }
    };
    let mut read_future = read_block(&mut file, pending_buffer);
    loop {
        let volume = VOLUME.load(Ordering::SeqCst);
        let apply_volume = |sample: i16| (sample as i32 * volume as i32 / 16) as i16;

        let mut decoder = #[coroutine]
        || {
            let mut state = AdpcmImaState::new();
            state.predictor = i16::from_le_bytes([next_block[0], next_block[1]]);
            state.step_index = next_block[2].min(88);
            let sample = state.predictor;
            let sample = apply_volume(sample);
            yield sample as u8;
            yield (sample >> 8) as u8;
            yield 0;
            yield 0;

            for b in &next_block[4..] {
                let sample = decode_adpcm_ima(*b & 0x0f, &mut state);
                let sample = apply_volume(sample);
                yield sample as u8;
                yield (sample >> 8) as u8;
                yield 0;
                yield 0;

                let sample = decode_adpcm_ima(*b >> 4, &mut state);
                let sample = apply_volume(sample);
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

        let player_future = join(transfer_future, read_future);
        let read_result = match select(STOP_SIGNAL.wait(), player_future).await {
            Either::First(_) => return,
            Either::Second((_, read_result)) => read_result,
        };

        (pending_buffer, ready_buffer) = (ready_buffer, pending_buffer);

        next_block = match read_result {
            Ok(BlockReadResult::Full) => &ready_buffer[..],
            Ok(BlockReadResult::Eof) => return, // cleanup DMA buffer afterwards
            Ok(BlockReadResult::Partial(size)) => {
                if size < 4 {
                    return; // would crash the decoder
                } else {
                    &ready_buffer[..size]
                }
            }
            Err(err) => {
                error!("reading audio file: {}", err);
                return;
            }
        };

        read_future = read_block(&mut file, pending_buffer);
    }
}

enum BlockReadResult {
    Full, // full buffer
    Partial(usize),
    Eof, // buffer is empty
}

async fn read_block<R>(file: &mut R, mut buf: &mut [u8]) -> Result<BlockReadResult, R::Error>
where
    R: embedded_io_async::Read,
    R::Error: defmt::Format,
{
    let full_size = buf.len();

    while !buf.is_empty() {
        match retry(async || file.read(buf).await, 2).await {
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
