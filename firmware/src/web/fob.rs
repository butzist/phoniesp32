use alloc::vec::Vec;
use futures::stream::StreamExt;
use heapless::String;
use picoserve::{
    extract::{self, FromRequestParts},
    request::Request,
    response::{
        chunked::{ChunkWriter, ChunkedResponse, Chunks, ChunksWritten},
        IntoResponse, Json, Response, ResponseWriter, StatusCode,
    },
    routing::RequestHandlerService,
    ResponseSent,
};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    entities::{
        audio_file::{AudioFile, AudioMetadata},
        playlist::{PlayListRef, Playlist},
    },
    sd::SdFileSystem,
    web::{AppState, FileEntry},
};

#[derive(Serialize)]
pub struct Association {
    fob: String<8>,
    files: Vec<FileEntry>,
}

#[derive(Serialize)]
pub struct LastFob {
    last_fob: Option<String<8>>,
}

#[derive(Deserialize)]
pub struct AssociationRequest {
    fob: String<8>,
    files: Vec<String<8>>,
}

pub async fn last() -> impl IntoResponse {
    let last_fob = crate::rfid::LAST_FOB.lock().await;
    Json(LastFob {
        last_fob: last_fob.clone(),
    })
}

pub async fn associate(
    extract::State(state): extract::State<AppState>,
    extract::Json(req): extract::Json<AssociationRequest, 256>,
) -> impl IntoResponse {
    let audio_files: Vec<AudioFile> = req.files.into_iter().map(AudioFile::new).collect();
    Playlist::write(state.fs, req.fob, &audio_files)
        .await
        .unwrap();
}

#[derive(Deserialize, Default)]
struct ListQuery {
    fob: Option<String<8>>,
}

pub struct ListAssociationsService;

impl RequestHandlerService<AppState> for ListAssociationsService {
    async fn call_request_handler_service<
        R: embedded_io_async::Read,
        W: ResponseWriter<Error = R::Error>,
    >(
        &self,
        state: &AppState,
        _path_parameters: (),
        request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let query = extract::Query::<ListQuery>::from_request_parts(state, &request.parts)
            .await
            .ok();

        let connection = request.body_connection.finalize().await?;

        if let Some(extract::Query(ListQuery { fob: Some(fob) })) = query {
            let playlist = PlayListRef::new(fob.clone()).read(state.fs).await.ok();
            if let Some(playlist) = playlist {
                let association = playlist_to_association(playlist, state.fs).await;

                Json(association)
                    .write_to(connection, response_writer)
                    .await
            } else {
                Response::new(StatusCode::NOT_FOUND, "")
                    .write_to(connection, response_writer)
                    .await
            }
        } else {
            ChunkedResponse::new(StreamingAssociations {
                state: state.clone(),
            })
            .write_to(connection, response_writer)
            .await
        }
    }
}

struct StreamingAssociations {
    state: AppState,
}

impl Chunks for StreamingAssociations {
    async fn write_chunks<W: picoserve::io::Write>(
        self,
        mut writer: ChunkWriter<W>,
    ) -> Result<ChunksWritten, W::Error> {
        let mut stream = match Playlist::list(self.state.fs).await {
            Ok(s) => s,
            Err(_) => {
                // TODO: how to report error?
                writer.write_chunk(b"[]").await?;
                return writer.finalize().await;
            }
        };

        writer.write_chunk(b"[").await?;
        let mut first = true;
        while let Some(playlist) = stream.next().await {
            let association = playlist_to_association(playlist, self.state.fs).await;

            if !first {
                writer.write_chunk(b",").await?;
            }
            first = false;

            let json = serde_json::to_string(&association).map_err(|_| ()).unwrap();
            writer.write_chunk(json.as_bytes()).await?;
        }

        writer.write_chunk(b"]").await?;
        writer.finalize().await
    }

    fn content_type(&self) -> &'static str {
        "application/json"
    }
}

async fn playlist_to_association(playlist: Playlist, fs: &SdFileSystem<'_>) -> Association {
    let name = playlist.name;
    let files = playlist.files;
    let mut file_entries = Vec::new();
    for f in &files {
        let metadata = f.metadata(fs).await.unwrap_or_default();
        file_entries.push(FileEntry {
            name: f.name().try_into().unwrap(),
            metadata: AudioMetadata {
                artist: metadata.artist,
                title: metadata.title,
                album: metadata.album,
                duration: metadata.duration,
            },
        });
    }

    Association {
        fob: name,
        files: file_entries,
    }
}
