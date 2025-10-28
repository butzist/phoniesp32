use alloc::{vec, vec::Vec};
use heapless::String;
use picoserve::{
    extract,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;

use crate::{
    entities::{
        audio_file::AudioFile,
        playlist::{PlayListRef, Playlist},
    },
    player::{PlayerCommand, State, Status},
    web::AppState,
};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayRequest {
    File(String<8>),
    Playlist(Vec<String<8>>),
    PlaylistRef(String<8>),
}

pub async fn status(extract::State(_state): extract::State<AppState>) -> impl IntoResponse {
    Json(Status {
        state: State::Stopped,
        position_seconds: None,
        duration_seconds: None,
        description: None,
    })
}

pub async fn play(
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<PlayRequest, 16>,
) -> impl IntoResponse {
    match req {
        PlayRequest::File(file) => {
            let audio_file = AudioFile::new(file);
            let playlist = Playlist::new("WEB_API".try_into().unwrap(), vec![audio_file]);
            state.commands.send(PlayerCommand::Playlist(playlist)).await;
        }
        PlayRequest::Playlist(files) => {
            let audio_files = files.into_iter().map(AudioFile::new).collect::<Vec<_>>();
            let playlist = Playlist::new("WEB_API".try_into().unwrap(), audio_files);
            state.commands.send(PlayerCommand::Playlist(playlist)).await;
        }
        PlayRequest::PlaylistRef(playlist_ref) => {
            let playlist_ref = PlayListRef::new(playlist_ref);
            state
                .commands
                .send(PlayerCommand::PlaylistRef(playlist_ref))
                .await;
        }
    }
    Response::ok("")
}

pub async fn stop(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::Stop).await;
    Response::ok("")
}

pub async fn pause(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::Pause).await;
    Response::ok("")
}

pub async fn volume_up(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::VolumeUp).await;
    Response::ok("")
}

pub async fn volume_down(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::VolumeDown).await;
    Response::ok("")
}
