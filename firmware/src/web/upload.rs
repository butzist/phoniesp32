use core::str::FromStr;

use alloc::format;
use embedded_io_async::{Read, Write};
use heapless::String;
use picoserve::{
    extract,
    io::ReadExt,
    request::Request,
    response::{IntoResponse, Response, ResponseWriter, StatusCode},
    routing::RequestHandlerService,
    ResponseSent,
};
use serde::Deserialize;

use crate::{web::AppState, with_extension};

pub struct UploadFileName(String<8>);

impl FromStr for UploadFileName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        String::try_from(s).map(Self)
    }
}

pub struct UploadService;

impl RequestHandlerService<AppState, (UploadFileName,)> for UploadService {
    async fn call_request_handler_service<
        R: embedded_io_async::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (UploadFileName,),
        mut request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let root = state.fs.root_dir();
        let dir = if !root.dir_exists("files").await.unwrap() {
            root.create_dir("files").await.unwrap()
        } else {
            root.open_dir("files").await.unwrap()
        };

        let fname = with_extension(&path_parameters.0 .0, "wav").unwrap();
        let mut file = dir.create_file(&fname).await.unwrap();
        file.truncate().await.unwrap();

        let mut body = request.body_connection.body().reader();
        let mut buffer = [0u8; 512];
        loop {
            match body.read(&mut buffer).await {
                Ok(0) => {
                    // eof
                    break;
                }
                Ok(n) => {
                    file.write_all(&buffer[..n]).await.unwrap();
                }
                Err(err) => {
                    // stream terminated early?
                    // delete file
                    return Err(err);
                }
            }
        }

        body.discard_all_data().await?;
        let connection = request.body_connection.finalize().await?;
        "".write_to(connection, response_writer).await
    }
}

#[derive(Deserialize)]
pub struct FileInfo {
    duration_seconds: u16,
    description: String<50>,
}

pub async fn put_info(
    name: UploadFileName,
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<FileInfo, 64>,
) -> impl IntoResponse {
    let root = state.fs.root_dir();
    let dir = if !root.dir_exists("files").await.unwrap() {
        root.create_dir("files").await.unwrap()
    } else {
        root.open_dir("files").await.unwrap()
    };

    let fname = with_extension(&name.0, "wav").unwrap();
    if !dir.file_exists(&fname).await.unwrap() {
        return Response::new(StatusCode::NOT_FOUND, "file not found");
    }

    let fname = with_extension(&name.0, "inf").unwrap();
    let mut file = dir.create_file(&fname).await.unwrap();
    file.truncate().await.unwrap();

    let line = format!("{},{}\r\n", req.duration_seconds, req.description);
    file.write_all(line.as_bytes()).await.unwrap();
    file.close().await.unwrap();

    state.fs.flush().await.unwrap();

    Response::new(StatusCode::NO_CONTENT, "")
}
