mod decode;
mod encode;
mod error;
mod normalize;
mod resample;

pub use error::TranscodeError;

use audio_file_utils::metadata::Metadata;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::formats::FormatOptions;
use symphonia::default;

const OUT_RATE: u32 = 44100;

pub fn extract_metadata(input: &[u8]) -> Metadata {
    let cursor = std::io::Cursor::new(input.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let mut probed = match default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts) {
        Ok(p) => p,
        Err(_) => return Metadata::default(),
    };

    let mut artist = None;
    let mut title = None;
    let mut album = None;

    if let Some(meta) = probed.metadata.get() {
        if let Some(metadata) = meta.current() {
            for tag in metadata.tags() {
                match tag.std_key {
                    Some(StandardTagKey::Artist) => artist = Some(tag.value.to_string()),
                    Some(StandardTagKey::TrackTitle) => title = Some(tag.value.to_string()),
                    Some(StandardTagKey::Album) => album = Some(tag.value.to_string()),
                    _ => {}
                }
            }
        }
    }

    let artist_str = artist.unwrap_or("Unknown".to_string());
    let title_str = title.unwrap_or("Unknown".to_string());
    let album_str = album.unwrap_or("Unknown".to_string());

    Metadata {
        artist: artist_str.as_str().try_into().unwrap_or("Unknown".try_into().unwrap()),
        title: title_str.as_str().try_into().unwrap_or("Unknown".try_into().unwrap()),
        album: album_str.as_str().try_into().unwrap_or("Unknown".try_into().unwrap()),
    }
}

pub async fn decode_and_normalize(
    input: Box<[u8]>,
    mut progress: impl FnMut(usize, usize) + Clone,
) -> Result<Box<[u8]>, TranscodeError> {
    let metadata = extract_metadata(&input);

    let make_progress = |from: usize, to: usize| {
        let mut progress = progress.clone();
        progress(from, 100);
        move |current: usize, total: usize| {
            let out_range = to - from;
            let out_current = from + current * out_range / total;
            progress(out_current, 100);
        }
    };

    let (downmixed, sample_rate) = decode::decode_to_mono(input, make_progress(0, 33))?;
    let mut samples = resample::resample(
        &downmixed,
        OUT_RATE as f64 / sample_rate as f64,
        make_progress(34, 67),
    )?;

    // Spotify -14 lufs
    normalize::loudness_normalize(&mut samples, OUT_RATE, 1, -14.0, make_progress(68, 85));

    let samples = to_i16(samples.into());
    progress(90, 100);

    let file = encode::encode_ima_adpcm_wav(&samples, OUT_RATE, &metadata).await?;

    progress(100, 100);
    Ok(file)
}

fn to_i16(samples: Box<[f32]>) -> Box<[i16]> {
    let max = i16::MAX - 1;
    let result: Vec<i16> = samples
        .into_iter()
        .map(|x| (x.clamp(-1.0, 1.0) * max as f32) as i16)
        .collect();

    result.into()
}
