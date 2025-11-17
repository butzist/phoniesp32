use alloc::vec;
use embassy_futures::join::join;
use embedded_io_async::Write as _;
use picoserve::io::Read;
use picoserve::{
    ResponseSent,
    io::ReadExt,
    request::Request,
    response::{IntoResponse, ResponseWriter},
    routing::RequestHandlerService,
};

use crate::{
    entities::audio_file::AudioFile,
    web::{AppState, files::AudioFileName},
};

pub struct UploadService;

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
        let mut file_handle = audio_file.create(state.fs).await.unwrap();

        let mut body = request.body_connection.body().reader();
        let mut buffer1 = vec![0u8; 512];
        let mut buffer2 = vec![0u8; 512];

        let mut read_buffer = &mut buffer1;
        let mut write_buffer = &mut buffer2;
        let mut last_read_size = 0;

        loop {
            // TODO delete file on error
            let (read_res, write_res) = join(
                read_max(&mut body, read_buffer),
                file_handle.write_all(&write_buffer[..last_read_size]),
            )
            .await;
            write_res.unwrap();
            last_read_size = read_res?;

            (write_buffer, read_buffer) = (read_buffer, write_buffer);

            if last_read_size != 512 {
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
        "".write_to(connection, response_writer).await
    }
}

async fn read_max<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, R::Error> {
    let mut buffer_pos = 0;
    while !buf.is_empty() {
        match r.read(&mut buf[buffer_pos..]).await {
            Ok(0) => break,
            Ok(n) => buffer_pos += n,
            Err(e) => return Err(e),
        }
    }
    Ok(buffer_pos)
}
