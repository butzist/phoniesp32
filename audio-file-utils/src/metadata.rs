use core::iter::repeat_n;

use embedded_io_async::{ErrorType, Read, ReadExactError, Write};
use heapless::{String, Vec};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub artist: String<31>,
    pub title: String<31>,
    pub album: String<31>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MetadataAndDuration {
    pub metadata: Metadata,
    pub duration: usize,
}

#[derive(Error, Debug)]
#[error("Error while extracting metadata")]
pub enum Error<E> {
    Read(ReadExactError<E>),
    Write(E),
    InvalidFileType,
}

impl Default for Metadata {
    fn default() -> Self {
        let default: String<31> = "Unknown".try_into().unwrap();
        Self {
            artist: default.clone(),
            title: default.clone(),
            album: default,
        }
    }
}

pub const INFO_CHUNK_SIZE: usize = 4 + 40 * 3;

pub async fn extract_metadata<R>(mut reader: R) -> Result<Metadata, Error<<R as ErrorType>::Error>>
where
    R: Read,
{
    let mut buf = [0u8; 12];
    reader.read_exact(&mut buf).await.map_err(Error::Read)?;
    if &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        return Err(Error::InvalidFileType);
    }

    let mut artist = None;
    let mut title = None;
    let mut album = None;

    loop {
        let mut chunk_header = [0u8; 8];
        // check for end of file
        if reader.read_exact(&mut chunk_header).await.is_err() {
            break;
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes(chunk_header[4..8].try_into().unwrap()) as usize;

        if chunk_id == b"LIST" {
            let mut list_type = [0u8; 4];
            reader
                .read_exact(&mut list_type)
                .await
                .map_err(Error::Read)?;
            let mut remaining = chunk_size - 4;
            if &list_type == b"INFO" {
                while remaining >= 8 {
                    let mut sub_header = [0u8; 8];
                    reader
                        .read_exact(&mut sub_header)
                        .await
                        .map_err(Error::Read)?;
                    let sub_id = &sub_header[0..4];
                    let sub_size =
                        u32::from_le_bytes(sub_header[4..8].try_into().unwrap()) as usize;
                    let text_size = sub_size.min(31);
                    let mut data_vec: Vec<u8, 31> = Vec::from_iter(repeat_n(0, 31));
                    reader
                        .read_exact(&mut data_vec[..text_size])
                        .await
                        .map_err(Error::Read)?;
                    while data_vec.last() == Some(&0) {
                        data_vec.pop();
                    }
                    let text_str = core::str::from_utf8(&data_vec).unwrap_or("Unknown");
                    let text: String<31> =
                        text_str.try_into().unwrap_or("Unknown".try_into().unwrap());
                    match sub_id {
                        b"IART" => artist = Some(text),
                        b"INAM" => title = Some(text),
                        b"IPRD" => album = Some(text),
                        _ => {}
                    }
                    if text_size < sub_size {
                        let remaining_sub = sub_size - text_size;
                        skip(&mut reader, remaining_sub).await?;
                    }
                    remaining -= 8 + sub_size;
                }
            }

            return Ok(Metadata {
                artist: artist.unwrap_or("Unknown".try_into().unwrap()),
                title: title.unwrap_or("Unknown".try_into().unwrap()),
                album: album.unwrap_or("Unknown".try_into().unwrap()),
            });
        } else {
            // Skip chunk
            skip(&mut reader, chunk_size).await?;
        }
    }

    Ok(Metadata {
        artist: "Unknown".try_into().unwrap(),
        title: "Unknown".try_into().unwrap(),
        album: "Unknown".try_into().unwrap(),
    })
}

async fn skip<R>(reader: &mut R, mut size: usize) -> Result<(), Error<<R as ErrorType>::Error>>
where
    R: Read,
{
    let mut buf = [0u8; 16];
    while size > 0 {
        let to_read = size.min(16);
        reader
            .read_exact(&mut buf[..to_read])
            .await
            .map_err(Error::Read)?;
        size -= to_read;
    }

    Ok(())
}

pub async fn write_info_chunk<W>(
    mut writer: W,
    metadata: &Metadata,
) -> Result<(), Error<<W as ErrorType>::Error>>
where
    W: Write,
{
    writer.write_all(b"INFO").await.map_err(Error::Write)?;

    // Artist
    let artist_data = metadata.artist.as_bytes();
    let artist_len = artist_data.len().min(31);
    writer.write_all(b"IART").await.map_err(Error::Write)?;
    writer
        .write_all(&((artist_len + 1) as u32).to_le_bytes())
        .await
        .map_err(Error::Write)?;
    writer
        .write_all(&artist_data[..artist_len])
        .await
        .map_err(Error::Write)?;
    writer.write_all(&[0]).await.map_err(Error::Write)?;

    // Title
    let title_data = metadata.title.as_bytes();
    let title_len = title_data.len().min(31);
    writer.write_all(b"INAM").await.map_err(Error::Write)?;
    writer
        .write_all(&((title_len + 1) as u32).to_le_bytes())
        .await
        .map_err(Error::Write)?;
    writer
        .write_all(&title_data[..title_len])
        .await
        .map_err(Error::Write)?;
    writer.write_all(&[0]).await.map_err(Error::Write)?;

    // Album
    let album_data = metadata.album.as_bytes();
    let album_len = album_data.len().min(31);
    writer.write_all(b"IPRD").await.map_err(Error::Write)?;
    writer
        .write_all(&((album_len + 1) as u32).to_le_bytes())
        .await
        .map_err(Error::Write)?;
    writer
        .write_all(&album_data[..album_len])
        .await
        .map_err(Error::Write)?;
    writer.write_all(&[0]).await.map_err(Error::Write)?;

    // Pad the LIST data with zeros
    let written = 4 + (8 + artist_len + 1) + (8 + title_len + 1) + (8 + album_len + 1);
    let padding = INFO_CHUNK_SIZE - written;
    for _ in 0..padding {
        writer.write_all(&[0]).await.map_err(Error::Write)?;
    }

    Ok(())
}
