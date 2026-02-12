use crate::PrintErr;
use crate::entities::playlist::{PlayListRef, Playlist};
use crate::player::playback::toggle_pause_player;
use crate::player::status::{AudioFileWithMetadata, PlaylistWithMetadata};

use alloc::vec::Vec;
use core::sync::atomic::AtomicU8;
use defmt::info;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;

use super::playback::{play_files, skip_player, stop_player, volume_down, volume_up};

#[derive(Clone, Copy, defmt::Format)]
pub enum Skip {
    Next,
    Previous,
}

#[derive(defmt::Format)]
pub enum PlayerCommand {
    Stop,
    Playlist(Playlist),
    PlaylistRef(PlayListRef),
    Pause,
    VolumeUp,
    VolumeDown,
    Skip(Skip),
}

#[derive(Clone)]
pub struct PlayerHandle {
    sender: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
    volume: &'static AtomicU8,
}

impl PlayerHandle {
    pub fn new(
        sender: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
        volume: &'static AtomicU8,
    ) -> Self {
        Self { sender, volume }
    }

    pub async fn stop(&self) {
        self.sender.send(PlayerCommand::Stop).await;
    }

    pub async fn pause(&self) {
        self.sender.send(PlayerCommand::Pause).await;
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

    pub async fn play_playlist(&self, playlist: Playlist) {
        self.sender.send(PlayerCommand::Playlist(playlist)).await;
    }

    pub async fn play_playlist_ref(&self, playlist_ref: PlayListRef) {
        self.sender
            .send(PlayerCommand::PlaylistRef(playlist_ref))
            .await;
    }

    pub fn get_volume(&self) -> u8 {
        self.volume.load(core::sync::atomic::Ordering::SeqCst)
    }
}

pub async fn handle_command(
    command: PlayerCommand,
    player: &mut super::playback::Player,
    status: &super::status::Status,
    spawner: &embassy_executor::Spawner,
) {
    match command {
        PlayerCommand::Stop => {
            info!("Player command: STOP");
            stop_player(player).await;
        }
        PlayerCommand::Pause => {
            info!("Player command: PAUSE");
            toggle_pause_player(player);
        }
        PlayerCommand::VolumeUp => {
            info!("Player command: VOLUME UP");
            volume_up(player);
        }
        PlayerCommand::VolumeDown => {
            info!("Player command: VOLUME DOWN");
            volume_down(player);
        }
        PlayerCommand::Skip(skip) => match skip {
            Skip::Next => {
                info!("Player command: SKIP NEXT");
                skip_player(skip, player).await;
            }
            Skip::Previous => {
                info!("Player command: SKIP PREVIOUS");
                skip_player(skip, player).await;
            }
        },
        PlayerCommand::Playlist(playlist) => {
            info!(
                "Player command: PLAY LIST with {} files",
                playlist.files.len()
            );

            stop_player(player).await;
            play_playlist(playlist, player, status, spawner).await;
        }
        PlayerCommand::PlaylistRef(playlist_ref) => {
            info!("Player command: PLAY LIST REF {}", playlist_ref,);

            stop_player(player).await;
            let fs_guard = player.fs.borrow_mut().await;
            if let Some(playlist) = playlist_ref
                .read(&fs_guard)
                .await
                .print_err("Failed to read playlist")
            {
                drop(fs_guard);
                play_playlist(playlist, player, status, spawner).await;
            }
        }
    }
}

async fn play_playlist(
    playlist: Playlist,
    player: &mut super::Player,
    status: &super::Status,
    spawner: &embassy_executor::Spawner,
) {
    let playlist_with_metadata = playlist_with_metadata_from_playlist(&playlist, player.fs).await;
    status.update_playlist(Some(playlist_with_metadata));

    play_files(playlist.files, player, spawner).await;
}

async fn playlist_with_metadata_from_playlist(
    playlist: &Playlist,
    fs: &crate::sd::SdFsWrapper,
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
