use embedded_io_async::Write;
use heapless::String;
use picoserve::{
    extract,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

use crate::{web::AppState, with_extension};

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
    Json(LastFob { last_fob: None })
}

pub async fn associate(
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<AssociationRequest, 64>,
) -> impl IntoResponse {
    let root = state.fs.root_dir();
    let dir = if !root.dir_exists("fobs").await.unwrap() {
        root.create_dir("fobs").await.unwrap()
    } else {
        root.open_dir("fobs").await.unwrap()
    };

    let fname = with_extension(&req.fob, "m3u").unwrap();
    let mut file = dir.create_file(&fname).await.unwrap();
    file.truncate().await.unwrap();

    file.write_all(b"#EXTM3U\r\n").await.unwrap();

    // TODO copy over artist info
    file.write_all(b"#EXTINF:60,Unknown Artist - Unknown Track\r\n")
        .await
        .unwrap();
    file.write_all(b"..\\files\\").await.unwrap();
    file.write_all(req.file.as_bytes()).await.unwrap();
    file.write_all(b"\r\n").await.unwrap();
    file.flush().await.unwrap();
}
