mod decode;
mod encode;
mod error;
mod normalize;
mod resample;

pub use error::TranscodeError;

const OUT_RATE: u32 = 44100;

pub fn decode_and_normalize(
    input: Box<[u8]>,
    mut progress: impl FnMut(usize, usize) + Clone,
) -> Result<Box<[u8]>, TranscodeError> {
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

    let file = encode::encode_ima_adpcm_wav(&samples, OUT_RATE)?;

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
