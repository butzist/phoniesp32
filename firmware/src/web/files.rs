use core::str::FromStr;

use futures::stream::StreamExt;
use heapless::String;
use picoserve::{
    extract,
    request::Request,
    response::{
        chunked::{ChunkWriter, ChunkedResponse, Chunks, ChunksWritten},
        IntoResponse, Json, Response, ResponseWriter, StatusCode,
    },
    routing::RequestHandlerService,
    ResponseSent,
};
use serde_json;

use crate::{
    entities::audio_file::AudioFile,
    web::{AppState, AudioMetadata, FileEntry},
};

pub struct AudioFileName(pub String<8>);

impl FromStr for AudioFileName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        String::try_from(s).map(Self)
    }
}

struct StreamingFiles {
    state: AppState,
}

impl Chunks for StreamingFiles {
    async fn write_chunks<W: picoserve::io::Write>(
        self,
        mut writer: ChunkWriter<W>,
    ) -> Result<ChunksWritten, W::Error> {
        let mut stream = match AudioFile::list(self.state.fs).await {
            Ok(s) => s,
            Err(_) => {
                writer.write_chunk(b"[]").await?;
                return writer.finalize().await;
            }
        };

        writer.write_chunk(b"[").await?;
        let mut first = true;
        while let Some((name, metadata)) = stream.next().await {
            let file_metadata = AudioMetadata {
                artist: metadata.artist,
                title: metadata.title,
                album: metadata.album,
                duration: metadata.duration,
            };
            let file_entry = FileEntry {
                name,
                metadata: file_metadata,
            };

            if !first {
                writer.write_chunk(b",").await?;
            }
            first = false;

            let json = serde_json::to_string(&file_entry).map_err(|_| ()).unwrap();
            writer.write_chunk(json.as_bytes()).await?;
        }

        writer.write_chunk(b"]").await?;
        writer.finalize().await
    }

    fn content_type(&self) -> &'static str {
        "application/json"
    }
}

pub async fn list(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    ChunkedResponse::new(StreamingFiles { state })
}

pub struct GetMetadataService;

impl RequestHandlerService<AppState, (AudioFileName,)> for GetMetadataService {
    async fn call_request_handler_service<
        R: embedded_io_async::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        path_parameters: (AudioFileName,),
        request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let name = path_parameters.0 .0;
        let connection = request.body_connection.finalize().await?;

        let audio_file = AudioFile::new(name);
        match audio_file.metadata(state.fs).await {
            Ok(metadata) => {
                let file_metadata = AudioMetadata {
                    artist: metadata.artist,
                    title: metadata.title,
                    album: metadata.album,
                    duration: metadata.duration,
                };

                Json(file_metadata)
                    .write_to(connection, response_writer)
                    .await
            }
            Err(_) => {
                Response::new(StatusCode::NOT_FOUND, "")
                    .write_to(connection, response_writer)
                    .await
            }
        }
    }
}
