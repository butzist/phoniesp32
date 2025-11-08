use anyhow::{bail, Context};
use audio_file_utils::metadata::{
    extract_metadata as extract_audio_metadata, write_info_chunk, Metadata as AudioMetadata,
};
use std::io::{Cursor, Read, Seek, SeekFrom};

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub artist: heapless::String<31>,
    pub title: heapless::String<31>,
    pub album: heapless::String<31>,
}

pub async fn extract_metadata(data: &[u8]) -> Metadata {
    let audio_metadata = extract_audio_metadata(data).await.unwrap_or_default();
    Metadata {
        artist: audio_metadata.artist,
        title: audio_metadata.title,
        album: audio_metadata.album,
    }
}

pub async fn update_metadata(data: &mut [u8], metadata: &Metadata) -> anyhow::Result<()> {
    let mut cursor = Cursor::new(data);

    // Seek to the LIST section 40 bytes from start
    cursor
        .seek(SeekFrom::Start(40))
        .context("Seek to LIST chunk")?;

    // Read and validate "LIST" tag
    let mut list_tag = [0u8; 4];
    cursor.read_exact(&mut list_tag).context("Read LIST tag")?;
    if &list_tag != b"LIST" {
        bail!("LIST tag not found");
    }

    // Read and validate length = 124
    let mut length_bytes = [0u8; 4];
    cursor
        .read_exact(&mut length_bytes)
        .context("Reading LIST chunk length")?;
    let length = u32::from_le_bytes(length_bytes);
    if length != 124 {
        bail!("Unexpected LIST chunk length")
    }

    // Get the position where INFO chunk should be written (after LIST header)
    let info_start = cursor.position();
    let info_end = info_start + 124; // INFO chunk size
    let info_buffer = &mut cursor.get_mut()[info_start as usize..info_end as usize];

    // Convert our Metadata to AudioMetadata
    let audio_metadata = AudioMetadata {
        artist: metadata.artist.clone(),
        title: metadata.title.clone(),
        album: metadata.album.clone(),
    };

    // Call write_info_chunk with the metadata - use the sub-buffer directly
    write_info_chunk(info_buffer, &audio_metadata)
        .await
        .context("writing INFO chunk")?;

    Ok(())
}
