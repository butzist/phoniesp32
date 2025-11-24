use crate::PrintErr;
use crate::entities::playlist::{PlayListRef, Playlist};
use crate::player::playback::toggle_pause_player;
use crate::player::status::{AudioFileWithMetadata, PlaylistWithMetadata};
use crate::sd::SdFileSystem;
use alloc::vec::Vec;
use defmt::info;

use super::playback::{play_files, skip_player, stop_player, volume_down, volume_up};

#[derive(Clone, Copy)]
pub enum Skip {
    Next,
    Previous,
}

pub enum PlayerCommand {
    Stop,
    Playlist(Playlist),
    PlaylistRef(PlayListRef),
    Pause,
    VolumeUp,
    VolumeDown,
    Skip(Skip),
}

pub async fn handle_command(
    command: PlayerCommand,
    fs: &'static SdFileSystem<'static>,
    player: &mut super::playback::Player,
    status: &super::status::Status,
    spawner: &embassy_executor::Spawner,
) {
    match command {
        PlayerCommand::Stop => {
            info!("Player command: STOP");
            stop_player().await;
        }
        PlayerCommand::Pause => {
            info!("Player command: PAUSE");
            toggle_pause_player();
        }
        PlayerCommand::VolumeUp => {
            info!("Player command: VOLUME UP");
            volume_up();
        }
        PlayerCommand::VolumeDown => {
            info!("Player command: VOLUME DOWN");
            volume_down();
        }
        PlayerCommand::Skip(skip) => match skip {
            Skip::Next => {
                info!("Player command: SKIP NEXT");
                skip_player(skip).await;
            }
            Skip::Previous => {
                info!("Player command: SKIP PREVIOUS");
                skip_player(skip).await;
            }
        },
        PlayerCommand::Playlist(playlist) => {
            info!(
                "Player command: PLAY LIST with {} files",
                playlist.files.len()
            );

            play_playlist(playlist, fs, player, status, spawner).await;
        }
        PlayerCommand::PlaylistRef(playlist_ref) => {
            if let Some(playlist) = playlist_ref
                .read(fs)
                .await
                .print_err("Failed to read playlist")
            {
                info!(
                    "Player command: PLAY LIST REF with {} files",
                    playlist.files.len()
                );

                play_playlist(playlist, fs, player, status, spawner).await;
            }
        }
    }
}

async fn play_playlist(
    playlist: Playlist,
    fs: &'static SdFileSystem<'_>,
    player: &mut super::Player,
    status: &super::Status,
    spawner: &embassy_executor::Spawner,
) {
    let playlist_with_metadata = playlist_with_metadata_from_playlist(&playlist, fs).await;
    status.update_playlist(Some(playlist_with_metadata));

    play_files(fs, playlist.files, player, spawner).await;
}

async fn playlist_with_metadata_from_playlist(
    playlist: &Playlist,
    fs: &crate::sd::SdFileSystem<'_>,
) -> PlaylistWithMetadata {
    let mut files_with_metadata = Vec::new();

    for file in &playlist.files {
        let metadata = file.metadata(fs).await.unwrap_or_default();
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
