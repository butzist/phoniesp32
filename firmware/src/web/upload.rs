use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use defmt::error;
use embassy_futures::join::join;
use embedded_io_async::Write;
use picoserve::io::Read;
use picoserve::{
    ResponseSent,
    io::ReadExt,
    request::Request,
    response::{IntoResponse, ResponseWriter, StatusCode},
    routing::RequestHandlerService,
};

use crate::{
    entities::audio_file::AudioFile,
    web::{AppState, files::AudioFileName},
};

const BUFFER_SIZE: usize = 4096;

pub struct CreateFileService;
pub struct UploadService;
pub struct PatchUploadService;
pub struct HeadFileService;

impl RequestHandlerService<AppState, (AudioFileName,)> for CreateFileService {
    async fn call_request_handler_service<
        R: picoserve::io::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (AudioFileName,),
        request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let name = path_parameters.0.0;
        let audio_file = AudioFile::new(name.clone());

        // Create empty file
        let fs_guard = state.fs.borrow_mut().await;
        audio_file.create(&fs_guard).await.unwrap();

        let connection = request.body_connection.finalize().await?;
        let location = format!("/api/files/{}", name);
        picoserve::response::Response::new(picoserve::response::StatusCode::CREATED, "")
            .with_header("Location", location)
            .write_to(connection, response_writer)
            .await
    }
}

impl RequestHandlerService<AppState, (AudioFileName,)> for UploadService {
    async fn call_request_handler_service<
        R: picoserve::io::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (AudioFileName,),
        mut request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let name = path_parameters.0.0;
        let audio_file = AudioFile::new(name);

        // Create empty file first
        let fs_guard = state.fs.borrow_mut().await;
        let mut file_handle = match audio_file.create(&fs_guard).await {
            Ok(handle) => handle,
            Err(_) => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(StatusCode::INTERNAL_SERVER_ERROR, "")
                    .write_to(connection, response_writer)
                    .await;
            }
        };

        let mut body = request.body_connection.body().reader();
        let mut buffer1 = vec![0u8; BUFFER_SIZE];
        let mut buffer2 = vec![0u8; BUFFER_SIZE];

        let mut read_buffer = &mut buffer1;
        let mut write_buffer = &mut buffer2;
        let mut last_read_size = 0;
        let mut total_read_size = 0;

        loop {
            // TODO delete file on error
            let (read_res, write_res) = join(
                read_max(&mut body, read_buffer),
                write_all(&mut file_handle, write_buffer, last_read_size),
            )
            .await;
            write_res.unwrap();
            last_read_size = read_res?;
            total_read_size += last_read_size;
            error!("total {} bytes read", total_read_size);

            (write_buffer, read_buffer) = (read_buffer, write_buffer);

            if last_read_size != BUFFER_SIZE {
                file_handle
                    .write_all(&write_buffer[..last_read_size])
                    .await
                    .unwrap();
                break;
            }
        }
        file_handle.flush().await.unwrap();

        body.discard_all_data().await?;
        let connection = request.body_connection.finalize().await?;
        picoserve::response::Response::new(StatusCode::NO_CONTENT, "")
            .write_to(connection, response_writer)
            .await
    }
}

async fn write_all<W: Write>(w: &mut W, buf: &[u8], size: usize) -> Result<(), W::Error>
where
    W::Error: defmt::Format,
{
    error!("writing {} bytes", size);
    match w.write_all(&buf[..size]).await {
        Ok(_) => {
            error!("write OK");
            Ok(())
        }
        Err(err) => {
            error!("write Error: {:?}", err);
            Err(err)
        }
    }
}

impl RequestHandlerService<AppState, (AudioFileName,)> for PatchUploadService {
    async fn call_request_handler_service<
        R: picoserve::io::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (AudioFileName,),
        mut request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let name = path_parameters.0.0;
        let audio_file = AudioFile::new(name);

        // Extract Upload-Offset header
        let upload_offset = match request.parts.headers().get("Upload-Offset") {
            Some(offset_str) => {
                // Try to convert HeaderValue to string using different approaches
                let offset_str = match offset_str.as_str() {
                    Ok(s) => s,
                    Err(_) => {
                        let connection = request.body_connection.finalize().await?;
                        return picoserve::response::Response::new(
                            StatusCode::BAD_REQUEST,
                            "Invalid Upload-Offset header",
                        )
                        .write_to(connection, response_writer)
                        .await;
                    }
                };

                match offset_str.parse::<u64>() {
                    Ok(offset) => offset,
                    Err(_) => {
                        let connection = request.body_connection.finalize().await?;
                        return picoserve::response::Response::new(
                            StatusCode::BAD_REQUEST,
                            "Invalid Upload-Offset header",
                        )
                        .write_to(connection, response_writer)
                        .await;
                    }
                }
            }
            None => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(
                    StatusCode::BAD_REQUEST,
                    "Upload-Offset header required",
                )
                .write_to(connection, response_writer)
                .await;
            }
        };

        // Check if file exists
        let fs_guard = state.fs.borrow_mut().await;
        match audio_file.exists(&fs_guard).await {
            Ok(true) => {}
            Ok(false) => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(StatusCode::NOT_FOUND, "")
                    .write_to(connection, response_writer)
                    .await;
            }
            Err(_) => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(StatusCode::INTERNAL_SERVER_ERROR, "")
                    .write_to(connection, response_writer)
                    .await;
            }
        }

        // Check if offset is valid (not beyond file size)
        match audio_file.size(&fs_guard).await {
            Ok(file_size) => {
                if upload_offset > file_size {
                    let connection = request.body_connection.finalize().await?;
                    return picoserve::response::Response::new(
                        StatusCode::BAD_REQUEST,
                        "Upload-Offset exceeds file size",
                    )
                    .write_to(connection, response_writer)
                    .await;
                }
            }
            Err(_) => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(StatusCode::INTERNAL_SERVER_ERROR, "")
                    .write_to(connection, response_writer)
                    .await;
            }
        }

        // Open file at specified offset
        let mut file_handle = match audio_file.append_at(&fs_guard, upload_offset).await {
            Ok(handle) => handle,
            Err(_) => {
                let connection = request.body_connection.finalize().await?;
                return picoserve::response::Response::new(StatusCode::INTERNAL_SERVER_ERROR, "")
                    .write_to(connection, response_writer)
                    .await;
            }
        };

        let mut body = request.body_connection.body().reader();
        let mut buffer1 = vec![0u8; BUFFER_SIZE];
        let mut buffer2 = vec![0u8; BUFFER_SIZE];

        let mut read_buffer = &mut buffer1;
        let mut write_buffer = &mut buffer2;
        let mut last_read_size = 0;
        let mut total_read_size = 0;

        loop {
            let (read_res, write_res) = join(
                read_max(&mut body, read_buffer),
                write_all(&mut file_handle, write_buffer, last_read_size),
            )
            .await;
            write_res.unwrap();
            last_read_size = read_res?;
            total_read_size += last_read_size;
            error!("total {} bytes read", total_read_size);

            (write_buffer, read_buffer) = (read_buffer, write_buffer);

            if last_read_size != BUFFER_SIZE {
                file_handle
                    .write_all(&write_buffer[..last_read_size])
                    .await
                    .unwrap();
                break;
            }
        }
        file_handle.flush().await.unwrap();

        body.discard_all_data().await?;
        let connection = request.body_connection.finalize().await?;
        picoserve::response::Response::new(StatusCode::NO_CONTENT, "")
            .write_to(connection, response_writer)
            .await
    }
}

impl RequestHandlerService<AppState, (AudioFileName,)> for HeadFileService {
    async fn call_request_handler_service<
        R: picoserve::io::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (AudioFileName,),
        request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let name = path_parameters.0.0;
        let audio_file = AudioFile::new(name);

        let connection = request.body_connection.finalize().await?;

        let fs_guard = state.fs.borrow_mut().await;
        match audio_file.size(&fs_guard).await {
            Ok(file_size) => {
                picoserve::response::Response::new(StatusCode::OK, "")
                    .with_header("Upload-Offset", file_size.to_string())
                    .write_to(connection, response_writer)
                    .await
            }
            Err(_) => {
                picoserve::response::Response::new(StatusCode::NOT_FOUND, "")
                    .write_to(connection, response_writer)
                    .await
            }
        }
    }
}

async fn read_max<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, R::Error> {
    error!("reading {} bytes", buf.len());
    let mut buffer_pos = 0;
    while buffer_pos < buf.len() {
        match r.read(&mut buf[buffer_pos..]).await {
            Ok(0) => {
                error!("read EOF");
                break;
            }
            Ok(n) => {
                error!("read {} bytes", n);
                buffer_pos += n;
            }
            Err(e) => {
                error!("read Error");
                return Err(e);
            }
        }
    }
    error!("read {} bytes total", buffer_pos);
    Ok(buffer_pos)
}
