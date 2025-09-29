use std::io::{Cursor, ErrorKind};

use crate::error::TranscodeError;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
pub(crate) use symphonia::core::errors::Error as DecoderError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub(crate) fn decode_to_mono(
    input: Box<[u8]>,
    progress: impl Fn(usize, usize),
) -> Result<(Box<[f32]>, u32), TranscodeError> {
    let cursor = Cursor::new(input);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    // Use the default options for metadata and format readers.
    let hint = Hint::default();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Get the instantiated format reader.
    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(TranscodeError::NoAudioTracks)?;

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    let input_sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(TranscodeError::UnknownSampleRate)?;
    let input_channels = track
        .codec_params
        .channels
        .ok_or(TranscodeError::UnknownChannelsCount)?
        .count();
    let duration = track.codec_params.n_frames;

    let mut downmixed = Vec::new();

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecoderError::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(DecoderError::IoError(err)) => {
                if err.kind() == ErrorKind::UnexpectedEof {
                    // Looks like this is actually the expected EOF
                    break;
                }
                return Err(err.into());
            }
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                return Err(err.into());
            }
        };

        if let Some(duration) = duration {
            progress(packet.ts() as usize, duration as usize);
        }

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(DecoderError::IoError(_) | DecoderError::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                return Err(err.into());
            }
        };

        // Prepare input for processing
        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_interleaved_ref(decoded);

        downmix_to_mono(sample_buf.samples(), input_channels, &mut downmixed);
    }

    Ok((downmixed.into(), input_sample_rate))
}

fn downmix_to_mono<E: Extend<f32>>(samples: &[f32], channels: usize, output: &mut E) {
    if channels == 1 {
        output.extend(samples.iter().copied());
    } else {
        let downmixed = samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32);

        output.extend(downmixed);
    }
}
