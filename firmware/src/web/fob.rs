use heapless::String;
use picoserve::{
    extract,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{audio_file::AudioFile, playlist::Playlist},
    web::AppState,
};

#[derive(Serialize)]
struct LastFob {
    last_fob: Option<String<8>>,
}

#[derive(Deserialize)]
pub struct AssociationRequest {
    fob: String<8>,
    file: String<8>,
}

pub async fn last() -> impl IntoResponse {
    let last_fob = crate::rfid::LAST_FOB.lock().await;
    Json(LastFob {
        last_fob: last_fob.clone(),
    })
}

pub async fn associate(
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<AssociationRequest, 64>,
) -> impl IntoResponse {
    let audio_file = AudioFile::new(req.file);
    Playlist::write(state.fs, req.fob, &[audio_file])
        .await
        .unwrap();
}
