use heapless::String;

use crate::sd::{FileHandle, SdFileSystem};
use crate::{with_extension, PrintErr};

const FILE_DIR: &str = "files";
const FILE_EXT: &str = "wav";

#[derive(Clone)]
pub struct Info {
    pub artist: String<32>,
    pub title: String<32>,
    pub album: String<32>,
    pub duration: u32,
}

pub struct AudioFile(String<8>);

impl AudioFile {
    pub fn new(name: String<8>) -> Self {
        Self(name)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub async fn open<'a>(&self, fs: &'a SdFileSystem<'a>) -> Result<FileHandle<'a>, ()> {
        let root = fs.root_dir();
        let fname = with_extension(&self.0, FILE_EXT).unwrap();
        let dir = root
            .open_dir(FILE_DIR)
            .await
            .print_err("Opening files directory")
            .ok_or(())?;

        dir.open_file(&fname)
            .await
            .print_err("Opening file")
            .ok_or(())
    }

    pub async fn create<'a>(&self, fs: &'a SdFileSystem<'a>) -> Result<FileHandle<'a>, ()> {
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

    pub fn from_path(path: &str) -> Option<Self> {
        if path.starts_with("..\\files\\") && path.ends_with(".wav") {
            let start = "..\\files\\".len();
            let end = path.len() - ".wav".len();

            Some(Self(path[start..end].parse::<String<8>>().ok()?))
        } else {
            None
        }
    }

    pub async fn info(&self, fs: &SdFileSystem<'_>) -> Result<Info, ()> {
        // For now, fake info. In future, parse ID3 tags or WAV header for duration.
        let _file = self.open(fs).await?; // Open to potentially read metadata
        Ok(Info {
            artist: "Unknown Artist".try_into().unwrap(),
            title: "Unknown Track".try_into().unwrap(),
            album: "Unknown Album".try_into().unwrap(),
            duration: 60, // Fake duration in seconds
        })
    }
}
