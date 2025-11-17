use embedded_io_async::Write;
use picoserve::{
    extract,
    response::{IntoResponse, Response, StatusCode},
};

use crate::{DeviceConfig, web::AppState};

pub async fn put(
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<DeviceConfig>,
) -> impl IntoResponse {
    let root = state.fs.root_dir();

    let mut file = root.create_file("config.jsn").await.unwrap();
    file.truncate().await.unwrap();

    let buffer = serde_json::to_vec(&req).unwrap();
    file.write_all(&buffer).await.unwrap();
    file.close().await.unwrap();

    state.fs.flush().await.unwrap();

    Response::new(StatusCode::NO_CONTENT, "")
}

pub async fn delete(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    let root = state.fs.root_dir();

    match root.remove("config.jsn").await {
        Ok(_) => {}
        Err(embedded_fatfs::Error::NotFound) => {
            return Response::new(StatusCode::NOT_FOUND, "not found");
        }
        Err(_) => panic!("FIXME filesystem error"),
    }

    state.fs.flush().await.unwrap();

    Response::new(StatusCode::NO_CONTENT, "")
}
