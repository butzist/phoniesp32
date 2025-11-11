use alloc::vec::Vec;
use embassy_sync::lazy_lock::LazyLock;
use embassy_sync::watch::Watch;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Receiver};
use heapless::String;
use serde::Serialize;

use crate::entities::audio_file::{AudioFile, AudioMetadata};

pub struct Status {
    playback_position: Watch<CriticalSectionRawMutex, u32, 1>,
    playback_status: Watch<CriticalSectionRawMutex, PlaybackStatus, 1>,
    current_playlist: Watch<CriticalSectionRawMutex, Option<PlaylistWithMetadata>, 1>,
}

#[derive(Clone, Serialize)]
pub struct PlaybackStatus {
    pub state: State,
    pub metadata: Option<AudioMetadata>,
    pub index_in_playlist: usize,
    pub file_name: Option<String<8>>,
    pub playlist_name: Option<String<8>>,
}

#[derive(Clone, Serialize, PartialEq)]
pub enum State {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Serialize)]
pub struct PlaylistWithMetadata {
    pub playlist_name: String<8>,
    pub files: Vec<AudioFileWithMetadata>,
}

#[derive(Clone, Serialize)]
pub struct AudioFileWithMetadata {
    pub file: AudioFile,
    pub metadata: AudioMetadata,
}

impl Status {
    fn new() -> Self {
        let status = Self {
            playback_position: Watch::new(),
            playback_status: Watch::new(),
            current_playlist: Watch::new(),
        };

        status.update_state(State::Stopped);

        status
    }

    pub fn get() -> &'static Self {
        static STATUS: LazyLock<Status> = LazyLock::new(Status::new);
        STATUS.get()
    }

    // Getters
    pub fn get_playback_position(&self) -> u32 {
        self.playback_position.try_get().unwrap()
    }

    pub fn get_playback_status(&self) -> PlaybackStatus {
        self.playback_status.try_get().unwrap()
    }

    pub fn get_current_playlist(&self) -> Option<PlaylistWithMetadata> {
        self.current_playlist.try_get().unwrap()
    }

    pub fn recv_playback_position<'a>(&'a self) -> Receiver<'a, CriticalSectionRawMutex, u32, 1> {
        self.playback_position.receiver().unwrap()
    }

    pub fn recv_playback_status<'a>(
        &'a self,
    ) -> Receiver<'a, CriticalSectionRawMutex, PlaybackStatus, 1> {
        self.playback_status.receiver().unwrap()
    }

    pub fn recv_current_playlist<'a>(
        &'a self,
    ) -> Receiver<'a, CriticalSectionRawMutex, Option<PlaylistWithMetadata>, 1> {
        self.current_playlist.receiver().unwrap()
    }

    // Setters
    pub fn update_state(&self, state: State) {
        match state {
            State::Playing | State::Paused => self.update_playback_status(|status| {
                status.state = state;
            }),
            State::Stopped => {
                self.playback_status.sender().send(PlaybackStatus {
                    state: State::Stopped,
                    metadata: None,
                    index_in_playlist: 0,
                    file_name: None,
                    playlist_name: None,
                });
                self.update_playlist(None);
                self.update_position(0);
            }
        }
    }

    pub fn update_file(&self, index_in_playlist: usize) {
        let playlist = self.get_current_playlist();
        let file = if let Some(ref playlist) = playlist {
            playlist.files.get(index_in_playlist)
        } else {
            None
        };

        if let Some(file) = file {
            self.update_playback_status(|status| {
                status.file_name = Some(file.file.name().try_into().unwrap());
                status.metadata = Some(file.metadata.clone());
                status.index_in_playlist = index_in_playlist;
                status.state = State::Playing;
            });
        } else {
            self.update_playback_status(|status| {
                status.file_name = None;
                status.metadata = None;
                status.index_in_playlist = index_in_playlist;
                status.state = State::Playing;
            });
        }
        self.update_position(0);
    }

    pub fn update_position(&self, position_seconds: u32) {
        self.playback_position.sender().send(position_seconds);
    }

    fn update_playback_status(&self, f: impl FnOnce(&mut PlaybackStatus)) {
        let mut current = self.playback_status.try_get().unwrap();

        f(&mut current);
        self.playback_status.sender().send(current);
    }

    pub fn update_playlist(&self, playlist: Option<PlaylistWithMetadata>) {
        self.current_playlist.sender().send(playlist);
    }
}
