use clap::Args;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Args)]
#[command(about = "Transcode an audio file")]
pub struct TranscodeCommand {
    /// Input audio file path
    pub input_file: PathBuf,
    /// Override artist metadata
    #[arg(long)]
    pub artist: Option<String>,
    /// Override album metadata
    #[arg(long)]
    pub album: Option<String>,
    /// Override title metadata
    #[arg(long)]
    pub title: Option<String>,
}

impl TranscodeCommand {
    pub async fn execute(self) -> Result<(), Box<dyn std::error::Error>> {
        use comfy_table::{Table, presets::UTF8_FULL};
        use indicatif::{ProgressBar, ProgressStyle};
        use transcoder::{decode_and_normalize, extract_metadata};

        // Read input file
        let input_data = std::fs::read(&self.input_file)?;

        // Create progress bar
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Transcoding...");

        // Transcode with progress callback
        let mut result = decode_and_normalize(input_data.into(), |current, _total| {
            pb.set_position(current as u64);
        })
        .await?;

        pb.finish_with_message("Transcoding complete!");

        // Updata metadata in transcoded buffer
        let transcoded_metadata = extract_metadata(&result.data);
        let final_metadata = self.override_metadata(transcoded_metadata)?;
        self.update_metadata_in_buffer(&mut result.data, &final_metadata)
            .await?;
        let actual_metadata = audio_file_utils::metadata::extract_metadata(&mut &result.data[..])
            .await
            .unwrap_or_default();

        // Write output file with updated metadata
        std::fs::write(&result.filename, &result.data)?;

        // Display metadata table
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        table.add_row(vec!["Output File", &result.filename]);
        table.add_row(vec!["Artist", actual_metadata.artist.as_ref()]);
        table.add_row(vec!["Title", actual_metadata.title.as_ref()]);
        table.add_row(vec!["Album", actual_metadata.album.as_ref()]);
        table.add_row(vec!["File Size", &format!("{} bytes", result.data.len())]);

        println!("{}", table);

        Ok(())
    }

    /// Update metadata inline in the WAV buffer by rewriting the LIST INFO chunk
    async fn update_metadata_in_buffer(
        &self,
        data: &mut [u8],
        metadata: &audio_file_utils::metadata::Metadata,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use audio_file_utils::metadata::{INFO_CHUNK_SIZE, write_info_chunk};

        let data_len = data.len();
        let mut cursor = Cursor::new(data);

        // Seek to the LIST section 40 bytes from start (based on web implementation)
        cursor.seek(SeekFrom::Start(40))?;

        // Read and validate "LIST" tag
        let mut list_tag = [0u8; 4];
        cursor.read_exact(&mut list_tag)?;
        if &list_tag != b"LIST" {
            return Err("LIST tag not found at expected position".into());
        }

        // Read and validate length = 124 (INFO_CHUNK_SIZE)
        let mut length_bytes = [0u8; 4];
        cursor.read_exact(&mut length_bytes)?;
        let length = u32::from_le_bytes(length_bytes);
        if length != INFO_CHUNK_SIZE as u32 {
            return Err("Unexpected LIST chunk length".into());
        }

        // Get the position where INFO chunk should be written (after LIST header)
        let info_start = cursor.position();
        let info_end = info_start + INFO_CHUNK_SIZE as u64;

        if info_end as usize > data_len {
            return Err("INFO chunk extends beyond buffer".into());
        }

        let info_buffer = &mut cursor.get_mut()[info_start as usize..info_end as usize];

        // Write the metadata to the INFO chunk
        write_info_chunk(info_buffer, metadata)
            .await
            .map_err(|_| std::io::Error::other("Failed to write info chunk"))?;

        Ok(())
    }

    /// Override transcoded metadata with command line parameters, if provided
    fn override_metadata(
        &self,
        transcoded_metadata: audio_file_utils::metadata::Metadata,
    ) -> Result<audio_file_utils::metadata::Metadata, Box<dyn std::error::Error>> {
        let final_artist = if let Some(ref artist_override) = self.artist {
            artist_override.as_str().try_into().map_err(|_| {
                format!(
                    "Artist '{}' is too long (max 31 characters)",
                    artist_override
                )
            })?
        } else {
            transcoded_metadata.artist.clone()
        };

        let final_album = if let Some(ref album_override) = self.album {
            album_override.as_str().try_into().map_err(|_| {
                format!("Album '{}' is too long (max 31 characters)", album_override)
            })?
        } else {
            transcoded_metadata.album.clone()
        };

        let final_title = if let Some(ref title_override) = self.title {
            title_override.as_str().try_into().map_err(|_| {
                format!("Title '{}' is too long (max 31 characters)", title_override)
            })?
        } else {
            transcoded_metadata.title.clone()
        };

        Ok(audio_file_utils::metadata::Metadata {
            artist: final_artist,
            title: final_title,
            album: final_album,
        })
    }
}
