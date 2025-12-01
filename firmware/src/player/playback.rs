use core::f32::consts::PI;
use core::iter::repeat_n;
use core::ops::Coroutine;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::entities::audio_file::AudioFile;
use crate::player::control::Skip;
use crate::player::status::{State, Status};
use crate::retry;
use crate::sd::{PlaybackGuard, SdFileSystem};
use crate::{PrintErr, extend_to_static};
use audio_codec_algorithms::{AdpcmImaState, decode_adpcm_ima};
use defmt::{error, warn};
use embassy_futures::join::join;
use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::lazy_lock::LazyLock;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use esp_hal::dma::{AnyI2sDmaChannel, DmaDescriptor};
use esp_hal::i2s::master::{I2s, asynch::I2sWriteDmaTransferAsync};

use {esp_backtrace as _, esp_println as _};

extern crate alloc;
use alloc::vec::Vec;

const DMA_SIZE: usize = 6 * 4096;
const DMA_CHUNKS: usize = 7;

// FIXME: find a better (encapsulated) way to share this signal
pub static STOP_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static SKIP_SIGNAL: Signal<CriticalSectionRawMutex, super::control::Skip> = Signal::new();
static VOLUME: AtomicU8 = AtomicU8::new(8);
static PAUSED: AtomicBool = AtomicBool::new(false);

pub struct Player {
    pub i2s: esp_hal::i2s::AnyI2s<'static>,
    pub dma: AnyI2sDmaChannel<'static>,
    pub bclk: esp_hal::gpio::AnyPin<'static>,
    pub ws: esp_hal::gpio::AnyPin<'static>,
    pub dout: esp_hal::gpio::AnyPin<'static>,
    pub dma_buffer: &'static mut [u8; DMA_SIZE],
    pub dma_descriptors: &'static mut [DmaDescriptor; DMA_CHUNKS],
}

impl Player {
    pub fn new(
        i2s: esp_hal::i2s::AnyI2s<'static>,
        dma: AnyI2sDmaChannel<'static>,
        bclk: esp_hal::gpio::AnyPin<'static>,
        ws: esp_hal::gpio::AnyPin<'static>,
        dout: esp_hal::gpio::AnyPin<'static>,
    ) -> Self {
        let (_, _, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(0, DMA_SIZE);
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
        let config = esp_hal::i2s::master::Config::new_tdm_philips()
            .with_sample_rate(esp_hal::time::Rate::from_hz(44100))
            .with_data_format(esp_hal::i2s::master::DataFormat::Data16Channel16);

        let i2s = I2s::new(self.i2s.reborrow(), self.dma.reborrow(), config)
            .unwrap()
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

pub async fn play_files(
    fs: &'static crate::sd::SdFsWrapper,
    files: Vec<AudioFile>,
    player: &mut Player,
    spawner: &embassy_executor::Spawner,
) {
    let fs_guard = fs.borrow_for_playback().await;

    // SAFETY: a spawned task needs static lifetimes because it might run forever. In our case the task will be
    // terminated though, before it is restarted with a new file.
    let dma_transfer = unsafe {
        core::mem::transmute::<
            I2sWriteDmaTransferAsync<'_, &mut [u8; DMA_SIZE]>,
            I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
        >(player.transfer())
    };
    spawner.must_spawn(play_files_task(fs_guard, files, dma_transfer));
}

#[embassy_executor::task]
async fn play_files_task(
    fs: PlaybackGuard<'static>,
    files: Vec<AudioFile>,
    mut dma_transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
) {
    play_beep(4, &mut dma_transfer).await;

    Status::get().update_state(State::Playing);
    play_files_task_inner(fs, files, dma_transfer).await;
    Status::get().update_state(State::Stopped);
}

enum ExitReason {
    Finished,
    Error,
    Stopped,
    Skipped(Skip),
}

async fn play_files_task_inner(
    fs: PlaybackGuard<'static>,
    files: Vec<AudioFile>,
    mut dma_transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
) {
    let files_vec = files;
    let mut current_index = 0;
    let mut first = true;

    while current_index < files_vec.len() {
        if !first {
            // sleep for ~2s
            play_samples_from_iterator(repeat_n(0, 2 * 44100), &mut dma_transfer).await;
            if STOP_SIGNAL.signaled() {
                STOP_SIGNAL.reset();
                return;
            }
            first = false;
        }

        warn!("current: {}", current_index);
        Status::get().update_file(current_index);
        let file = files_vec[current_index].clone();
        let reason = play_file(&fs, file, &mut dma_transfer).await;

        match reason {
            ExitReason::Finished => current_index += 1,
            ExitReason::Error => current_index += 1,
            ExitReason::Stopped => return,
            ExitReason::Skipped(skip) => match skip {
                super::control::Skip::Next => {
                    current_index += 1;
                }
                super::control::Skip::Previous => {
                    current_index = current_index.saturating_sub(1);
                }
            },
        }
    }
}

pub async fn stop_player() {
    STOP_SIGNAL.signal(());
    PAUSED.store(false, Ordering::SeqCst);
    Timer::after_millis(50).await;
    STOP_SIGNAL.reset();
}

pub async fn skip_player(skip: super::control::Skip) {
    SKIP_SIGNAL.signal(skip);
    PAUSED.store(false, Ordering::SeqCst);
    Timer::after_millis(50).await;
    SKIP_SIGNAL.reset();
}

pub fn toggle_pause_player() {
    PAUSED.update(Ordering::SeqCst, Ordering::SeqCst, |paused| !paused);
}

pub fn volume_up() {
    VOLUME.update(Ordering::SeqCst, Ordering::SeqCst, |vol| 16.min(vol + 1));
}

pub fn volume_down() {
    VOLUME.update(Ordering::SeqCst, Ordering::SeqCst, |vol| 1.max(vol - 1));
}

pub fn get_volume() -> u8 {
    VOLUME.load(Ordering::SeqCst)
}

async fn play_beep(
    duration_100ms: u16,
    dma_transfer: &mut I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
) {
    let freq = 1000.;
    let samples = (0..44100 / 100 * duration_100ms)
        .map(|i| (libm::sinf(i as f32 / 44100. * freq * 2. * PI) * i16::MAX as f32) as i16);

    play_samples_from_iterator(samples, dma_transfer).await;
}

async fn play_samples_from_iterator(
    iter: impl IntoIterator<Item = i16>,
    dma_transfer: &mut I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
) {
    let mut done = false;
    let mut iter = iter.into_iter();
    while !done {
        dma_transfer
            .push_with(|buf: &mut [u8]| {
                let n_samples = buf.len() / 4;
                for n in 0..n_samples {
                    if let Some(sample) = iter.next() {
                        buf[n * 4] = sample as u8;
                        buf[n * 4 + 1] = (sample >> 8) as u8;
                        buf[n * 4 + 2] = 0;
                        buf[n * 4 + 3] = 0;
                    } else {
                        done = true;
                        return n * 4;
                    }
                }

                n_samples * 4
            })
            .await
            .print_err("I2S DMA transfer");
    }
}

async fn play_file<'a>(
    fs: &'a SdFileSystem,
    file: AudioFile,
    dma_transfer: &'a mut I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
) -> ExitReason {
    let Ok(mut file_handle) = file.data_reader(fs).await else {
        return ExitReason::Error;
    };

    static PENDING_BUFFER: LazyLock<Mutex<CriticalSectionRawMutex, [u8; 1024]>> =
        LazyLock::new(|| Mutex::new([0u8; 1024]));
    static READY_BUFFER: LazyLock<Mutex<CriticalSectionRawMutex, [u8; 1024]>> =
        LazyLock::new(|| Mutex::new([0u8; 1024]));

    let mut pending_buffer = PENDING_BUFFER.get().lock().await;
    let mut pending_buffer = &mut *pending_buffer;
    let mut ready_buffer = READY_BUFFER.get().lock().await;
    let mut ready_buffer = &mut *ready_buffer;

    let mut next_block = match read_block(&mut file_handle, ready_buffer).await {
        Ok(BlockReadResult::Full) => &ready_buffer[..],
        Ok(BlockReadResult::Eof) => return ExitReason::Finished, // cleanup DMA buffer afterwards
        Ok(BlockReadResult::Partial(size)) => {
            if size < 4 {
                return ExitReason::Finished; // would crash the decoder
            } else {
                &ready_buffer[..size]
            }
        }
        Err(err) => {
            error!("reading audio file: {}", err);
            return ExitReason::Error;
        }
    };
    let mut read_future = read_block(&mut file_handle, pending_buffer);

    let mut bytes_read = 0;
    let mut last_position_update = 0;

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
        let read_result = match select3(STOP_SIGNAL.wait(), SKIP_SIGNAL.wait(), player_future).await
        {
            Either3::First(_) => return ExitReason::Stopped,
            Either3::Second(skip) => return ExitReason::Skipped(skip),
            Either3::Third((_, read_result)) => read_result,
        };

        // send zero samples while paused
        if PAUSED.load(Ordering::SeqCst) {
            Status::get().update_state(State::Paused);

            while PAUSED.load(Ordering::SeqCst) {
                dma_transfer
                    .push_with(|buf: &mut [u8]| {
                        let mut bytes = 0usize;
                        while bytes + 4 <= buf.len() {
                            buf[bytes] = 0;
                            buf[bytes + 1] = 0;
                            buf[bytes + 2] = 0;
                            buf[bytes + 3] = 0;
                            bytes += 4;
                        }

                        bytes
                    })
                    .await
                    .print_err("I2S DMA transfer");
            }

            Status::get().update_state(State::Playing);
        }

        // Update position tracking
        bytes_read += next_block.len();
        let current_position = (bytes_read / 22050) as u32; // 2 samples per byte @ 44.1 kHz
        if current_position != last_position_update {
            Status::get().update_position(current_position);
            last_position_update = current_position;
        }

        (pending_buffer, ready_buffer) = (ready_buffer, pending_buffer);

        next_block = match read_result {
            Ok(BlockReadResult::Full) => &ready_buffer[..],
            Ok(BlockReadResult::Eof) => return ExitReason::Finished, // cleanup DMA buffer afterwards
            Ok(BlockReadResult::Partial(size)) => {
                if size < 4 {
                    return ExitReason::Finished; // would crash the decoder
                } else {
                    &ready_buffer[..size]
                }
            }
            Err(err) => {
                error!("reading audio file: {}", err);
                return ExitReason::Error;
            }
        };

        read_future = read_block(&mut file_handle, pending_buffer);
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
