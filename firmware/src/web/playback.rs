use heapless::String;
use picoserve::{
    extract,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;

use crate::{
    player::{PlayerCommand, State, Status},
    web::AppState,
};

#[derive(Deserialize)]
pub struct PlayRequest {
    file: String<8>,
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
    state.commands.send(PlayerCommand::Play(req.file)).await;
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
