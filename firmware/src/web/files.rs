use futures::stream::StreamExt;
use picoserve::{
    extract,
    response::{
        chunked::{ChunkWriter, ChunkedResponse, Chunks, ChunksWritten},
        IntoResponse,
    },
};
use serde_json;

use crate::{
    entities::audio_file::list_files,
    web::{AppState, FileEntry, FileMetadata},
};

struct StreamingFiles {
    state: AppState,
}

impl Chunks for StreamingFiles {
    async fn write_chunks<W: picoserve::io::Write>(
        self,
        mut writer: ChunkWriter<W>,
    ) -> Result<ChunksWritten, W::Error> {
        let mut stream = match list_files(self.state.fs).await {
            Ok(s) => s,
            Err(_) => {
                writer.write_chunk(b"[]").await?;
                return writer.finalize().await;
            }
        };

        writer.write_chunk(b"[").await?;
        let mut first = true;
        while let Some(item) = stream.next().await {
            if let Ok((name, metadata)) = item {
                let file_metadata = FileMetadata {
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
