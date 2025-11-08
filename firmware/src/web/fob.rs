use alloc::vec::Vec;
use futures::stream::StreamExt;
use heapless::String;
use picoserve::{
    extract,
    response::{
        chunked::{ChunkWriter, ChunkedResponse, Chunks, ChunksWritten},
        IntoResponse, Json,
    },
};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    entities::{
        audio_file::AudioFile,
        playlist::{list_playlists, Playlist},
    },
    web::{AppState, FileEntry},
    player::FileMetadata,
};

struct StreamingAssociations {
    state: AppState,
}

impl Chunks for StreamingAssociations {
    async fn write_chunks<W: picoserve::io::Write>(
        self,
        mut writer: ChunkWriter<W>,
    ) -> Result<ChunksWritten, W::Error> {
        let mut stream = match list_playlists(self.state.fs).await {
            Ok(s) => s,
            Err(_) => {
                // TODO: how to report error?
                writer.write_chunk(b"[]").await?;
                return writer.finalize().await;
            }
        };

        writer.write_chunk(b"[").await?;
        let mut first = true;
        while let Some(item) = stream.next().await {
            if let Ok((name, files)) = item {
                let mut file_entries = Vec::new();
                for f in &files {
                    let metadata = f.metadata(self.state.fs).await.unwrap_or_default();
                    file_entries.push(FileEntry {
                        name: f.name().try_into().unwrap(),
                        metadata: FileMetadata {
                            artist: metadata.artist,
                            title: metadata.title,
                            album: metadata.album,
                            duration: metadata.duration,
                        },
                    });
                }
                let association = Association {
                    fob: name,
                    files: file_entries,
                };

                if !first {
                    writer.write_chunk(b",").await?;
                }
                first = false;

                let json = serde_json::to_string(&association).map_err(|_| ()).unwrap();
                writer.write_chunk(json.as_bytes()).await?;
            }
        }

        writer.write_chunk(b"]").await?;
        writer.finalize().await
    }

    fn content_type(&self) -> &'static str {
        "application/json"
    }
}

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
    file: String<8>,
}

pub async fn last() -> impl IntoResponse {
    let last_fob = crate::rfid::LAST_FOB.lock().await;
    Json(LastFob {
        last_fob: last_fob.clone(),
    })
}

pub async fn list(extract::State(state): extract::State<AppState>) -> impl IntoResponse {
    ChunkedResponse::new(StreamingAssociations { state })
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
