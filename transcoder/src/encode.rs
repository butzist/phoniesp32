use audio_codec_algorithms::{AdpcmImaState, encode_adpcm_ima_ms};
use std::io::{Cursor, Write};

const IMA_BLOCK_SAMPLES: usize = 2041;
const IMA_BLOCK_BYTES: usize = 1024;

/// Encode mono PCM16 samples to IMA ADPCM (WAV format 0x0011)
/// and return a valid RIFF/WAVE file as Box<[u8]>.
///
/// If sample count is even, the last sample is skipped
/// (since IMA ADPCM mono requires an odd number per block).
pub(crate) fn encode_ima_adpcm_wav(
    samples: &[i16],
    sample_rate: u32,
) -> std::io::Result<Box<[u8]>> {
    let total_blocks = samples.len() / IMA_BLOCK_SAMPLES;
    if total_blocks == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not enough samples",
        ));
    }

    // prepare output buffer
    const HEADER_SIZE: usize = 48;
    let total_adpcm_size = total_blocks * IMA_BLOCK_BYTES;
    let file_size = HEADER_SIZE + total_adpcm_size;
    let mut adpcm_buf = vec![0u8; file_size];

    // write header and data
    let mut states = [AdpcmImaState::default()];
    write_ima_adpcm_wav_header(
        &mut adpcm_buf[..HEADER_SIZE],
        total_adpcm_size as u32,
        sample_rate,
    );

    for block_idx in 0..total_blocks {
        let sample_start = block_idx * IMA_BLOCK_SAMPLES;
        let sample_end = sample_start + IMA_BLOCK_SAMPLES;
        let buffer_start = HEADER_SIZE + block_idx * IMA_BLOCK_BYTES;
        let buffer_end = buffer_start + IMA_BLOCK_BYTES;

        encode_adpcm_ima_ms(
            &samples[sample_start..sample_end],
            &mut states,
            &mut adpcm_buf[buffer_start..buffer_end],
        )
        .expect("IMA ADPCM block encode failed");
    }

    Ok(adpcm_buf.into())
}

fn write_ima_adpcm_wav_header(buffer: &mut [u8], data_size: u32, sample_rate: u32) {
    let audio_format: u16 = 0x0011; // IMA ADPCM
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 4;
    let block_align: u16 = 1024; // bytes per ADPCM block (mono)
    let byte_rate: u32 = (sample_rate * block_align as u32) / 2041; // approx
    let fmt_chunk_size: u32 = 20;
    let extra_size: u16 = 2;
    let samples_per_block: u16 = 2041;

    let riff_chunk_size: u32 = 4 + (8 + fmt_chunk_size) + (8 + data_size);

    let mut writer = Cursor::new(buffer);

    // RIFF header
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&riff_chunk_size.to_le_bytes()).unwrap();
    writer.write_all(b"WAVE").unwrap();

    // fmt chunk
    writer.write_all(b"fmt ").unwrap();
    writer.write_all(&fmt_chunk_size.to_le_bytes()).unwrap();
    writer.write_all(&audio_format.to_le_bytes()).unwrap();
    writer.write_all(&num_channels.to_le_bytes()).unwrap();
    writer.write_all(&sample_rate.to_le_bytes()).unwrap();
    writer.write_all(&byte_rate.to_le_bytes()).unwrap();
    writer.write_all(&block_align.to_le_bytes()).unwrap();
    writer.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    writer.write_all(&extra_size.to_le_bytes()).unwrap();
    writer.write_all(&samples_per_block.to_le_bytes()).unwrap();

    // data chunk header
    writer.write_all(b"data").unwrap();
    writer.write_all(&data_size.to_le_bytes()).unwrap();
}
