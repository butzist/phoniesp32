use alloc::{vec, vec::Vec};
use heapless::String;
use picoserve::{
    extract,
    response::{IntoResponse, Json, Response, StatusCode},
};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{
        audio_file::AudioFile,
        playlist::{PlayListRef, Playlist},
    },
    player::control::Skip,
    player::{
        status::{PlaylistWithMetadata, State, Status},
        PlayerCommand,
    },
    web::AppState,
};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayRequest {
    File(String<8>),
    Playlist(Vec<String<8>>),
    PlaylistRef(String<8>),
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub position_seconds: u32,
    pub state: State,
    pub index_in_playlist: usize,
    pub playlist_name: Option<String<8>>,
}

#[derive(Serialize)]
pub struct CurrentPlaylistResponse(PlaylistWithMetadata);

pub async fn status() -> impl IntoResponse {
    let player_status = Status::get();
    let position = player_status.get_playback_position();
    let current_file = player_status.get_playback_status();
    let playlist = player_status.get_current_playlist();

    let state = current_file.state;
    let index_in_playlist = current_file.index_in_playlist;
    let playlist_name = playlist.map(|p| p.playlist_name);

    Json(StatusResponse {
        position_seconds: position,
        state,
        index_in_playlist,
        playlist_name,
    })
}

pub async fn current_playlist() -> impl IntoResponse {
    let player_status = Status::get();
    let current_playlist = player_status.get_current_playlist();

    Json(current_playlist.map(CurrentPlaylistResponse))
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
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn stop(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::Stop).await;
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn pause(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::Pause).await;
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn volume_up(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::VolumeUp).await;
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn volume_down(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::VolumeDown).await;
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn next(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state.commands.send(PlayerCommand::Skip(Skip::Next)).await;
    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn previous(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    state
        .commands
        .send(PlayerCommand::Skip(Skip::Previous))
        .await;
    Response::new(StatusCode::NO_CONTENT, "")
}
