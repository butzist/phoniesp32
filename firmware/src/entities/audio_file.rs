use core::pin::Pin;

use alloc::boxed::Box;
use defmt::warn;
use heapless::String;
use serde::Serialize;

use crate::drivers::sd::{FileHandle, SdFileSystem};
use crate::entities::basename;
use crate::{PrintErr, with_extension};
use audio_codec_algorithms::{AdpcmImaState, decode_adpcm_ima};
use audio_file_utils::metadata::{INFO_CHUNK_SIZE, extract_metadata};
use embedded_io_async::{Read, Seek, SeekFrom};
use futures::stream::{self, Stream, StreamExt};

const FILE_DIR: &str = "FILES";
const FILE_EXT: &str = ".WAV";

#[derive(Clone, Serialize)]
pub struct AudioMetadata {
    pub artist: heapless::String<31>,
    pub title: heapless::String<31>,
    pub album: heapless::String<31>,
    pub duration: u32,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        let default: String<31> = "Unknown".try_into().unwrap();
        Self {
            artist: default.clone(),
            title: default.clone(),
            album: default,
            duration: 60,
        }
    }
}

#[derive(Clone, serde::Serialize, defmt::Format)]
pub struct AudioFile(String<8>);

impl AudioFile {
    pub fn new(name: String<8>) -> Self {
        Self(name)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub async fn open<'a>(&self, fs: &'a SdFileSystem) -> Result<FileHandle<'a>, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: Opening files directory")
            .ok_or(())?;

        dir.open_file(&fname)
            .await
            .print_err("AudioFile: Opening file")
            .ok_or(())
    }

    pub async fn create<'a>(&self, fs: &'a SdFileSystem) -> Result<FileHandle<'a>, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = if !root.dir_exists(FILE_DIR).await.unwrap_or(false) {
            root.create_dir(FILE_DIR).await.unwrap()
        } else {
            root.open_dir(FILE_DIR).await.unwrap()
        };

        let mut file = dir.create_file(&fname).await.unwrap();
        file.truncate().await.unwrap();

        Ok(file)
    }

    pub async fn exists(&self, fs: &SdFileSystem) -> Result<bool, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: Opening files directory")
            .ok_or(())?;

        let meta = dir
            .open_meta(&fname)
            .await
            .print_err("AudioFile: Checking file existence")
            .ok_or(())?;
        Ok(meta.is_file())
    }

    pub async fn size(&self, fs: &SdFileSystem) -> Result<u64, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: Opening files directory")
            .ok_or(())?;

        let meta = dir
            .open_meta(&fname)
            .await
            .print_err("AudioFile: Getting file metadata")
            .ok_or(())?;
        Ok(meta.len())
    }

    pub async fn append_at<'a>(
        &self,
        fs: &'a SdFileSystem,
        offset: u64,
    ) -> Result<FileHandle<'a>, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: Opening files directory")
            .ok_or(())?;

        let mut file = dir
            .open_file(&fname)
            .await
            .print_err("AudioFile: Opening file for append at offset")
            .ok_or(())?;

        if let Err(e) = file.seek(SeekFrom::Start(offset)).await {
            defmt::error!("AudioFile: seeking to offset: {:?}", e);
            return Err(());
        }

        Ok(file)
    }

    pub fn from_path(path: &str) -> Option<Self> {
        if path.starts_with("..\\FILES\\") && path.ends_with(FILE_EXT) {
            let start = "..\\FILES\\".len();
            let end = path.len() - FILE_EXT.len();

            Some(Self(path[start..end].parse::<String<8>>().ok()?))
        } else {
            None
        }
    }

    pub async fn data_reader<'a>(
        &'a self,
        fs: &'a SdFileSystem,
    ) -> Result<impl embedded_io_async::Read<Error = impl defmt::Format> + use<'a>, ()> {
        let mut file = self.open(fs).await?;

        let list_chunk_size = 8 + INFO_CHUNK_SIZE as u64;
        let header_size = 48 + list_chunk_size;

        file.seek(SeekFrom::Start(header_size)).await.unwrap();
        Ok(file)
    }

    pub async fn metadata(&self, fs: &SdFileSystem) -> Result<AudioMetadata, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: Opening files directory")
            .ok_or(())?;

        let meta = dir
            .open_meta(&fname)
            .await
            .print_err("AudioFile: Checking file")
            .ok_or(())?;
        let file_size = meta.len();

        let mut file = dir
            .open_file(&fname)
            .await
            .print_err("AudioFile: Opening file")
            .ok_or(())?;
        let audio_metadata = extract_metadata(&mut file).await.unwrap_or_default();

        let list_chunk_size = 8 + INFO_CHUNK_SIZE as u64;
        let header_size = 48 + list_chunk_size;
        let data_size = file_size - header_size;

        // Assume fixed format: 44100 Hz, mono, IMA ADPCM (4 bits/sample, 2 samples/byte)
        let duration = (data_size / 22050) as u32;

        Ok(AudioMetadata {
            artist: audio_metadata.artist,
            title: audio_metadata.title,
            album: audio_metadata.album,
            duration,
        })
    }

    pub async fn list<'a>(
        fs: &'a SdFileSystem,
    ) -> Result<Pin<Box<dyn Stream<Item = (String<8>, AudioMetadata)> + 'a>>, ()> {
        let root = fs.root_dir();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("AudioFile: open files dir")
            .ok_or(())?;

        let stream = stream::unfold((dir.iter(), fs), |(mut iter, fs)| async move {
            match iter.next().await {
                Some(Ok(entry)) => {
                    if entry.is_file() {
                        if let Some(name) = basename(entry.short_file_name_as_bytes(), FILE_EXT) {
                            let audio_file = AudioFile::new(name.clone());
                            let metadata = audio_file.metadata(fs).await.unwrap_or_default();
                            Some((Some((name, metadata)), (iter, fs)))
                        } else {
                            warn!(
                                "AudioFile: unknown file: \\FILES\\{}",
                                entry.short_file_name_as_bytes()
                            );
                            Some((None, (iter, fs)))
                        }
                    } else {
                        Some((None, (iter, fs)))
                    }
                }
                Some(Err(_)) => Some((None, (iter, fs))),
                None => None,
            }
        })
        .filter_map(|x| async { x });

        Ok(Box::pin(stream))
    }
}

// ---- IMA ADPCM streaming decoder ----

pub struct AudioDecoder<R> {
    reader: R,
}

impl<R: Read> AudioDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Read the next ADPCM block from the reader and decode to mono i16 samples.
    /// `out` should have capacity for at least 2041 samples.
    /// Returns the number of samples written (0 = EOF, max 2041).
    pub async fn next_samples(&mut self, out: &mut [i16]) -> Result<usize, ()> {
        let mut raw = [0u8; 1024];
        let mut offset = 0;
        while offset < 1024 {
            match crate::retry(async || self.reader.read(&mut raw[offset..]).await, 2).await {
                Ok(0) => break,
                Ok(n) => offset += n,
                Err(_) => return Err(()),
            }
        }

        if offset < 4 {
            return Ok(0);
        }

        let mut state = AdpcmImaState::new();
        state.predictor = i16::from_le_bytes([raw[0], raw[1]]);
        state.step_index = raw[2].min(88);

        let max = out.len();
        let mut count = 0;
        if count < max {
            out[count] = state.predictor;
            count += 1;
        }

        for b in &raw[4..offset] {
            if count >= max {
                break;
            }
            out[count] = decode_adpcm_ima(*b & 0x0f, &mut state);
            count += 1;
            if count >= max {
                break;
            }
            out[count] = decode_adpcm_ima(*b >> 4, &mut state);
            count += 1;
        }

        Ok(count)
    }
}
