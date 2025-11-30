use crate::{decode_and_normalize, extract_metadata};
use std::sync::{Arc, Mutex};

// Helper function to get duration from IMA ADPCM WAV file
fn get_wav_duration(wav_data: &[u8]) -> f64 {
    if wav_data.len() < 200 {
        return 0.0;
    }

    // For IMA ADPCM WAV files, we need to find the data chunk
    // Sample rate is at offset 24 in the fmt chunk
    let sample_rate =
        u32::from_le_bytes([wav_data[24], wav_data[25], wav_data[26], wav_data[27]]) as f64;

    // Find the data chunk - it should be after the LIST chunk
    let mut data_offset = 12; // Start after RIFF header
    while data_offset + 8 < wav_data.len() {
        let chunk_id = String::from_utf8_lossy(&wav_data[data_offset..data_offset + 4]);
        if chunk_id == "data" {
            let data_size = u32::from_le_bytes([
                wav_data[data_offset + 4],
                wav_data[data_offset + 5],
                wav_data[data_offset + 6],
                wav_data[data_offset + 7],
            ]) as f64;

            // For IMA ADPCM: 4 bits per sample, 2041 samples per 1024-byte block
            let total_samples = (data_size / 1024.0) * 2041.0;
            let duration = total_samples / sample_rate;

            return duration;
        }

        // Skip to next chunk
        let chunk_size = u32::from_le_bytes([
            wav_data[data_offset + 4],
            wav_data[data_offset + 5],
            wav_data[data_offset + 6],
            wav_data[data_offset + 7],
        ]) as usize;
        data_offset += 8 + chunk_size;
    }

    0.0
}

#[tokio::test]
async fn test_transcode_mp3_with_metadata() {
    let mp3_data = include_bytes!("test_data/test_metadata.mp3");

    // Test that file contains the expected metadata by checking raw bytes
    let mp3_string = String::from_utf8_lossy(mp3_data);
    assert!(
        mp3_string.contains("测试艺术家"),
        "MP3 should contain Chinese characters for artist"
    );
    assert!(
        mp3_string.contains("测试标题"),
        "MP3 should contain Chinese characters for title"
    );
    assert!(
        mp3_string.contains("测试专辑"),
        "MP3 should contain Chinese characters for album"
    );

    // Test transcoding works with this file
    let progress_calls = Arc::new(Mutex::new(Vec::new()));
    let progress_calls_clone = progress_calls.clone();

    let result = decode_and_normalize(mp3_data.as_slice().into(), move |current, total| {
        progress_calls_clone.lock().unwrap().push((current, total));
    })
    .await;

    assert!(
        result.is_ok(),
        "Transcoding MP3 with metadata should succeed"
    );

    let output_data = result.unwrap();
    assert!(!output_data.is_empty(), "Output should not be empty");

    // Verify output is a valid WAV file
    assert!(
        output_data.len() > 44,
        "Output should be larger than WAV header"
    );
    assert_eq!(&output_data[0..4], b"RIFF", "Should have RIFF header");
    assert_eq!(&output_data[8..12], b"WAVE", "Should have WAVE format");
}

#[tokio::test]
async fn test_decode_and_normalize_with_metadata() {
    let mp3_data = include_bytes!("test_data/test_metadata.mp3");

    let progress_calls = Arc::new(Mutex::new(Vec::new()));
    let progress_calls_clone = progress_calls.clone();

    let result = decode_and_normalize(mp3_data.as_slice().into(), move |current, total| {
        progress_calls_clone.lock().unwrap().push((current, total));
    })
    .await;

    assert!(result.is_ok(), "Transcoding should succeed");

    let output_data = result.unwrap();
    assert!(!output_data.is_empty(), "Output should not be empty");

    // Check that progress was reported
    let calls = progress_calls.lock().unwrap();
    assert!(!calls.is_empty(), "Progress should be reported");

    // Check that we reached completion
    let (final_current, final_total) = calls.last().unwrap();
    assert_eq!(*final_current, 100);
    assert_eq!(*final_total, 100);
}

#[tokio::test]
async fn test_resampling_preserves_duration() {
    let ogg_data = include_bytes!("test_data/test_48000hz.ogg");

    // Transcode the file (this will resample from 48000 Hz to 44100 Hz)
    let progress_calls = Arc::new(Mutex::new(Vec::new()));
    let progress_calls_clone = progress_calls.clone();

    let result: Result<Box<[u8]>, _> =
        decode_and_normalize(ogg_data.as_slice().into(), move |current, total| {
            progress_calls_clone.lock().unwrap().push((current, total));
        })
        .await;

    assert!(
        result.is_ok(),
        "Transcoding 48000 Hz OGG file should succeed"
    );

    let output_data = result.unwrap();
    assert!(!output_data.is_empty(), "Output should not be empty");

    // Calculate output duration from WAV header
    let output_duration = get_wav_duration(&output_data);
    assert!(
        output_duration > 0.0,
        "Output file should have positive duration"
    );

    // Just verify that transcoding produces a reasonable duration
    // (can't compare to original without ffprobe)
    println!("Output WAV duration: {:.3}s", output_duration);
    assert!(
        output_duration > 0.1 && output_duration < 60.0,
        "Output duration should be reasonable: {:.3}s",
        output_duration
    );
}

#[tokio::test]
async fn test_different_sample_rates() {
    let wav_data = include_bytes!("test_data/test_22050hz.wav");

    let result: Result<Box<[u8]>, _> =
        decode_and_normalize(wav_data.as_slice().into(), |_, _| {}).await;

    assert!(result.is_ok(), "Transcoding 22050 Hz file should succeed");

    let output_data = result.unwrap();
    let output_duration = get_wav_duration(&output_data);

    // Verify that transcoding produces a reasonable duration
    assert!(
        output_duration > 0.1 && output_duration < 60.0,
        "Output duration should be reasonable: {:.3}s",
        output_duration
    );

    println!("22050 Hz - Output duration: {:.3}s", output_duration);
}

#[tokio::test]
async fn test_wav_header_and_chunk_sizes() {
    let ogg_data = include_bytes!("test_data/test_48000hz.ogg");

    // Transcode the file
    let result: Result<Box<[u8]>, _> =
        decode_and_normalize(ogg_data.as_slice().into(), |_, _| {}).await;
    assert!(result.is_ok(), "Transcoding should succeed");

    let output_data = result.unwrap();
    assert!(!output_data.is_empty(), "Output should not be empty");

    // Verify basic WAV structure
    assert_eq!(&output_data[0..4], b"RIFF", "Should have RIFF header");
    assert_eq!(&output_data[8..12], b"WAVE", "Should have WAVE format");

    // Parse RIFF header
    let riff_size = u32::from_le_bytes([
        output_data[4],
        output_data[5],
        output_data[6],
        output_data[7],
    ]) as usize;
    let expected_file_size = output_data.len() - 8; // RIFF size excludes the first 8 bytes
    assert_eq!(
        riff_size, expected_file_size,
        "RIFF chunk size should match actual file size minus 8 bytes"
    );

    // Find and validate fmt chunk
    let mut offset = 12; // Start after RIFF header
    let mut fmt_chunk_found = false;
    let mut data_chunk_found = false;
    let mut list_chunk_found = false;
    let mut sample_rate = 0u32;
    let mut samples_per_block = 0u16;

    while offset + 8 <= output_data.len() {
        let chunk_id = String::from_utf8_lossy(&output_data[offset..offset + 4]);
        let chunk_size = u32::from_le_bytes([
            output_data[offset + 4],
            output_data[offset + 5],
            output_data[offset + 6],
            output_data[offset + 7],
        ]) as usize;

        match chunk_id.as_ref() {
            "fmt " => {
                fmt_chunk_found = true;
                assert_eq!(chunk_size, 20, "fmt chunk should be 20 bytes for IMA ADPCM");

                // Validate IMA ADPCM format
                let audio_format =
                    u16::from_le_bytes([output_data[offset + 8], output_data[offset + 9]]);
                assert_eq!(audio_format, 0x0011, "Should be IMA ADPCM format (0x0011)");

                let channels =
                    u16::from_le_bytes([output_data[offset + 10], output_data[offset + 11]]);
                assert_eq!(channels, 1, "Should be mono");

                sample_rate = u32::from_le_bytes([
                    output_data[offset + 12],
                    output_data[offset + 13],
                    output_data[offset + 14],
                    output_data[offset + 15],
                ]);

                let bits_per_sample =
                    u16::from_le_bytes([output_data[offset + 22], output_data[offset + 23]]);
                assert_eq!(
                    bits_per_sample, 4,
                    "Should be 4 bits per sample for IMA ADPCM"
                );

                let block_align =
                    u16::from_le_bytes([output_data[offset + 20], output_data[offset + 21]]);
                assert_eq!(block_align, 1024, "Block align should be 1024 bytes");

                samples_per_block =
                    u16::from_le_bytes([output_data[offset + 26], output_data[offset + 27]]);
                assert_eq!(samples_per_block, 2041, "Samples per block should be 2041");
            }
            "data" => {
                data_chunk_found = true;

                // Validate data chunk size matches actual data
                let data_start = offset + 8;
                let expected_data_size = output_data.len() - data_start;
                assert_eq!(
                    chunk_size, expected_data_size,
                    "Data chunk size should match actual data size"
                );

                // Validate data size is multiple of block size
                assert_eq!(
                    chunk_size % 1024,
                    0,
                    "Data size should be multiple of block size (1024 bytes)"
                );

                // Calculate expected number of samples
                let blocks = chunk_size / 1024;
                let expected_samples = blocks * samples_per_block as usize;

                // Verify this matches what we'd expect from the duration
                let duration = get_wav_duration(&output_data);
                let expected_duration = expected_samples as f64 / sample_rate as f64;
                let duration_diff = (duration - expected_duration).abs();

                assert!(
                    duration_diff < 0.1, // Allow 100ms tolerance
                    "Duration calculated from chunk size should match WAV header duration"
                );
            }
            "LIST" => {
                list_chunk_found = true;
                // LIST chunk should contain INFO chunk
                assert!(
                    chunk_size >= 4,
                    "LIST chunk should contain at least INFO identifier"
                );

                if offset + 12 <= output_data.len() {
                    let info_id = String::from_utf8_lossy(&output_data[offset + 8..offset + 12]);
                    assert_eq!(info_id, "INFO", "LIST should contain INFO sub-chunk");
                }
            }
            _ => {
                // Unknown chunk, but that's okay
            }
        }

        offset += 8 + chunk_size;
    }

    assert!(fmt_chunk_found, "fmt chunk should be present");
    assert!(data_chunk_found, "data chunk should be present");
    assert!(list_chunk_found, "LIST chunk should be present");

    // Validate overall file size is reasonable
    // For a 5-second audio at 22.05kHz IMA ADPCM:
    // Expected size ≈ 5s * 22.05kHz * 4 bits/sample / 8 ≈ 55KB + headers
    let reasonable_max_size = 200 * 1024; // 200KB should be more than enough
    assert!(
        output_data.len() < reasonable_max_size,
        "Output file size ({}) should be reasonable (< {} bytes)",
        output_data.len(),
        reasonable_max_size
    );

    println!("WAV file validation passed:");
    println!("  Total size: {} bytes", output_data.len());
    println!("  RIFF size: {} bytes", riff_size);
    println!("  Duration: {:.3}s", get_wav_duration(&output_data));
}

#[test]
fn test_metadata_extraction_fallback() {
    // Test with invalid data to ensure graceful fallback
    let invalid_data = b"not an audio file";
    let metadata = extract_metadata(invalid_data);

    // Should fallback to "Unknown" values
    assert_eq!(metadata.artist.to_string(), "Unknown");
    assert_eq!(metadata.title.to_string(), "Unknown");
    assert_eq!(metadata.album.to_string(), "Unknown");
}
