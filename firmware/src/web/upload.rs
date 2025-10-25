use core::str::FromStr;

use alloc::vec;
use embassy_futures::join::join;
use embedded_io_async::{Read, Write};
use heapless::String;
use picoserve::{
    io::ReadExt,
    request::Request,
    response::{IntoResponse, ResponseWriter},
    routing::RequestHandlerService,
    ResponseSent,
};

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
        let mut buffer1 = vec![0u8; 512];
        let mut buffer2 = vec![0u8; 512];

        let mut read_buffer = &mut buffer1;
        let mut write_buffer = &mut buffer2;
        let mut last_read_size = 0;

        loop {
            // TODO delete file on error
            let (read_res, write_res) = join(
                read_max(&mut body, read_buffer),
                file.write_all(&write_buffer[..last_read_size]),
            )
            .await;
            write_res.unwrap();
            last_read_size = read_res?;

            (write_buffer, read_buffer) = (read_buffer, write_buffer);

            if last_read_size != 512 {
                file.write_all(&write_buffer[..last_read_size])
                    .await
                    .unwrap();
                break;
            }
        }
        file.flush().await.unwrap();

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
