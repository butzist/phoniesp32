use core::ops::Coroutine;
use core::pin::Pin;

use crate::PrintErr;
use crate::{retry, sd::SdFileSystem};
use audio_codec_algorithms::{decode_adpcm_ima, AdpcmImaState};
use defmt::error;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Receiver;
use embassy_sync::signal::Signal;
use embedded_io_async::{Read, Seek};
use esp_hal::i2s::master::asynch::I2sWriteDmaTransferAsync;
use heapless::String;
use static_cell::make_static;
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

static STOP_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub enum PlayerCommand {
    Stop,
    Play(String<12>),
}

#[embassy_executor::task]
pub async fn player(
    mut dma_transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; 4 * 4096]>,
    fs: &'static SdFileSystem<'static>,
    commands: Receiver<'static, NoopRawMutex, PlayerCommand, 2>,
) {
    loop {
        match commands.receive().await {
            PlayerCommand::Stop => todo!(),
            PlayerCommand::Play(file) => {
                // TODO spawn
                play_file(fs, file, &mut dma_transfer).await;
            }
        }
    }
}

async fn play_file(
    fs: &'static SdFileSystem<'static>,
    file_name: String<12>,
    dma_transfer: &mut I2sWriteDmaTransferAsync<'static, &'static mut [u8; 4 * 4096]>,
) {
    let root = fs.root_dir();
    let mut file = root.open_file(&file_name).await.unwrap();
    // skip header
    file.seek(embedded_io::SeekFrom::Start(48)).await.unwrap();

    let mut pending_buffer = make_static!([0u8; 1024]);
    let mut ready_buffer = make_static!([0u8; 1024]);

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

        let player_future = join(transfer_future, read_future);
        let read_result = match select(player_future, STOP_SIGNAL.wait()).await {
            Either::First((_, read_result)) => read_result,
            Either::Second(_) => return,
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
