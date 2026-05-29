pub mod status;

use core::cell::RefCell;
use core::sync::atomic::{AtomicU8, Ordering};

use alloc::boxed::Box;
use defmt::{debug, info, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::{Channel, Sender};
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use futures::future::LocalBoxFuture;

use self::status::{AudioFileWithMetadata, PlaylistWithMetadata, State, Status};
use crate::PrintErr;
use crate::drivers::audio::{AudioBuffer, AudioPacket, AudioSender, BUF_SAMPLES, Player};
use crate::drivers::sd::{PlaybackGuard, SdFsWrapper};
use crate::entities::audio_file::{AudioDecoder, AudioFile};
use crate::entities::playlist::{PlayListRef, Playlist};

extern crate alloc;
use alloc::rc::Rc;
use alloc::vec::Vec;

// ---- Command protocol ----

#[derive(Clone, Copy, defmt::Format)]
pub enum Skip {
    Next,
    Previous,
}

#[derive(defmt::Format)]
pub enum PlayerCommand {
    Stop,
    PlayFile(AudioFile),
    Playlist(Playlist),
    PlaylistRef(PlayListRef),
    Pause,
    SetVolume(u8),
    VolumeUp,
    VolumeDown,
    Skip(Skip),
}

// ---- Shared playback state ----

struct PlaybackContext {
    desired_state: &'static Watch<CriticalSectionRawMutex, State, 2>,
    skip_signal: &'static Signal<CriticalSectionRawMutex, Skip>,
    volume: &'static AtomicU8,
    status: &'static Status,
}

impl PlaybackContext {
    fn new() -> Self {
        let desired_state =
            crate::mk_static!(Watch<CriticalSectionRawMutex, State, 2>, Watch::new());
        let skip_signal = crate::mk_static!(Signal<CriticalSectionRawMutex, Skip>, Signal::new());
        let volume = crate::mk_static!(AtomicU8, AtomicU8::new(8));
        let status = crate::mk_static!(Status, Status::new());

        desired_state.sender().send(State::Stopped);

        Self {
            desired_state,
            skip_signal,
            volume,
            status,
        }
    }

    fn set_desired_state(&self, state: State) {
        self.desired_state.sender().send(state);
    }

    async fn wait_for_desired_state(&self, state: State) {
        let mut rx = self.desired_state.receiver().unwrap();
        while rx.try_get().unwrap() != state {
            rx.changed().await;
        }
    }
}

// ---- PlaybackController ----

pub struct PlaybackController {
    player: Rc<RefCell<Player>>,
    fs: &'static SdFsWrapper,
    context: &'static PlaybackContext,
}

impl PlaybackController {
    pub fn new(player: Player, fs: &'static SdFsWrapper) -> Self {
        let context = crate::mk_static!(PlaybackContext, PlaybackContext::new());
        Self {
            player: Rc::new(RefCell::new(player)),
            fs,
            context,
        }
    }

    pub fn spawn(self, spawner: &Spawner) -> PlaybackHandle {
        let command_channel =
            crate::mk_static!(Channel<NoopRawMutex, PlayerCommand, 2>, Channel::new());

        let handle = PlaybackHandle::new(command_channel.sender(), self.context);

        let player_for_task = self.player.clone();
        spawner.must_spawn(controller_task(
            player_for_task,
            self.fs,
            command_channel,
            self.context,
            *spawner,
        ));

        handle
    }
}

#[embassy_executor::task]
async fn controller_task(
    player: Rc<RefCell<Player>>,
    fs: &'static SdFsWrapper,
    command_channel: &'static Channel<NoopRawMutex, PlayerCommand, 2>,
    context: &'static PlaybackContext,
    spawner: Spawner,
) {
    let receiver = command_channel.receiver();

    loop {
        let command = receiver.receive().await;
        handle_command(command, player.clone(), fs, &spawner, context).await;
    }
}

// ---- Command dispatch ----

async fn stop_playback(context: &PlaybackContext) {
    debug!("Playback: requesting STOP");
    context.set_desired_state(State::Stopped);
    context.status.wait_for_state(State::Stopped).await;
    debug!("Playback: STOP confirmed");
}

async fn handle_command(
    command: PlayerCommand,
    player: Rc<RefCell<Player>>,
    fs: &'static SdFsWrapper,
    spawner: &Spawner,
    context: &'static PlaybackContext,
) {
    match command {
        PlayerCommand::Stop => {
            info!("Playback: command: STOP");
            stop_playback(context).await;
        }
        PlayerCommand::Pause => {
            info!("Playback: command: PAUSE");
            let current = context.desired_state.try_get().unwrap();
            let new = match current {
                State::Playing => State::Paused,
                State::Paused => State::Playing,
                State::Stopped => State::Stopped,
            };
            context.set_desired_state(new);
        }
        PlayerCommand::VolumeUp => {
            info!("Playback: command: VOLUME UP");
            context
                .volume
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |vol| {
                    Some(16.min(vol + 1))
                })
                .ok();
        }
        PlayerCommand::VolumeDown => {
            info!("Playback: command: VOLUME DOWN");
            context
                .volume
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |vol| {
                    Some(1.max(vol - 1))
                })
                .ok();
        }
        PlayerCommand::SetVolume(vol) => {
            info!("Playback: command: SET VOLUME {}", vol);
            context.volume.store(16.min(vol), Ordering::SeqCst);
        }
        PlayerCommand::Skip(skip) => {
            info!("Playback: command: SKIP {:?}", skip);
            context.skip_signal.signal(skip);
        }
        PlayerCommand::PlayFile(file) => {
            info!("Playback: command: PLAY FILE {}", file.name());
            stop_playback(context).await;
            let playlist = Playlist::new("SINGLE".try_into().unwrap(), alloc::vec![file]);
            play_playlist(playlist, player, fs, spawner, context).await;
        }
        PlayerCommand::Playlist(playlist) => {
            info!(
                "Playback: command: PLAY LIST with {} files",
                playlist.files.len()
            );
            stop_playback(context).await;
            play_playlist(playlist, player, fs, spawner, context).await;
        }
        PlayerCommand::PlaylistRef(playlist_ref) => {
            info!("Playback: command: PLAY LIST REF {}", playlist_ref);
            stop_playback(context).await;
            let fs_guard = fs.borrow_mut().await;
            if let Some(playlist) = playlist_ref
                .read(&fs_guard)
                .await
                .print_err("Playback: Failed to read playlist")
            {
                drop(fs_guard);
                play_playlist(playlist, player, fs, spawner, context).await;
            }
        }
    }
}

async fn play_playlist(
    playlist: Playlist,
    player: Rc<RefCell<Player>>,
    fs: &'static SdFsWrapper,
    spawner: &Spawner,
    context: &'static PlaybackContext,
) {
    debug!("Playback: building playlist metadata");
    let playlist_with_metadata = playlist_with_metadata_from_playlist(&playlist, fs).await;
    context.status.update_playlist(Some(playlist_with_metadata));

    debug!("Playback: borrowing fs for playback");
    let stop_fn: Box<dyn Fn() -> LocalBoxFuture<'static, ()>> =
        Box::new(move || Box::pin(async move { context.set_desired_state(State::Stopped) }));
    let fs_guard = fs.borrow_for_playback(stop_fn).await;
    debug!("Playback: fs borrowed, spawning playlist task");

    spawner.must_spawn(playlist_task(
        fs_guard,
        playlist.files,
        player,
        context,
        *spawner,
    ));
}

// ---- Helpers ----

struct SendInterrupted;

struct PlaybackStream<'a> {
    sender: &'a AudioSender,
    context: &'a PlaybackContext,
}

impl<'a> PlaybackStream<'a> {
    async fn send_packet(&self, packet: AudioPacket) -> Result<(), SendInterrupted> {
        match select(
            self.sender.send(packet),
            self.context.wait_for_desired_state(State::Stopped),
        )
        .await
        {
            Either::First(_) => Ok(()),
            Either::Second(_) => Err(SendInterrupted),
        }
    }

    async fn play_beep(&self, duration_100ms: u32) -> Result<(), SendInterrupted> {
        let total_samples = duration_100ms as usize * 4410;
        let mut remaining = total_samples;
        let mut phase: f32 = 0.0;
        let phase_step = 2.0f32 * core::f32::consts::PI * 1000.0f32 / 44100.0f32;

        while remaining > 0 {
            let mut buf = AudioBuffer::alloc();
            let n = remaining.min(BUF_SAMPLES);
            for i in 0..n {
                let sample = (libm::sinf(phase) * 16384.0f32) as i16;
                buf.samples[i] = sample;
                phase += phase_step;
                if phase >= 2.0f32 * core::f32::consts::PI {
                    phase -= 2.0f32 * core::f32::consts::PI;
                }
            }
            buf.len = n;

            self.send_packet(AudioPacket::Buffer(buf)).await?;
            remaining -= n;
        }

        Ok(())
    }

    async fn play_silence(&self, duration_secs: u32) -> Result<(), SendInterrupted> {
        let total = duration_secs * 44100;
        self.send_packet(AudioPacket::Silence(total)).await?;
        Ok(())
    }

    async fn handle_pause(&self) -> Result<(), SendInterrupted> {
        let mut rx = self.context.desired_state.receiver().unwrap();
        let mut desired_state = rx.try_get().unwrap();
        loop {
            match desired_state {
                State::Paused => {
                    debug!("Playback: paused");
                    self.context.status.update_state(State::Paused);
                    match select(
                        self.sender.send(AudioPacket::Silence(BUF_SAMPLES as u32)),
                        rx.changed(),
                    )
                    .await
                    {
                        Either::First(_) => {}
                        Either::Second(value) => {
                            debug!("Playback: state changed while paused");
                            desired_state = value;

                            if desired_state == State::Playing {
                                debug!("Playback: resuming from pause");
                                self.context.status.update_state(State::Playing);
                                return Ok(());
                            }
                        }
                    }
                }
                State::Playing => {
                    return Ok(());
                }
                State::Stopped => {
                    debug!("Playback: stopped while paused");
                    return Err(SendInterrupted);
                }
            }
        }
    }
}

fn handle_skip(skip: Skip, current_index: &mut usize, total_files: usize) {
    match skip {
        Skip::Next => *current_index = (*current_index + 1).min(total_files.saturating_sub(1)),
        Skip::Previous => *current_index = current_index.saturating_sub(1),
    }
}

// ---- Playlist task (decoder + channel producer) ----

#[embassy_executor::task]
async fn playlist_task(
    fs_guard: PlaybackGuard<'static>,
    files: Vec<AudioFile>,
    player: Rc<RefCell<Player>>,
    context: &'static PlaybackContext,
    spawner: Spawner,
) {
    context.set_desired_state(State::Playing);
    debug!(
        "Playback: playlist task starting with {} files",
        files.len()
    );

    debug!("Playback: starting I2S output task");
    // Start the I2S output task
    let sender = Player::start(player, &spawner, context.status);
    debug!("Playback: I2S output task started");

    let stream = PlaybackStream {
        sender: &sender,
        context,
    };
    let _ = stream.playlist_task_inner(fs_guard, files).await;
    stream.close().await;
}

impl PlaybackStream<'_> {
    async fn close(&self) {
        debug!("Playback: sending EOF to I2S task");
        self.sender.send(AudioPacket::Eof).await;
        debug!("Playback: waiting for STOP confirmation");
        self.context.status.wait_for_state(State::Stopped).await;
        debug!("Playback: stream closed");
    }

    async fn playlist_task_inner(
        &self,
        fs_guard: PlaybackGuard<'static>,
        files: Vec<AudioFile>,
    ) -> Result<(), SendInterrupted> {
        debug!("Playback: playing start beep");
        self.play_beep(1).await?;

        let mut current_index: usize = 0;
        let total_files = files.len();

        while current_index < total_files {
            if current_index > 0 {
                self.play_silence(1).await?;
            }

            self.context.status.update_file(current_index);
            debug!(
                "Playback: opening file {} (index {})",
                files[current_index].name(),
                current_index
            );

            let file_handle = match files[current_index].data_reader(&fs_guard).await {
                Ok(fh) => fh,
                Err(_) => {
                    warn!("Playback: could not read file at index {}", current_index);
                    current_index += 1;
                    continue;
                }
            };
            let mut decoder = AudioDecoder::new(file_handle);

            let mut total_samples: u64 = 0;
            let mut last_position_update: u32 = 0;

            loop {
                self.handle_pause().await?;

                let mut buf = AudioBuffer::alloc();

                let n = match select3(
                    decoder.next_samples(&mut buf.samples),
                    self.context.skip_signal.wait(),
                    self.context.wait_for_desired_state(State::Stopped),
                )
                .await
                {
                    Either3::First(Ok(n)) => n,
                    Either3::First(Err(_)) => {
                        warn!("Playback: file read error");
                        0
                    }
                    Either3::Second(skip) => {
                        debug!("Playback: skip {:?} during decode", skip);
                        self.context.skip_signal.reset();
                        handle_skip(skip, &mut current_index, total_files);
                        break;
                    }
                    Either3::Third(_) => {
                        debug!("Playback: stopped during decode");
                        return Err(SendInterrupted);
                    }
                };

                if n == 0 {
                    debug!("Playback: file {} done, moving to next", current_index);
                    current_index += 1;
                    break;
                }

                buf.len = n;

                let vol = self.context.volume.load(Ordering::SeqCst);
                for s in buf.samples[..n].iter_mut() {
                    *s = (*s as i32 * vol as i32 / 16) as i16;
                }

                total_samples += n as u64;
                let position = (total_samples / 44100) as u32;
                if position != last_position_update {
                    self.context.status.update_position(position);
                    last_position_update = position;
                }

                match select3(
                    self.sender.send(AudioPacket::Buffer(buf)),
                    self.context.skip_signal.wait(),
                    self.context.wait_for_desired_state(State::Stopped),
                )
                .await
                {
                    Either3::First(_) => {}
                    Either3::Second(skip) => {
                        debug!("Playback: skip {:?} during send", skip);
                        self.context.skip_signal.reset();
                        handle_skip(skip, &mut current_index, total_files);
                        break;
                    }
                    Either3::Third(_) => {
                        debug!("Playback: stopped during send");
                        return Err(SendInterrupted);
                    }
                }
            }
        }

        Ok(())
    }
}

async fn playlist_with_metadata_from_playlist(
    playlist: &Playlist,
    fs: &SdFsWrapper,
) -> PlaylistWithMetadata {
    let mut files_with_metadata = Vec::new();
    let fs_guard = fs.borrow_mut().await;
    for file in &playlist.files {
        let metadata = file.metadata(&fs_guard).await.unwrap_or_default();
        files_with_metadata.push(AudioFileWithMetadata {
            file: file.clone(),
            metadata,
        });
    }
    PlaylistWithMetadata {
        playlist_name: playlist.name.clone(),
        files: files_with_metadata,
    }
}

// ---- PlaybackHandle ----

#[derive(Clone)]
pub struct PlaybackHandle {
    sender: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    context: &'static PlaybackContext,
}

impl PlaybackHandle {
    fn new(
        sender: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
        context: &'static PlaybackContext,
    ) -> Self {
        Self { sender, context }
    }

    pub async fn stop(&self) {
        self.sender.send(PlayerCommand::Stop).await;
    }

    pub async fn pause(&self) {
        self.sender.send(PlayerCommand::Pause).await;
    }

    pub async fn play_file(&self, file: AudioFile) {
        self.sender.send(PlayerCommand::PlayFile(file)).await;
    }

    pub async fn play_playlist(&self, playlist: Playlist) {
        self.sender.send(PlayerCommand::Playlist(playlist)).await;
    }

    pub async fn play_playlist_ref(&self, playlist_ref: PlayListRef) {
        self.sender
            .send(PlayerCommand::PlaylistRef(playlist_ref))
            .await;
    }

    pub async fn set_volume(&self, volume: u8) {
        self.sender.send(PlayerCommand::SetVolume(volume)).await;
    }

    pub async fn volume_up(&self) {
        self.sender.send(PlayerCommand::VolumeUp).await;
    }

    pub async fn volume_down(&self) {
        self.sender.send(PlayerCommand::VolumeDown).await;
    }

    pub async fn skip_next(&self) {
        self.sender.send(PlayerCommand::Skip(Skip::Next)).await;
    }

    pub async fn skip_previous(&self) {
        self.sender.send(PlayerCommand::Skip(Skip::Previous)).await;
    }

    pub fn get_volume(&self) -> u8 {
        self.context.volume.load(Ordering::SeqCst)
    }

    pub fn status(&self) -> &Status {
        self.context.status
    }
}
