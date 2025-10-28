use super::audio_file::AudioFile;
use crate::sd::SdFileSystem;
use crate::{with_extension, PrintErr};
use alloc::{string::ToString, vec::Vec};
use defmt::error;
use embedded_io_async::{Read, Write};
use heapless::String;

const PLAYLIST_DIR: &str = "fobs";
const PLAYLIST_EXT: &str = "m3u";

pub struct PlayListRef(String<8>);

impl PlayListRef {
    pub fn new(name: String<8>) -> Self {
        Self(name)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub async fn read(self, fs: &SdFileSystem<'_>) -> Result<Playlist, ()> {
        let root = fs.root_dir();
        let dir = root
            .open_dir(PLAYLIST_DIR)
            .await
            .print_err("Failed to open fobs directory")
            .ok_or(())?;
        let fname = with_extension(&self.0, PLAYLIST_EXT).unwrap();
        let mut file = dir
            .open_file(&fname)
            .await
            .print_err("Failed to open playlist file")
            .ok_or(())?;

        // Read entire file
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 256];
        loop {
            match file.read(&mut temp_buf).await {
                Ok(0) => break,
                Ok(n) => buffer.extend_from_slice(&temp_buf[..n]),
                Err(_) => {
                    error!("Error reading playlist file");
                    return Err(());
                }
            }
        }

        // Parse
        let content = core::str::from_utf8(&buffer)
            .map_err(|_| ())
            .print_err("Invalid UTF-8 in playlist")
            .ok_or(())?;
        let mut files = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(audio_file) = AudioFile::from_path(line) {
                files.push(audio_file);
            }
        }

        Ok(Playlist::new(self.0, files))
    }
}

pub struct Playlist {
    pub name: String<8>,
    pub files: Vec<AudioFile>,
}

impl Playlist {
    pub fn new(name: String<8>, files: Vec<AudioFile>) -> Self {
        Self { name, files }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn write(
        fs: &SdFileSystem<'_>,
        name: String<8>,
        files: &[AudioFile],
    ) -> Result<(), ()> {
        let root = fs.root_dir();
        let dir = if !root.dir_exists(PLAYLIST_DIR).await.unwrap_or(false) {
            root.create_dir(PLAYLIST_DIR).await.unwrap()
        } else {
            root.open_dir(PLAYLIST_DIR).await.unwrap()
        };

        let fname = with_extension(&name, PLAYLIST_EXT).unwrap();
        let mut file = dir.create_file(&fname).await.unwrap();
        file.truncate().await.unwrap();

        file.write_all(b"#EXTM3U\r\n").await.unwrap();

        for file_entry in files {
            let info = file_entry.info(fs).await.unwrap();
            file.write_all(b"#EXTINF:").await.unwrap();
            file.write_all(info.duration.to_string().as_bytes())
                .await
                .unwrap();
            file.write_all(b",").await.unwrap();
            file.write_all(info.artist.as_bytes()).await.unwrap();
            file.write_all(b" - ").await.unwrap();
            file.write_all(info.title.as_bytes()).await.unwrap();
            file.write_all(b"\r\n").await.unwrap();
            file.write_all(b"..\\files\\").await.unwrap();
            file.write_all(file_entry.name().as_bytes()).await.unwrap();
            file.write_all(b".wav\r\n").await.unwrap();
        }
        file.flush().await.unwrap();

        Ok(())
    }
}
